use std::collections::HashMap;

use anyhow::Result;
use tokio::sync::watch;

use crate::domain::entity::VoteMessage;

#[async_trait::async_trait]
pub trait UserVoteRepository: Send + Sync + 'static {
    async fn is_voted(&self, target_id: u64, username: String, target_type: String)
    -> Result<bool>;
    async fn is_batch_voted(
        &self,
        target_ids: Vec<u64>,
        username: String,
        target_type: String,
    ) -> Result<HashMap<u64, bool>>;
    async fn publish(&self, msg: VoteMessage) -> Result<bool>;
    async fn consumer(&self, target_type: String, mut receive: watch::Receiver<bool>)
    -> Result<()>;
}
