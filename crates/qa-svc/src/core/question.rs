use crate::{LatestQuestionResponse, QuestionEntity, QuestionRepository};
use anyhow::{Result, anyhow};
use qa_sys_core::RedisPool;
use redis::Commands;
use sqlx::PgPool;
use tracing::info;

pub struct QuestionRepositoryImpl {
    pg: PgPool,
    redis: RedisPool,
}

impl QuestionRepositoryImpl {
    pub fn new(pg: PgPool, redis: RedisPool) -> Self {
        Self { pg, redis }
    }
    fn get_hash_key(&self, target_type: String) -> String {
        format!("qa-sys:{}:read_count:hash", target_type)
    }

    async fn update_read_count(
        &self,
        target_type: String,
        target_id: i64,
        icreament: i64,
    ) -> Result<()> {
        let sql = format!(
            "update {} set read_count=read_count+$1 where id=$2",
            QuestionEntity::table_name()
        );
        let res = sqlx::query(sqlx::AssertSqlSafe(&*sql))
            .bind(icreament)
            .bind(target_id)
            .execute(&self.pg)
            .await?;
        if res.rows_affected() == 0 {
            return Err(anyhow!(
                "failed to update target_id:{} target_type:{}",
                target_id,
                target_type
            ));
        }
        let hash_key = self.get_hash_key(target_type);
        let field_key = target_id.to_string();
        match &self.redis {
            RedisPool::Single(pool) => {
                let mut conn = pool.get()?;
                let _remain: i64 = conn.hincr(&hash_key, &field_key, -icreament)?;
            }
            RedisPool::Cluster(pool) => {
                let mut conn = pool.get()?;
                let _remain: i64 = conn.hincr(&hash_key, &field_key, -icreament)?;
            }
        }

        Ok(())
    }

    async fn handler_redis_topg(&self, target_type: String) -> Result<i64> {
        let mut cursor: u64 = 0;
        let pattern = "*";
        let count = 500;
        loop {
            match &self.redis {
                RedisPool::Single(pool) => {
                    let mut conn = pool.get()?;
                    cursor = self
                        .reids_cmd(
                            &mut conn,
                            target_type.clone(),
                            cursor,
                            pattern.to_string(),
                            count,
                        )
                        .await?;
                    if cursor == 0 {
                        break;
                    }
                }
                RedisPool::Cluster(pool) => {
                    let mut conn = pool.get()?;
                    cursor = self
                        .reids_cmd(
                            &mut conn,
                            target_type.clone(),
                            cursor,
                            pattern.to_string(),
                            count,
                        )
                        .await?;
                    if cursor == 0 {
                        break;
                    }
                }
            }
        }
        Ok(0)
    }

    async fn reids_cmd(
        &self,
        conn: &mut impl redis::ConnectionLike,
        target_type: String,
        cursor: u64,
        pattern: String,
        count: u64,
    ) -> Result<u64> {
        let hash_key = self.get_hash_key(target_type.clone());
        let (next_cursor, replies): (u64, Vec<(String, i64)>) = redis::cmd("HSCAN")
            .arg(&hash_key)
            .arg(cursor)
            .arg("MATCH")
            .arg(pattern)
            .arg("COUNT")
            .arg(count)
            .query(conn)?;
        for (field, value) in replies.iter() {
            let target_id = field.parse::<i64>()?;
            let icreament = *value;
            let res = self
                .update_read_count(target_type.clone(), target_id, icreament)
                .await;
            if res.is_err() {
                return Err(anyhow!(
                    "failed to update read count,target_type:{},target_id:{}",
                    target_type,
                    target_id
                ));
            }
            info!(
                "update read count,target_type:{},target_id:{},icreament:{}",
                target_type, target_id, icreament
            );
        }
        Ok(next_cursor)
    }
}
/*/
CREATE TABLE questions (
    id bigserial PRIMARY KEY,
    title varchar(300) NOT NULL DEFAULT '',
    content text NOT NULL,
    created_by varchar(50) NOT NULL DEFAULT '',
    updated_by varchar(50) NOT NULL DEFAULT '',
    created_at timestamp NOT NULL,
    updated_at timestamp DEFAULT NULL,
    read_count bigint NOT NULL DEFAULT 0,
    reply_count bigint NOT NULL DEFAULT 0
);
*/

