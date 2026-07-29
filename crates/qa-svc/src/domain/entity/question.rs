/*
CREATE TABLE questions (
    id bigserial PRIMARY KEY,
    title varchar(300) NOT NULL DEFAULT '',
    content text NOT NULL,
    created_by varchar(50) NOT NULL DEFAULT '',
    updated_by varchar(50) NOT NULL DEFAULT '',
    created_at timestamp NOT NULL,
    updated_at timestamp DEFAULT NULL,
    read_count bigint NOT NULL DEFAULT 0,
    reply_count bigint NOT NULL DEFAULT 0
);
*/

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;

const QUESTION_TABLE: &str = "questions";

#[derive(Debug, Default, Serialize, Deserialize, FromRow)]
pub struct QuestionEntity {
    pub id: i64,
    pub title: String,
    pub content: String,
    pub created_by: String,
    pub updated_by: String,
    pub created_at: NaiveDateTime,
    pub updated_at: Option<NaiveDateTime>,
    pub read_count: i64,
    pub reply_count: i64,
}

pub struct LatestQuestionResponse {
    pub questions: Vec<QuestionEntity>,
    pub last_id: Option<i64>,
    pub is_end: bool,
}
impl QuestionEntity {
    pub fn table_name() -> String {
        QUESTION_TABLE.to_string()
    }
}
