/*/CREATE TABLE answers (
    id bigserial PRIMARY KEY,
    question_id bigint NOT NULL DEFAULT 0,
    content text NOT NULL,
    created_by varchar(50) NOT NULL DEFAULT '',
    updated_by varchar(50) NOT NULL DEFAULT '',
    created_at timestamp NOT NULL,
    updated_at timestamp DEFAULT NULL,
    agree_count bigint NOT NULL DEFAULT 0
); */

use chrono::NaiveDateTime;
use serde::{Deserialize, Serialize};
use sqlx::prelude::FromRow;

const ANSWER_TABLE: &str = "answers";

#[derive(Default, Debug, Serialize, Deserialize, FromRow)]
pub struct AnswerEntity {
    pub id: i64,
    pub question_id: i64,
    pub content: String,
    pub created_by: String,
    pub updated_by: Option<String>,
    pub created_at: NaiveDateTime,
    pub updated_at: Option<NaiveDateTime>,
    pub agree_count: i64,
}
impl AnswerEntity {
    pub fn table_name() -> String {
        ANSWER_TABLE.to_string()
    }
}

#[derive(Default, Debug, Serialize, Deserialize)]
pub struct LatestAnswerResponse {
    pub answers: Vec<AnswerEntity>,
    pub total: i64,
    pub total_page: i64,
    pub page_size: i64,
    pub current_page: i64,
    pub is_end: bool,
}

impl LatestAnswerResponse {
    pub fn new(answers: Vec<AnswerEntity>, total: i64, page_size: i64, current_page: i64) -> Self {
        let total_page = (total as f64 / page_size as f64).ceil() as i64;
        let is_end = current_page >= total_page;
        Self {
            answers,
            total,
            total_page,
            page_size,
            current_page,
            is_end,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{AnswerEntity, LatestAnswerResponse};

    #[test]
    fn test_should_calculate_empty_pagination() {
        let response = LatestAnswerResponse::new(Vec::new(), 0, 10, 1);

        assert_eq!(response.total_page, 0);
        assert!(response.is_end);
    }

    #[test]
    fn test_should_calculate_exact_and_partial_pages() {
        let exact = LatestAnswerResponse::new(Vec::new(), 20, 10, 1);
        assert_eq!(exact.total_page, 2);
        assert!(!exact.is_end);

        let partial = LatestAnswerResponse::new(Vec::new(), 21, 10, 1);
        assert_eq!(partial.total_page, 3);
        assert!(!partial.is_end);
    }

    #[test]
    fn test_should_mark_last_and_later_pages_as_end() {
        let last_page = LatestAnswerResponse::new(vec![AnswerEntity::default()], 20, 10, 2);
        assert!(last_page.is_end);

        let later_page = LatestAnswerResponse::new(Vec::new(), 20, 10, 3);
        assert!(later_page.is_end);
    }

    #[test]
    fn test_should_reject_zero_page_size() {
        assert!(LatestAnswerResponse::try_new(Vec::new(), 10, 0, 1).is_err());
    }
}
