mod common;

use anyhow::{Result, ensure};
use chrono::Local;
use common::TestDatabase;
use qa_svc::{AnswerEntity, AnswerRepository, AnswerRepositoryImpl};

fn answer(question_id: i64, created_by: &str, content: &str) -> AnswerEntity {
    AnswerEntity {
        question_id,
        content: content.to_string(),
        created_by: created_by.to_string(),
        created_at: Local::now().naive_local(),
        ..Default::default()
    }
}

#[tokio::test]
async fn test_should_add_find_update_and_delete_answer() -> Result<()> {
    let database = TestDatabase::create().await?;
    let result = async {
        let repository = AnswerRepositoryImpl::new(database.pool.clone());
        let id = repository.add(answer(42, "alice", "original")).await?;

        let inserted = repository.find_one(id).await?;
        ensure!(inserted.question_id == 42);
        ensure!(inserted.content == "original");
        ensure!(inserted.created_by == "alice");

        repository
            .update(id, "updated".to_string(), "alice".to_string())
            .await?;
        ensure!(repository.find_one(id).await?.content == "updated");

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
async fn test_should_check_answer_ownership_by_answer_id() -> Result<()> {
    let database = TestDatabase::create().await?;
    let result = async {
        let repository = AnswerRepositoryImpl::new(database.pool.clone());
        let id: i64 = sqlx::query_scalar(
            "INSERT INTO answers (question_id, content, created_by, created_at) VALUES ($1, $2, \
             $3, $4) RETURNING id",
        )
        .bind(999_i64)
        .bind("answer")
        .bind("alice")
        .bind(Local::now().naive_local())
        .fetch_one(&database.pool)
        .await?;

        ensure!(
            repository
                .check_answer_exist(id as u64, "alice".to_string())
                .await?
        );
        ensure!(
            !repository
                .check_answer_exist(id as u64, "bob".to_string())
                .await?
        );
        Ok(())
    }
    .await;
    database.cleanup().await?;
    result
}

#[tokio::test]
async fn test_should_paginate_answers_in_requested_order() -> Result<()> {
    let database = TestDatabase::create().await?;
    let result = async {
        let repository = AnswerRepositoryImpl::new(database.pool.clone());
        for content in ["first", "second", "third"] {
            sqlx::query(
                "INSERT INTO answers (question_id, content, created_by, created_at) VALUES ($1, \
                 $2, $3, $4)",
            )
            .bind(42_i64)
            .bind(content)
            .bind("alice")
            .bind(Local::now().naive_local())
            .execute(&database.pool)
            .await?;
        }

        let first_page = repository.find_latest(42, 2, 1).await?;
        ensure!(first_page.answers.len() == 2);
        ensure!(first_page.answers[0].content == "third");
        ensure!(first_page.answers[1].content == "second");
        ensure!(first_page.total == 3);
        ensure!(first_page.total_page == 2);
        ensure!(!first_page.is_end);

        let second_page = repository.find_latest(42, 2, 2).await?;
        ensure!(second_page.answers.len() == 1);
        ensure!(second_page.answers[0].content == "first");
        ensure!(second_page.is_end);
        Ok(())
    }
    .await;
    database.cleanup().await?;
    result
}
