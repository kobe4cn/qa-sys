mod common;
#[path = "common/pulsar.rs"]
mod test_pulsar;

use std::{sync::Arc, time::Duration};

use anyhow::{Context, Result, ensure};
use chrono::Local;
use common::TestDatabase;
use qa_svc::{
    UserVoteRepository, UserVoteRepositoryImpl, VoteMessage, config::xpulsar::VoteMessagingConfig,
};
use test_pulsar::pulsar_client;
use tokio::{sync::watch, time::timeout};
use uuid::Uuid;

fn messaging_config() -> VoteMessagingConfig {
    let suffix = Uuid::new_v4().simple().to_string();
    VoteMessagingConfig {
        topic: format!("non-persistent://public/default/qa-sys-vote-test-{suffix}"),
        subscription: format!("qa-sys-vote-test-{suffix}"),
        producer_name: format!("qa-sys-vote-test-{suffix}"),
        consumer_name: format!("qa-sys-vote-test-{suffix}"),
    }
}

async fn wait_for_vote_state(
    database: &TestDatabase,
    answer_id: i64,
    expected_agree_count: i64,
    expected_vote_count: i64,
) -> Result<()> {
    timeout(Duration::from_secs(10), async {
        loop {
            let agree_count: i64 =
                sqlx::query_scalar("SELECT agree_count FROM answers WHERE id = $1")
                    .bind(answer_id)
                    .fetch_one(&database.pool)
                    .await?;
            let vote_count: i64 = sqlx::query_scalar(
                "SELECT COUNT(*) FROM users_votes WHERE target_id = $1 AND target_type = $2",
            )
            .bind(answer_id)
            .bind("answer")
            .fetch_one(&database.pool)
            .await?;
            if agree_count == expected_agree_count && vote_count == expected_vote_count {
                return Ok(());
            }
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    })
    .await
    .context("vote message did not reach the expected state within 10 seconds")?
}

#[tokio::test]
async fn test_should_publish_consume_apply_and_cancel_answer_vote() -> Result<()> {
    let database = TestDatabase::create().await?;
    let answer_id: i64 = sqlx::query_scalar(
        "INSERT INTO answers (question_id, content, created_by, created_at) VALUES ($1, $2, $3, \
         $4) RETURNING id",
    )
    .bind(42_i64)
    .bind("answer")
    .bind("bob")
    .bind(Local::now().naive_local())
    .fetch_one(&database.pool)
    .await?;
    let repository = Arc::new(UserVoteRepositoryImpl::new(
        database.pool.clone(),
        pulsar_client().await?,
        messaging_config(),
    ));
    let (shutdown_sender, shutdown_receiver) = watch::channel(false);
    let consumer_repository = Arc::clone(&repository);
    let consumer = tokio::spawn(async move {
        consumer_repository
            .consumer("answer".to_string(), shutdown_receiver)
            .await
    });

    let result = async {
        tokio::time::sleep(Duration::from_millis(500)).await;
        ensure!(
            repository
                .publish(VoteMessage {
                    target_id: answer_id,
                    target_type: "answer".to_string(),
                    created_by: "alice".to_string(),
                    action: "up".to_string(),
                })
                .await?
        );
        wait_for_vote_state(&database, answer_id, 1, 1).await?;

        ensure!(
            repository
                .publish(VoteMessage {
                    target_id: answer_id,
                    target_type: "answer".to_string(),
                    created_by: "alice".to_string(),
                    action: "down".to_string(),
                })
                .await?
        );
        wait_for_vote_state(&database, answer_id, 0, 0).await
    }
    .await;

    drop(shutdown_sender);
    timeout(Duration::from_secs(5), consumer)
        .await
        .context("vote consumer did not shut down within 5 seconds")?
        .context("vote consumer task panicked")??;
    database.cleanup().await?;
    result
}
