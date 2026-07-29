use anyhow::Result;

use crate::domain::entity::{UserEntity, UserSessionEntity};

#[async_trait::async_trait]
pub trait UserRepository: Send + Sync + 'static {
    async fn check_user_exist(&self, username: String) -> Result<bool>;
    async fn add(&self, username: String, password: String) -> Result<()>;
    async fn fetch_one(&self, username: String) -> Result<UserEntity>;
    async fn fetch_users(&self, usernames: Vec<String>) -> Result<Vec<UserEntity>>;
    async fn delete(&self, username: String) -> Result<()>;

    //use session
    async fn get(&self, key: String) -> Result<UserSessionEntity>;
    async fn set(&self, key: String, value: UserSessionEntity, second: u64) -> Result<()>;
    async fn del(&self, key: String) -> Result<()>;
}
