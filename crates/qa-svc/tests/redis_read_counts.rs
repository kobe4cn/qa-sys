mod common;
#[path = "common/redis.rs"]
mod test_redis;

use anyhow::{Result, ensure};
use chrono::Local;
use common::TestDatabase;
use qa_svc::{QuestionEntity, QuestionRepository, QuestionRepositoryImpl};
use qa_sys_core::RedisPool;
use redis::Commands;
use test_redis::redis_pool;

const READ_COUNT_HASH: &str = "qa-sys:question:read_count:hash";

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

fn hash_value(pool: &RedisPool, field: &str) -> Result<i64> {
    match pool {
        RedisPool::Single(pool) => {
            let mut connection = pool.get()?;
            Ok(connection.hget(READ_COUNT_HASH, field)?)
        }
        RedisPool::Cluster(pool) => {
            let mut connection = pool.get()?;
            Ok(connection.hget(READ_COUNT_HASH, field)?)
        }
    }
}

#[tokio::test]
async fn test_should_increment_and_flush_question_read_counts() -> Result<()> {
    let database = TestDatabase::create().await?;
    let redis = redis_pool()?;
    clear_database(&redis)?;
    let result = async {
        let repository = QuestionRepositoryImpl::new(database.pool.clone(), redis.clone());
        let question_id = repository
            .add(QuestionEntity {
                title: "Redis read count".to_string(),
                content: "content".to_string(),
                created_by: "alice".to_string(),
                created_at: Local::now().naive_local(),
                ..Default::default()
            })
            .await?;

        ensure!(repository.incr(question_id, "question".to_string()).await? == 1);
        ensure!(repository.incr(question_id, "question".to_string()).await? == 2);
        ensure!(repository.find_one(question_id).await?.read_count == 0);
        ensure!(hash_value(&redis, &question_id.to_string())? == 2);

        repository.handler("question".to_string()).await?;
        ensure!(repository.find_one(question_id).await?.read_count == 2);
        ensure!(hash_value(&redis, &question_id.to_string())? == 0);

        repository.handler("question".to_string()).await?;
        ensure!(repository.find_one(question_id).await?.read_count == 2);
        Ok(())
    }
    .await;
    clear_database(&redis)?;
    database.cleanup().await?;
    result
}
