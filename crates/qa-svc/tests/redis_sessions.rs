mod common;
#[path = "common/redis.rs"]
mod test_redis;

use std::time::Duration;

use anyhow::{Result, ensure};
use common::TestDatabase;
use qa_svc::{UserRepository, UserRepositoryImpl, UserSessionEntity};
use qa_sys_core::RedisPool;
use redis::Commands;
use test_redis::redis_pool;

fn clear_database(pool: &RedisPool) -> Result<()> {
    match pool {
        RedisPool::Single(pool) => {
            let mut connection = pool.get()?;
            redis::cmd("FLUSHDB").query::<()>(&mut *connection)?;
        }
        RedisPool::Cluster(pool) => {
            let mut connection = pool.get()?;
            redis::cmd("FLUSHDB").query::<()>(&mut *connection)?;
        }
    }
    Ok(())
}

fn set_raw_value(pool: &RedisPool, key: &str, value: &str) -> Result<()> {
    match pool {
        RedisPool::Single(pool) => {
            let mut connection = pool.get()?;
            connection.set::<_, _, ()>(key, value)?;
        }
        RedisPool::Cluster(pool) => {
            let mut connection = pool.get()?;
            connection.set::<_, _, ()>(key, value)?;
        }
    }
    Ok(())
}

fn ttl(pool: &RedisPool, key: &str) -> Result<i64> {
    match pool {
        RedisPool::Single(pool) => {
            let mut connection = pool.get()?;
            Ok(connection.ttl(key)?)
        }
        RedisPool::Cluster(pool) => {
            let mut connection = pool.get()?;
            Ok(connection.ttl(key)?)
        }
    }
}

fn session(username: &str) -> UserSessionEntity {
    UserSessionEntity {
        uid: 7,
        username: username.to_string(),
        openid: "openid-7".to_string(),
        login_time: "2026-07-29 10:00:00".to_string(),
        expire_time: "2026-07-30 10:00:00".to_string(),
    }
}

#[tokio::test]
async fn test_should_manage_session_lifecycle_ttl_and_invalid_data() -> Result<()> {
    let database = TestDatabase::create().await?;
    let redis = redis_pool()?;
    clear_database(&redis)?;
    let result = async {
        let repository = UserRepositoryImpl::new(database.pool.clone(), redis.clone());

        repository
            .set("session:alice".to_string(), session("alice"), 30)
            .await?;
        let stored = repository.get("session:alice".to_string()).await?;
        ensure!(stored.uid == 7);
        ensure!(stored.username == "alice");
        ensure!((1..=30).contains(&ttl(&redis, "session:alice")?));

        repository.del("session:alice".to_string()).await?;
        ensure!(repository.get("session:alice".to_string()).await.is_err());

        repository
            .set("session:short".to_string(), session("short"), 1)
            .await?;
        tokio::time::sleep(Duration::from_millis(1_100)).await;
        ensure!(repository.get("session:short".to_string()).await.is_err());

        set_raw_value(&redis, "session:broken", "{not-json")?;
        ensure!(repository.get("session:broken".to_string()).await.is_err());
        Ok(())
    }
    .await;
    clear_database(&redis)?;
    database.cleanup().await?;
    result
}
