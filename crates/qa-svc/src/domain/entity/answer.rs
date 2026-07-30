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
use thiserror::Error;

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

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PaginationError {
    #[error("total must not be negative, got {0}")]
    NegativeTotal(i64),
    #[error("page size must be greater than zero, got {0}")]
    InvalidPageSize(i64),
    #[error("current page must be greater than zero, got {0}")]
    InvalidCurrentPage(i64),
}

impl LatestAnswerResponse {
    pub fn try_new(
        answers: Vec<AnswerEntity>,
        total: i64,
        page_size: i64,
        current_page: i64,
    ) -> Result<Self, PaginationError> {
        if total < 0 {
            return Err(PaginationError::NegativeTotal(total));
        }
        if page_size <= 0 {
            return Err(PaginationError::InvalidPageSize(page_size));
        }
        if current_page <= 0 {
            return Err(PaginationError::InvalidCurrentPage(current_page));
        }

        let total_page = total / page_size + i64::from(total % page_size != 0);
        let is_end = total_page == 0 || current_page >= total_page;
        Ok(Self {
            answers,
            total,
            total_page,
            page_size,
            current_page,
            is_end,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{AnswerEntity, LatestAnswerResponse, PaginationError};

    #[test]
    fn test_should_calculate_empty_pagination() -> Result<(), PaginationError> {
        let response = LatestAnswerResponse::try_new(Vec::new(), 0, 10, 1)?;

        assert_eq!(response.total_page, 0);
        assert!(response.is_end);
        Ok(())
    }

    #[test]
    fn test_should_calculate_exact_and_partial_pages() -> Result<(), PaginationError> {
        let exact = LatestAnswerResponse::try_new(Vec::new(), 20, 10, 1)?;
        assert_eq!(exact.total_page, 2);
        assert!(!exact.is_end);

        let partial = LatestAnswerResponse::try_new(Vec::new(), 21, 10, 1)?;
        assert_eq!(partial.total_page, 3);
        assert!(!partial.is_end);
        Ok(())
    }

    #[test]
    fn test_should_mark_last_and_later_pages_as_end() -> Result<(), PaginationError> {
        let last_page = LatestAnswerResponse::try_new(vec![AnswerEntity::default()], 20, 10, 2)?;
        assert!(last_page.is_end);

        let later_page = LatestAnswerResponse::try_new(Vec::new(), 20, 10, 3)?;
        assert!(later_page.is_end);
        Ok(())
    }

    #[test]
    fn test_should_reject_zero_page_size() {
        assert!(LatestAnswerResponse::try_new(Vec::new(), 10, 0, 1).is_err());
    }

    #[test]
    fn test_should_reject_negative_total() {
        assert!(matches!(
            LatestAnswerResponse::try_new(Vec::new(), -1, 10, 1),
            Err(PaginationError::NegativeTotal(-1))
        ));
    }

    #[test]
    fn test_should_reject_non_positive_current_page() {
        assert!(matches!(
            LatestAnswerResponse::try_new(Vec::new(), 10, 10, 0),
            Err(PaginationError::InvalidCurrentPage(0))
        ));
    }
}
