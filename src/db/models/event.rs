use crate::db::DbConn;
use serde_json::Value;

use crate::api::EmptyResult;
use crate::error::MapResult;

use chrono::{NaiveDateTime, Utc};

// use super::User;

db_object! {
    // Upstream: https://github.com/bitwarden/server/blob/master/src/Core/Services/Implementations/EventService.cs
    // Upstream: https://github.com/bitwarden/server/blob/master/src/Core/Models/Api/Response/EventResponseModel.cs
    // Upstream SQL: https://github.com/bitwarden/server/blob/master/src/Sql/dbo/Tables/Event.sql
    #[derive(Debug, Identifiable, Queryable, Insertable, Associations, AsChangeset)]
    #[table_name = "event"]
    #[primary_key(uuid)]
    pub struct Event {
        pub uuid: String,
        pub event_type: i32, // EventType
        pub user_uuid: Option<String>,
        pub org_uuid: Option<String>,
        pub cipher_uuid: Option<String>,
        pub collection_uuid: Option<String>,
        pub group_uuid: Option<String>,
        pub org_user_uuid: Option<String>,
        pub act_user_uuid: Option<String>,
        // Upstream enum: https://github.com/bitwarden/server/blob/master/src/Core/Enums/DeviceType.cs
        pub device_type: Option<i32>,
        pub ip_address: Option<String>,
        pub event_date: NaiveDateTime,
    }
}

// Upstream enum: https://github.com/bitwarden/server/blob/master/src/Core/Enums/EventType.cs
// #[derive(FromPrimitive)]
#[allow(dead_code)]
pub enum EventType {
    // User
    UserLoggedIn = 1000,
    UserChangedPassword = 1001,
    UserUpdated2fa = 1002,
    UserDisabled2fa = 1003,
    UserRecovered2fa = 1004,
    UserFailedLogIn = 1005,
    UserFailedLogIn2fa = 1006,
    UserClientExportedVault = 1007,
    // Cipher
    CipherCreated = 1100,
    CipherUpdated = 1101,
    CipherDeleted = 1102,
    CipherAttachmentCreated = 1103,
    CipherAttachmentDeleted = 1104,
    CipherShared = 1105,
    CipherUpdatedCollections = 1106,
    CipherClientViewed = 1107,
    CipherClientToggledPasswordVisible = 1108,
    CipherClientToggledHiddenFieldVisible = 1109,
    CipherClientToggledCardCodeVisible = 1110,
    CipherClientCopiedPassword = 1111,
    CipherClientCopiedHiddenField = 1112,
    CipherClientCopiedCardCode = 1113,
    CipherClientAutofilled = 1114,
    // Collection
    CollectionCreated = 1300,
    CollectionUpdated = 1301,
    CollectionDeleted = 1302,
    // Group (DISABLE THIS, SINCE WE DO NOT SUPPORT GROUPS YET)
    // IF ONE OF THESE ID'S IS RETURNED IT WILL CRASH THE WEB VAULT
    // THIS BECAUSE THE GROUP_UUID WILL BE NULL
    // GroupCreated = 1400,
    // GroupUpdated = 1401,
    // GroupDeleted = 1402,
    // OrganizationUser
    OrganizationUserInvited = 1500,
    OrganizationUserConfirmed = 1501,
    OrganizationUserUpdated = 1502,
    OrganizationUserRemoved = 1503,
    OrganizationUserUpdatedGroups = 1504,
    // Organization
    OrganizationUpdated = 1600,
    OrganizationPurgedVault = 1601,
    // OrganizationClientExportedVault = 1602,
}

/// Local methods
impl Event {
    pub fn new(
        event_type: i32,
        event_date: Option<NaiveDateTime>,
    ) -> Self {
        let edate = match event_date {
            Some(d) => d,
            None => Utc::now().naive_utc(),
        };

        Self {
            uuid: crate::util::get_uuid(),
            event_type: event_type as i32,
            user_uuid: None,
            org_uuid: None,
            cipher_uuid: None,
            collection_uuid: None,
            group_uuid: None,
            org_user_uuid: None,
            act_user_uuid: None,
            device_type: None,
            ip_address: None,
            event_date: edate,
        }
    }

    pub fn to_json(&self) -> Value {
        use crate::util::format_date;

        json!({
            "Type": self.event_type,
            "UserId": self.user_uuid,
            "OrganizationId": self.org_uuid,
            "CipherId": self.cipher_uuid,
            "CollectionId": self.collection_uuid,
            "GroupId": self.group_uuid,
            "OrganizationUserId": self.org_user_uuid,
            "ActingUserId": self.act_user_uuid,
            "Date": format_date(&self.event_date),
            "DeviceType": self.device_type,
            "IpAddress": self.ip_address,
        })
    }
}

/// Database methods
/// https://github.com/bitwarden/server/blob/master/src/Core/Services/Implementations/EventService.cs
impl Event {
    /// #############
    /// Basic Queries
    pub fn save(&self, conn: &DbConn) -> EmptyResult {
        db_run! { conn:
            sqlite, mysql {
                diesel::replace_into(event::table)
                .values(EventDb::to_db(self))
                .execute(conn)
                .map_res("Error saving event")
            }
            postgresql {
                diesel::insert_into(event::table)
                .values(EventDb::to_db(self))
                .on_conflict(event::uuid)
                .do_update()
                .set(EventDb::to_db(self))
                .execute(conn)
                .map_res("Error saving event")
            }
        }
    }

    pub fn delete(self, conn: &DbConn) -> EmptyResult {
        db_run! { conn: {
            diesel::delete(event::table.filter(event::uuid.eq(self.uuid)))
                .execute(conn)
                .map_res("Error deleting event")
        }}
    }

    /// ##############
    /// Custom Queries
    pub fn find_by_organization_uuid(org_uuid: &str, start: &NaiveDateTime, end: &NaiveDateTime, conn: &DbConn) -> Vec<Self> {
        db_run! { conn: {
            event::table
                .filter(event::org_uuid.eq(org_uuid))
                .filter(event::event_date.between(start, end))
                .load::<EventDb>(conn)
                .expect("Error filtering events")
                .from_db()
        }}
    }

    pub fn find_by_cipher_uuid(cipher_uuid: &str, start: &NaiveDateTime, end: &NaiveDateTime, conn: &DbConn) -> Vec<Self> {
        db_run! { conn: {
            event::table
                .filter(event::cipher_uuid.eq(cipher_uuid))
                .filter(event::event_date.between(start, end))
                .load::<EventDb>(conn)
                .expect("Error filtering events")
                .from_db()
        }}
    }

}