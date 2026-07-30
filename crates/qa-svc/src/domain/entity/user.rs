use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;

/*
 * CREATE TABLE users (
    id bigserial PRIMARY KEY,
    username varchar(50) NOT NULL,
    password varchar(50) NOT NULL,
    nick varchar(100) NOT NULL DEFAULT '',
    openid varchar(32) NOT NULL DEFAULT '',
    created_at timestamp NOT NULL,
    updated_at timestamp DEFAULT NULL,
    CONSTRAINT uk_users_username UNIQUE (username)
);
*/
const USER_TABLE: &str = "users";

#[derive(Default, Debug, Serialize, Deserialize, FromRow)]
pub struct UserEntity {
    pub id: i64,
    pub username: String,
    pub password: String,
    pub nick: String,
    pub openid: String,
    pub created_at: NaiveDateTime,
    pub updated_at: Option<NaiveDateTime>,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct UserSessionEntity {
    pub uid: i64,
    pub username: String,
    pub openid: String,
    pub login_time: String,
    pub expire_time: String,
}

impl UserEntity {
    pub fn table_name() -> String {
        USER_TABLE.to_string()
    }
}
