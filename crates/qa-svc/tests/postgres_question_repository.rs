mod common;
#[path = "common/redis.rs"]
mod test_redis;

use anyhow::{Result, ensure};
use chrono::Local;
use common::TestDatabase;
use qa_svc::{QuestionEntity, QuestionRepository, QuestionRepositoryImpl};
use test_redis::redis_pool;

fn question(created_by: &str, title: &str) -> QuestionEntity {
    QuestionEntity {
        title: title.to_string(),
        content: format!("{title} content"),
        created_by: created_by.to_string(),
        created_at: Local::now().naive_local(),
        ..Default::default()
    }
}

#[tokio::test]
async fn test_should_add_find_and_update_question() -> Result<()> {
    let database = TestDatabase::create().await?;
    let result = async {
        let repository = QuestionRepositoryImpl::new(database.pool.clone(), redis_pool()?);
        let id = repository.add(question("alice", "original")).await?;
        let inserted = repository.find_one(id).await?;
        ensure!(inserted.title == "original");
        ensure!(inserted.created_by == "alice");

        let mut updated = question("alice", "updated");
        updated.updated_by = "alice".to_string();
        updated.updated_at = Some(Local::now().naive_local());
        repository.update(id, updated).await?;

        let stored = repository.find_one(id).await?;
        ensure!(stored.title == "updated");
        ensure!(stored.content == "updated content");
        ensure!(stored.updated_by == "alice");
        Ok(())
    }
    .await;
    database.cleanup().await?;
    result
}

#[tokio::test]
async fn test_should_delete_question_only_for_owner() -> Result<()> {
    let database = TestDatabase::create().await?;
    let result = async {
        let repository = QuestionRepositoryImpl::new(database.pool.clone(), redis_pool()?);
        let id = repository.add(question("alice", "owned")).await?;

        repository.delete(id, "bob".to_string()).await?;
        ensure!(repository.find_one(id).await.is_ok());

        repository.delete(id, "alice".to_string()).await?;
        ensure!(repository.find_one(id).await.is_err());
        Ok(())
    }
    .await;
    database.cleanup().await?;
    result
}

#[tokio::test]
async fn test_should_find_latest_questions_by_cursor_and_limit() -> Result<()> {
    let database = TestDatabase::create().await?;
    let result = async {
        let repository = QuestionRepositoryImpl::new(database.pool.clone(), redis_pool()?);
        let first = repository.add(question("alice", "first")).await?;
        let second = repository.add(question("alice", "second")).await?;
        let third = repository.add(question("alice", "third")).await?;

        let first_page = repository.find_latest(0, 2).await?;
        ensure!(first_page.questions.len() == 2);
        ensure!(first_page.questions[0].id == third as i64);
        ensure!(first_page.questions[1].id == second as i64);
        ensure!(first_page.last_id == Some(second as i64));
        ensure!(!first_page.is_end);

        let second_page = repository.find_latest(second, 2).await?;
        ensure!(second_page.questions.len() == 1);
        ensure!(second_page.questions[0].id == first as i64);
        ensure!(second_page.is_end);
        Ok(())
    }
    .await;
    database.cleanup().await?;
    result
}
