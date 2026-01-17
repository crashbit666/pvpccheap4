use chrono::{NaiveDateTime, NaiveTime};
use diesel::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Queryable, Selectable, Insertable, Serialize, Deserialize, Debug, Clone)]
#[diesel(table_name = crate::schema::prices)]
pub struct Price {
    pub timestamp: NaiveDateTime,
    pub price: f64,
    pub source: String,
}

#[derive(Queryable, Selectable, Insertable, Serialize, Deserialize, Debug, Clone)]
#[diesel(table_name = crate::schema::users)]
pub struct User {
    pub id: i32,
    pub username: String,
    pub password_hash: String,
    pub created_at: NaiveDateTime,
}

#[derive(Queryable, Selectable, Insertable, Serialize, Deserialize, Debug, Clone)]
#[diesel(table_name = crate::schema::user_integrations)]
pub struct UserIntegration {
    pub id: i32,
    pub user_id: i32,
    pub provider_name: String,
    pub credentials_json: String,
    pub is_active: bool,
    pub created_at: NaiveDateTime,
}

#[derive(Queryable, Selectable, Insertable, Serialize, Deserialize, Debug, Clone)]
#[diesel(table_name = crate::schema::devices)]
pub struct Device {
    pub id: i32,
    pub integration_id: i32,
    pub external_id: String,
    pub name: String,
    pub device_type: String,
    pub is_managed: bool,
}

#[derive(Queryable, Selectable, Insertable, Serialize, Deserialize, Debug, Clone)]
#[diesel(table_name = crate::schema::schedules)]
pub struct Schedule {
    pub id: i32,
    pub device_id: i32,
    pub user_id: i32,
    pub duration_minutes: i32,
    pub window_start: NaiveTime,
    pub window_end: NaiveTime,
    pub created_at: NaiveDateTime,
}
