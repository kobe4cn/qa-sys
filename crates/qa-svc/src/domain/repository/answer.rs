use anyhow::Result;

use crate::{LatestAnswerResponse, domain::entity::AnswerEntity};

#[async_trait::async_trait]
pub trait AnswerRepository: Send + Sync + 'static {
    async fn check_answer_exist(&self, id: u64, username: String) -> Result<bool>;
    async fn add(&self, answer: AnswerEntity) -> Result<u64>;
    async fn update(&self, id: u64, content: String, updated_by: String) -> Result<()>;
    async fn delete(&self, id: u64, username: String) -> Result<()>;
    async fn find_one(&self, id: u64) -> Result<AnswerEntity>;
    async fn find_latest(
        &self,
        question_id: u64,
        limit: u64,
        current_page: u64,
    ) -> Result<LatestAnswerResponse>;
}
