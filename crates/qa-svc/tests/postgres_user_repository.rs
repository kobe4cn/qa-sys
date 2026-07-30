mod common;
#[path = "common/redis.rs"]
mod test_redis;

use anyhow::{Result, ensure};
use common::TestDatabase;
use qa_svc::{UserRepository, UserRepositoryImpl};
use test_redis::redis_pool;

#[tokio::test]
async fn test_should_persist_fetch_and_delete_users() -> Result<()> {
    let database = TestDatabase::create().await?;
    let result = async {
        let repository = UserRepositoryImpl::new(database.pool.clone(), redis_pool()?);

        ensure!(!repository.check_user_exist("alice".to_string()).await?);
        repository
            .add("alice".to_string(), "secret-password".to_string())
            .await?;
        repository
            .add("bob".to_string(), "another-password".to_string())
            .await?;

        ensure!(repository.check_user_exist("alice".to_string()).await?);
        let alice = repository.fetch_one("alice".to_string()).await?;
        ensure!(alice.username == "alice");
        ensure!(alice.password != "secret-password");
        ensure!(alice.openid.len() == 32);

        let users = repository
            .fetch_users(vec!["bob".to_string(), "alice".to_string()])
            .await?;
        ensure!(users.len() == 2);

        repository.delete("alice".to_string()).await?;
        ensure!(!repository.check_user_exist("alice".to_string()).await?);
        Ok(())
    }
    .await;
    database.cleanup().await?;
    result
}

#[tokio::test]
async fn test_should_return_empty_user_list_for_empty_input() -> Result<()> {
    let database = TestDatabase::create().await?;
    let result = async {
        let repository = UserRepositoryImpl::new(database.pool.clone(), redis_pool()?);
        let users = repository.fetch_users(Vec::new()).await?;

        ensure!(users.is_empty());
        Ok(())
    }
    .await;
    database.cleanup().await?;
    result
}

#[tokio::test]
async fn test_should_reject_duplicate_username() -> Result<()> {
    let database = TestDatabase::create().await?;
    let result = async {
        let repository = UserRepositoryImpl::new(database.pool.clone(), redis_pool()?);
        repository
            .add("alice".to_string(), "secret-password".to_string())
            .await?;
        let duplicate = repository
            .add("alice".to_string(), "different-password".to_string())
            .await;

        ensure!(duplicate.is_err());
        Ok(())
    }
    .await;
    database.cleanup().await?;
    result
}
