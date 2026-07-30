mod common;
#[path = "common/pulsar.rs"]
mod test_pulsar;

use anyhow::{Result, ensure};
use chrono::Local;
use common::TestDatabase;
use qa_svc::{UserVoteRepository, UserVoteRepositoryImpl, config::xpulsar::VoteMessagingConfig};
use test_pulsar::pulsar_client;

fn vote_messaging_config() -> VoteMessagingConfig {
    VoteMessagingConfig {
        topic: "non-persistent://public/default/qa-sys-postgres-vote-test".to_string(),
        subscription: "qa-sys-postgres-vote-test".to_string(),
        producer_name: "qa-sys-postgres-vote-test".to_string(),
        consumer_name: "qa-sys-postgres-vote-test".to_string(),
    }
}

async fn insert_vote(
    database: &TestDatabase,
    target_id: i64,
    username: &str,
    target_type: &str,
) -> Result<()> {
    sqlx::query(
        "INSERT INTO users_votes (target_id, target_type, created_by, created_at) VALUES ($1, $2, \
         $3, $4)",
    )
    .bind(target_id)
    .bind(target_type)
    .bind(username)
    .bind(Local::now().naive_local())
    .execute(&database.pool)
    .await?;
    Ok(())
}

#[tokio::test]
async fn test_should_check_vote_by_target_user_and_type() -> Result<()> {
    let database = TestDatabase::create().await?;
    let result = async {
        insert_vote(&database, 42, "alice", "answer").await?;
        let repository = UserVoteRepositoryImpl::new(
            database.pool.clone(),
            pulsar_client().await?,
            vote_messaging_config(),
        );

        ensure!(
            repository
                .is_voted(42, "alice".to_string(), "answer".to_string())
                .await?
        );
        ensure!(
            !repository
                .is_voted(42, "bob".to_string(), "answer".to_string())
                .await?
        );
        ensure!(
            !repository
                .is_voted(42, "alice".to_string(), "question".to_string())
                .await?
        );
        Ok(())
    }
    .await;
    database.cleanup().await?;
    result
}

#[tokio::test]
async fn test_should_check_batch_votes() -> Result<()> {
    let database = TestDatabase::create().await?;
    let result = async {
        insert_vote(&database, 41, "alice", "answer").await?;
        insert_vote(&database, 43, "alice", "answer").await?;
        insert_vote(&database, 42, "bob", "answer").await?;
        let repository = UserVoteRepositoryImpl::new(
            database.pool.clone(),
            pulsar_client().await?,
            vote_messaging_config(),
        );

        let votes = repository
            .is_batch_voted(vec![41, 42, 43], "alice".to_string(), "answer".to_string())
            .await?;
        ensure!(votes.get(&41) == Some(&true));
        ensure!(!votes.contains_key(&42));
        ensure!(votes.get(&43) == Some(&true));
        Ok(())
    }
    .await;
    database.cleanup().await?;
    result
}

#[tokio::test]
async fn test_should_return_empty_vote_map_for_empty_target_list() -> Result<()> {
    let database = TestDatabase::create().await?;
    let result = async {
        let repository = UserVoteRepositoryImpl::new(
            database.pool.clone(),
            pulsar_client().await?,
            vote_messaging_config(),
        );
        let votes = repository
            .is_batch_voted(Vec::new(), "alice".to_string(), "answer".to_string())
            .await?;

        ensure!(votes.is_empty());
        Ok(())
    }
    .await;
    database.cleanup().await?;
    result
}
