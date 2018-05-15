use chrono::{NaiveDateTime, Utc};
use serde_json::Value as JsonValue;

use uuid::Uuid;

use super::{User, Organization, UserOrganization, Attachment, FolderCipher, CollectionCipher, UserOrgType};

#[derive(Debug, Identifiable, Queryable, Insertable, Associations)]
#[table_name = "ciphers"]
#[belongs_to(User, foreign_key = "user_uuid")]
#[belongs_to(Organization, foreign_key = "organization_uuid")]
#[primary_key(uuid)]
pub struct Cipher {
    pub uuid: String,
    pub created_at: NaiveDateTime,
    pub updated_at: NaiveDateTime,

    pub user_uuid: Option<String>,
    pub organization_uuid: Option<String>,

    /*
    Login = 1,
    SecureNote = 2,
    Card = 3,
    Identity = 4
    */
    pub type_: i32,
    pub name: String,
    pub notes: Option<String>,
    pub fields: Option<String>,

    pub data: String,

    pub favorite: bool,
}

/// Local methods
impl Cipher {
    pub fn new(type_: i32, name: String) -> Self {
        let now = Utc::now().naive_utc();

        Self {
            uuid: Uuid::new_v4().to_string(),
            created_at: now,
            updated_at: now,

            user_uuid: None,
            organization_uuid: None,

            type_,
            favorite: false,
            name,

            notes: None,
            fields: None,

            data: String::new(),
        }
    }
}

use diesel;
use diesel::prelude::*;
use db::DbConn;
use db::schema::*;

/// Database methods
impl Cipher {
    pub fn to_json(&self, host: &str, user_uuid: &str, conn: &DbConn) -> JsonValue {
        use serde_json;
        use util::format_date;
        use super::Attachment;

        let attachments = Attachment::find_by_cipher(&self.uuid, conn);
        let attachments_json: Vec<JsonValue> = attachments.iter().map(|c| c.to_json(host)).collect();

        let fields_json: JsonValue = if let Some(ref fields) = self.fields {
            serde_json::from_str(fields).unwrap()
        } else { JsonValue::Null };

        let mut data_json: JsonValue = serde_json::from_str(&self.data).unwrap();

        // TODO: ******* Backwards compat start **********
        // To remove backwards compatibility, just remove this entire section
        // and remove the compat code from ciphers::update_cipher_from_data
        if self.type_ == 1 && data_json["Uris"].is_array() {
            let uri = data_json["Uris"][0]["uri"].clone();
            data_json["Uri"] = uri;
        }
        // TODO: ******* Backwards compat end **********

        let mut json_object = json!({
            "Id": self.uuid,
            "Type": self.type_,
            "RevisionDate": format_date(&self.updated_at),
            "FolderId": self.get_folder_uuid(&user_uuid, &conn),
            "Favorite": self.favorite,
            "OrganizationId": self.organization_uuid,
            "Attachments": attachments_json,
            "OrganizationUseTotp": false,
            "CollectionIds": self.get_collections(user_uuid, &conn),

            "Name": self.name,
            "Notes": self.notes,
            "Fields": fields_json,

            "Data": data_json,

            "Object": "cipher",
            "Edit": true,
        });

        let key = match self.type_ {
            1 => "Login",
            2 => "SecureNote",
            3 => "Card",
            4 => "Identity",
            _ => panic!("Wrong type"),
        };

        json_object[key] = data_json;
        json_object
    }

    pub fn save(&mut self, conn: &DbConn) -> bool {
        self.updated_at = Utc::now().naive_utc();

        match diesel::replace_into(ciphers::table)
            .values(&*self)
            .execute(&**conn) {
            Ok(1) => true, // One row inserted
            _ => false,
        }
    }

    pub fn delete(self, conn: &DbConn) -> QueryResult<()> {
        FolderCipher::delete_all_by_cipher(&self.uuid, &conn)?;
        CollectionCipher::delete_all_by_cipher(&self.uuid, &conn)?;
        Attachment::delete_all_by_cipher(&self.uuid, &conn)?;

        diesel::delete(
            ciphers::table.filter(
                ciphers::uuid.eq(self.uuid)
            )
        ).execute(&**conn).and(Ok(()))
    }

    pub fn move_to_folder(&self, folder_uuid: Option<String>, user_uuid: &str, conn: &DbConn) -> Result<(), &str> {
        match self.get_folder_uuid(&user_uuid, &conn)  {
            None => {
                match folder_uuid {
                    Some(new_folder) => {
                        let folder_cipher = FolderCipher::new(&new_folder, &self.uuid);
                        folder_cipher.save(&conn).or(Err("Couldn't save folder setting"))
                    },
                    None => Ok(()) //nothing to do
                }
            },
            Some(current_folder) => {
                match folder_uuid {
                    Some(new_folder) => {
                        if current_folder == new_folder {
                            Ok(()) //nothing to do
                        } else {
                            match FolderCipher::find_by_folder_and_cipher(&current_folder, &self.uuid, &conn) {
                                Some(current_folder) => {
                                    current_folder.delete(&conn).or(Err("Failed removing old folder mapping"))
                                },
                                None => Ok(()) // Weird, but nothing to do
                            }.and_then(
                                |()| FolderCipher::new(&new_folder, &self.uuid)
                                .save(&conn).or(Err("Couldn't save folder setting"))
                            )
                        }
                    },
                    None => {
                        match FolderCipher::find_by_folder_and_cipher(&current_folder, &self.uuid, &conn) {
                            Some(current_folder) => {
                                current_folder.delete(&conn).or(Err("Failed removing old folder mapping"))
                            },
                            None => Err("Couldn't move from previous folder")
                        }
                    }
                }
            }
        }
    }

