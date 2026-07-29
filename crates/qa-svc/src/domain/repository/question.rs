use crate::{LatestQuestionResponse, domain::entity::QuestionEntity};
use anyhow::Result;

#[async_trait::async_trait]
pub trait QuestionRepository: Send + Sync + 'static {
    async fn add(&self, question: QuestionEntity) -> Result<u64>;
    async fn update(&self, id: u64, question: QuestionEntity) -> Result<()>;
    async fn delete(&self, id: u64, username: String) -> Result<()>;
    async fn find_one(&self, id: u64) -> Result<QuestionEntity>;
    async fn find_latest(&self, last_id: u64, limit: u64) -> Result<LatestQuestionResponse>;

    //read_count with redis
    async fn incr(&self, target_id: u64,target_type: String) -> Result<u64>;

    //read_count with pgsql
    async fn handler(&self, target_type: String) -> Result<()>;
}
