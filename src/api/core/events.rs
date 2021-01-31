use rocket::request::Form;
use rocket::Route;
use rocket_contrib::json::Json;
use serde_json::Value;

use chrono::{NaiveDateTime};

use crate::api::{JsonResult, JsonUpcaseVec, EmptyResult};

use crate::auth::{AdminHeaders, Headers, ClientIp};

use crate::db::models::{Event, Cipher};
use crate::db::DbConn;

use crate::util::parse_date;

use std::net::IpAddr;

/// ###############################################################################################################
/// /api routes

pub fn routes() -> Vec<Route> {
    routes![
        get_org_events,
        get_cipher_events,
    ]
}

#[derive(FromForm, Debug)]
struct EventRange {
    start: String,
    end: String,
    // Upstream info: https://github.com/bitwarden/server/blob/master/src/Core/Models/Data/PageOptions.cs
    //                https://github.com/bitwarden/server/blob/master/src/Core/Models/Data/PagedResult.cs
    #[form(field = "continuationToken")]
    continuation_token: Option<String>,
}

// Upstream: https://github.com/bitwarden/server/blob/master/src/Api/Controllers/EventsController.cs
#[get("/organizations/<org_id>/events?<data..>")]
fn get_org_events(org_id: String, data: Form<EventRange>, _headers: AdminHeaders, conn: DbConn) -> JsonResult {
    let start_date = parse_date(&data.start);
    let end_date = parse_date(&data.end);

    let events_json: Vec<Value> = Event::find_by_organization_uuid(&org_id, &start_date, &end_date, &conn)
        .iter()
        .map(Event::to_json)
        .collect();

    Ok(Json(json!({
        "Data": events_json,
        "Object": "list",
        "ContinuationToken": null,
    })))
}

#[get("/ciphers/<cipher_id>/events?<data..>")]
fn get_cipher_events(cipher_id: String, data: Form<EventRange>, _headers: Headers, conn: DbConn) -> JsonResult {
    let start_date = parse_date(&data.start);
    let end_date = parse_date(&data.end);

    let events_json: Vec<Value> = Event::find_by_cipher_uuid(&cipher_id, &start_date, &end_date, &conn)
        .iter()
        .map(Event::to_json)
        .collect();

    Ok(Json(json!({
        "Data": events_json,
        "Object": "list",
        "ContinuationToken": null,
    })))
}



/// ###############################################################################################################
/// /events routes

pub fn main_routes() -> Vec<Route> {
    routes![
        post_events_collect,
    ]
}

#[derive(Deserialize, Debug)]
#[allow(non_snake_case)]
struct EventCollection {
    // Mandatory
    Type: i32,
    Date: String,

    // Optional
    CipherId: Option<String>,
}

// Upstream: https://github.com/bitwarden/server/blob/master/src/Events/Controllers/CollectController.cs
// Upstream: https://github.com/bitwarden/server/blob/master/src/Core/Services/Implementations/EventService.cs
#[post("/collect", format = "application/json", data = "<data>")]
fn post_events_collect(data: JsonUpcaseVec<EventCollection>, headers: Headers, conn: DbConn, ip: ClientIp) -> EmptyResult {
    for d in data.iter().map(|d| &d.data) {
        if let Some(cipher_uuid) = &d.CipherId {
            let event_date = parse_date(&d.Date);
            new_cipher_event(cipher_uuid, d.Type, event_date, &headers.user.uuid, headers.device.atype, &ip.ip, &conn)?;
        }
    }

    Ok(())
}

pub fn new_cipher_event(cipher_uuid: &str, event_type: i32, event_date: NaiveDateTime, user_uuid: &str, device_type: i32, ip: &IpAddr, conn: &DbConn) -> EmptyResult {
    // Check if we can get some more information about the cipher and use that.
    let mut event = Event::new(event_type, Some(event_date));
    if let Some(cipher) = Cipher::find_by_uuid(cipher_uuid, &conn) {
        event.org_uuid = cipher.organization_uuid;
        event.cipher_uuid = Some(cipher.uuid);
        event.user_uuid = cipher.user_uuid;
    } else {
        event.cipher_uuid = Some(cipher_uuid.to_string());
    }

    event.ip_address = Some(ip.to_string());
    event.act_user_uuid = Some(user_uuid.to_string());
    event.device_type = Some(device_type);
    event.save(&conn)
}