    pub fn is_write_accessible_to_user(&self, user_uuid: &str, conn: &DbConn) -> bool {
        match self.user_uuid {
            Some(ref self_user_uuid) => self_user_uuid == user_uuid, // cipher directly owned by user
            None =>{
                match self.organization_uuid {
                    Some(ref org_uuid) => {
                        match users_organizations::table
                        .filter(users_organizations::org_uuid.eq(org_uuid))
                        .filter(users_organizations::user_uuid.eq(user_uuid))
                        .filter(users_organizations::access_all.eq(true))
                        .first::<UserOrganization>(&**conn).ok() {
                            Some(_) => true,
                            None => false //TODO R/W access on collection
                        }
                    },
                    None => false // cipher not in organization and not owned by user
                }
            }
        }
    }

    pub fn is_accessible_to_user(&self, user_uuid: &str, conn: &DbConn) -> bool {
        // TODO also check for read-only access
        self.is_write_accessible_to_user(user_uuid, conn)
    }

    pub fn get_folder_uuid(&self, user_uuid: &str, conn: &DbConn) -> Option<String> {
        folders_ciphers::table.inner_join(folders::table)
            .filter(folders::user_uuid.eq(&user_uuid))
            .filter(folders_ciphers::cipher_uuid.eq(&self.uuid))
            .select(folders_ciphers::folder_uuid)
            .first::<String>(&**conn).ok()
    }

    pub fn find_by_uuid(uuid: &str, conn: &DbConn) -> Option<Self> {
        ciphers::table
            .filter(ciphers::uuid.eq(uuid))
            .first::<Self>(&**conn).ok()
    }

    // Find all ciphers accesible to user
    pub fn find_by_user(user_uuid: &str, conn: &DbConn) -> Vec<Self> {
        ciphers::table
        .left_join(users_organizations::table.on(
            ciphers::organization_uuid.eq(users_organizations::org_uuid.nullable()).and(
                users_organizations::user_uuid.eq(user_uuid)
            )
        ))
        .left_join(ciphers_collections::table)
        .left_join(users_collections::table.on(
            ciphers_collections::collection_uuid.eq(users_collections::collection_uuid)
        ))
        .filter(ciphers::user_uuid.eq(user_uuid).or( // Cipher owner
            users_organizations::access_all.eq(true).or( // access_all in Organization
                users_organizations::type_.le(UserOrgType::Admin as i32).or( // Org admin or owner
                    users_collections::user_uuid.eq(user_uuid) // Access to Collection
                )
            )
        ))
        .select(ciphers::all_columns)
        .distinct()
        .load::<Self>(&**conn).expect("Error loading ciphers")
    }

    // Find all ciphers directly owned by user
    pub fn find_owned_by_user(user_uuid: &str, conn: &DbConn) -> Vec<Self> {
        ciphers::table
        .filter(ciphers::user_uuid.eq(user_uuid))
        .load::<Self>(&**conn).expect("Error loading ciphers")
    }

    pub fn find_by_org(org_uuid: &str, conn: &DbConn) -> Vec<Self> {
        ciphers::table
            .filter(ciphers::organization_uuid.eq(org_uuid))
            .load::<Self>(&**conn).expect("Error loading ciphers")
    }

    pub fn find_by_folder(folder_uuid: &str, conn: &DbConn) -> Vec<Self> {
        folders_ciphers::table.inner_join(ciphers::table)
            .filter(folders_ciphers::folder_uuid.eq(folder_uuid))
            .select(ciphers::all_columns)
            .load::<Self>(&**conn).expect("Error loading ciphers")
    }

    pub fn get_collections(&self, user_id: &str, conn: &DbConn) -> Vec<String> {
        ciphers_collections::table
        .inner_join(collections::table.on(
            collections::uuid.eq(ciphers_collections::collection_uuid)
        ))
        .inner_join(users_organizations::table.on(
            users_organizations::org_uuid.eq(collections::org_uuid).and(
                users_organizations::user_uuid.eq(user_id)
            )
        ))
        .left_join(users_collections::table.on(
            users_collections::collection_uuid.eq(ciphers_collections::collection_uuid)
        ))
        .filter(ciphers_collections::cipher_uuid.eq(&self.uuid))
        .filter(users_collections::user_uuid.eq(user_id).or( // User has access to collection
            users_organizations::access_all.eq(true).or( // User has access all
                users_organizations::type_.le(UserOrgType::Admin as i32) // User is admin or owner
            )
        ))
        .select(ciphers_collections::collection_uuid)
        .load::<String>(&**conn).unwrap_or(vec![])
    }
}
