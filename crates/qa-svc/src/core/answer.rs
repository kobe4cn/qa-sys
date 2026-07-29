use anyhow::Result;
use chrono::Local;

use sqlx::PgPool;
use tracing::info;

use crate::{AnswerEntity, AnswerRepository, LatestAnswerResponse};

pub struct AnswerRepositoryImpl {
    pg: PgPool,
}

impl AnswerRepositoryImpl {
    pub fn new(pg: PgPool) -> Self {
        Self { pg }
    }
}

#[async_trait::async_trait]
impl AnswerRepository for AnswerRepositoryImpl {
    async fn check_answer_exist(&self, id: u64, username: String) -> Result<bool> {
        let sql = format!(
            "select id from {} where question_id=$1 and created_by=$2",
            AnswerEntity::table_name()
        );
        let result: Option<i64> = sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
            .bind(id as i64)
            .bind(username)
            .fetch_optional(&self.pg)
            .await?;
        Ok(result.is_some())
    }
    /*/
    CREATE TABLE answers (
        id bigserial PRIMARY KEY,
        question_id bigint NOT NULL DEFAULT 0,
        content text NOT NULL,
        created_by varchar(50) NOT NULL DEFAULT '',
        updated_by varchar(50) NOT NULL DEFAULT '',
        created_at timestamp NOT NULL,
        updated_at timestamp DEFAULT NULL,
        agree_count bigint NOT NULL DEFAULT 0
    );
     */
    async fn add(&self, answer: AnswerEntity) -> Result<u64> {
        let sql = format!(
            "insert into {} (question_id,content,created_by,created_at) values ($1,$2,$3,$4) returning id",
            AnswerEntity::table_name()
        );
        let result: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
            .bind(answer.question_id)
            .bind(answer.content)
            .bind(answer.created_by)
            .bind(answer.updated_by)
            .bind(answer.created_at)
            .bind(answer.updated_at)
            .bind(answer.agree_count)
            .fetch_one(&self.pg)
            .await?;
        Ok(result as u64)
    }

    async fn update(&self, id: u64, content: String, updated_by: String) -> Result<()> {
        let sql = format!(
            "update {} set content=$1,updated_by=$2,updated_at=$3 where id=$4",
            AnswerEntity::table_name()
        );
        let res = sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(content)
            .bind(updated_by)
            .bind(Local::now().format("%Y-%m-%d %H:%M:%S").to_string())
            .bind(id as i64)
            .execute(&self.pg)
            .await?;
        info!("update answer rows_affected: {}", res.rows_affected());
        Ok(())
    }

    async fn delete(&self, id: u64, username: String) -> Result<()> {
        let sql = format!(
            "delete from {} where id=$1 and created_by=$2",
            AnswerEntity::table_name()
        );
        let res = sqlx::query(sqlx::AssertSqlSafe(sql))
            .bind(id as i64)
            .bind(username)
            .execute(&self.pg)
            .await?;
        info!("delete answer rows_affected: {}", res.rows_affected());
        Ok(())
    }

    async fn find_one(&self, id: u64) -> Result<AnswerEntity> {
        let sql = format!(
            "select * from {} where id=$1 limit 1",
            AnswerEntity::table_name()
        );
        let res = sqlx::query_as::<_, AnswerEntity>(sqlx::AssertSqlSafe(sql))
            .bind(id as i64)
            .fetch_one(&self.pg)
            .await?;
        Ok(res)
    }

    async fn find_latest(
        &self,
        question_id: u64,
        page: u64,
        limit: u64,
        sort: String,
    ) -> Result<LatestAnswerResponse> {
        let countsql = format!(
            "select count(*) from {} where question_id=$1",
            AnswerEntity::table_name()
        );
        let total = sqlx::query_scalar::<_, i64>(sqlx::AssertSqlSafe(countsql))
            .bind(question_id as i64)
            .fetch_one(&self.pg)
            .await?;

        let sql = format!(
            "select * from {} where question_id=$1 order by $2 desc limit $3 offset $4",
            AnswerEntity::table_name()
        );
        let res = sqlx::query_as::<_, AnswerEntity>(sqlx::AssertSqlSafe(sql))
            .bind(question_id as i64)
            .bind(sort)
            .bind(limit as i64)
            .bind((page - 1) as i64 * limit as i64)
            .fetch_all(&self.pg)
            .await?;
        Ok(LatestAnswerResponse::new(
            res,
            total,
            limit as i64,
            page as i64,
        ))
    }
}