#[async_trait::async_trait]
impl QuestionRepository for QuestionRepositoryImpl {
    async fn add(&self, question: QuestionEntity) -> Result<u64> {
        let sql = format!(
            "INSERT INTO {} (title, content, created_by, created_at) VALUES ($1, $2, $3, $4) RETURNING id",
            QuestionEntity::table_name()
        );
        let id: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(&*sql))
            .bind(question.title)
            .bind(question.content)
            .bind(question.created_by)
            .bind(question.created_at)
            .fetch_one(&self.pg)
            .await?;
        Ok(id as u64)
    }
    async fn update(&self, id: u64, question: QuestionEntity) -> Result<()> {
        let sql = format!(
            "update {} set title=$1,content=$2,updated_at=$3,updated_by=$4 where id=$5",
            QuestionEntity::table_name()
        );
        let res = sqlx::query(sqlx::AssertSqlSafe(&*sql))
            .bind(question.title)
            .bind(question.content)
            .bind(question.updated_at)
            .bind(question.updated_by)
            .bind(id as i64)
            .execute(&self.pg)
            .await?;
        info!("question update affect the rows {} ", res.rows_affected());
        Ok(())
    }
    async fn delete(&self, id: u64, username: String) -> Result<()> {
        let sql = format!(
            "delete from {} where id=$1 and create_by=$2",
            QuestionEntity::table_name()
        );
        let res = sqlx::query(sqlx::AssertSqlSafe(&*sql))
            .bind(id as i64)
            .bind(username)
            .execute(&self.pg)
            .await?;
        info!("question delete affect the rows {} ", res.rows_affected());
        Ok(())
    }
    async fn find_one(&self, id: u64) -> Result<QuestionEntity> {
        let sql = format!("select * from {} where id=$1", QuestionEntity::table_name());
        let question = sqlx::query_as(sqlx::AssertSqlSafe(&*sql))
            .bind(id as i64)
            .fetch_one(&self.pg)
            .await?;
        Ok(question)
    }
    async fn find_latest(&self, last_id: u64, limit: u64) -> Result<LatestQuestionResponse> {
        let mut questions = vec![];

        let sql = format!(
            "selec * from {} order by id desc limit $2",
            QuestionEntity::table_name()
        );
        questions = sqlx::query_as::<_, QuestionEntity>(sqlx::AssertSqlSafe(&*sql))
            .bind(last_id as i64)
            .bind(limit as i64)
            .fetch_all(&self.pg)
            .await?;

        let last_id = questions.last().map(|q| q.id);
        let is_end = questions.len() < limit as usize;
        Ok(LatestQuestionResponse {
            questions,
            last_id: last_id,
            is_end,
        })
    }

    //read_count with redis
    async fn incr(&self, target_id: u64, target_type: String) -> Result<u64> {
        let hash_key = self.get_hash_key(target_type.clone());
        let increment;
        match &self.redis {
            RedisPool::Single(pool) => {
                let mut conn = pool.get()?;
                increment = conn.hincr(hash_key, target_id.to_string(), 1)?;
            }
            RedisPool::Cluster(pool) => {
                let mut conn = pool.get()?;
                increment = conn.hincr(hash_key, target_id.to_string(), 1)?;
            }
        }
        info!(
            "incr target_id:{}, target_type:{}, read_count:{} ",
            target_id, target_type, increment
        );
        Ok(increment)
    }

    //read_count with pgsql
    async fn handler(&self, target_type: String) -> Result<()> {
        self.handler_redis_topg(target_type).await?;
        Ok(())
    }
}
