use anyhow::{Result, anyhow};
use qa_sys_core::RedisPool;
use redis::Commands;
use uuid::Uuid;

use crate::{UserEntity, UserRepository, UserSessionEntity};

pub struct UserRepositoryImpl {
    db: sqlx::PgPool,
    redis: RedisPool,
}

impl UserRepositoryImpl {
    pub fn new(db: sqlx::PgPool, redis: RedisPool) -> Self {
        Self { db, redis }
    }
    pub fn gen_in_placeholder(len: usize) -> String {
        (1..=len)
            .into_iter()
            .map(|i| format!("${i}"))
            .collect::<Vec<String>>()
            .join(",")
    }
}

#[async_trait::async_trait]
impl UserRepository for UserRepositoryImpl {
    async fn check_user_exist(&self, username: String) -> Result<bool> {
        let sql = format!(
            "SELECT id FROM {} WHERE username = $1 limit 1",
            UserEntity::table_name()
        );
        let user: Option<i64> = sqlx::query_scalar(sqlx::AssertSqlSafe(&*sql))
            .bind(username)
            .fetch_optional(&self.db)
            .await?;
        Ok(user.is_some())
    }

    async fn add(&self, username: String, password: String) -> Result<()> {
        let sql = format!(
            "INSERT INTO {} (username, password,openid,created_at) VALUES ($1, $2,$3,$4)",
            UserEntity::table_name()
        );

        let pwd = format!("{:x}", md5::compute(password.as_bytes()));

        let openid = Uuid::new_v4().to_string().replace('-', "");
        let create_at = chrono::Local::now().naive_local();
        let _ = sqlx::query(sqlx::AssertSqlSafe(&*sql))
            .bind(username)
            .bind(pwd)
            .bind(openid)
            .bind(create_at)
            .execute(&self.db)
            .await?;
        Ok(())
    }
    async fn fetch_one(&self, username: String) -> Result<UserEntity> {
        let sql = format!(
            r#"
            SELECT * FROM {} 
            WHERE username = $1 limit 1
        "#,
            UserEntity::table_name()
        );
        let user = sqlx::query_as(sqlx::AssertSqlSafe(&*sql))
            .bind(username)
            .fetch_one(&self.db)
            .await?;
        Ok(user)
    }
    async fn fetch_users(&self, usernames: Vec<String>) -> Result<Vec<UserEntity>> {
        if usernames.is_empty() {
            return Ok(Vec::new());
        }

        let users = Self::gen_in_placeholder(usernames.len());
        let sql = format!(
            "select * from {} where username in ({})",
            UserEntity::table_name(),
            users
        );
        let mut query = sqlx::query_as(sqlx::AssertSqlSafe(&*sql));
        for username in usernames {
            query = query.bind(username);
        }
        let users = query.fetch_all(&self.db).await?;
        Ok(users)
    }
    async fn delete(&self, username: String) -> Result<()> {
        let sql = format!("delete from {} where username=$1", UserEntity::table_name());
        let _ = sqlx::query(sqlx::AssertSqlSafe(&*sql))
            .bind(username)
            .execute(&self.db)
            .await?;
        Ok(())
    }

    // use session
    async fn get(&self, key: String) -> Result<UserSessionEntity> {
        let value: Option<String> = match &self.redis {
            RedisPool::Single(pool) => {
                let mut conn = pool.get()?;
                conn.get(&key)?
            }
            RedisPool::Cluster(pool) => {
                let mut conn = pool.get()?;
                conn.get(&key)?
            }
        };

        let Some(val) = value else {
            return Err(anyhow!("user session not found"));
        };

        let user = serde_json::from_str(&val)?;
        Ok(user)
    }
    async fn set(&self, key: String, value: UserSessionEntity, second: u64) -> Result<()> {
        let value = serde_json::to_string(&value)?;
        match &self.redis {
            RedisPool::Single(pool) => {
                let mut conn = pool.get()?;
                let _: () = conn.set_ex(&key, value, second)?;
            }
            RedisPool::Cluster(pool) => {
                let mut conn = pool.get()?;
                let _: () = conn.set_ex(&key, value, second)?;
            }
        }
        Ok(())
    }
    async fn del(&self, key: String) -> Result<()> {
        match &self.redis {
            RedisPool::Single(pool) => {
                let mut conn = pool.get()?;
                let _: () = conn.del(&key)?;
            }
            RedisPool::Cluster(pool) => {
                let mut conn = pool.get()?;
                let _: () = conn.del(&key)?;
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::UserRepositoryImpl;

    #[test]
    fn test_should_generate_empty_placeholder_list_for_zero_values() {
        assert_eq!(UserRepositoryImpl::gen_in_placeholder(0), "");
    }

    #[test]
    fn test_should_generate_contiguous_postgres_placeholders() {
        assert_eq!(UserRepositoryImpl::gen_in_placeholder(1), "$1");
        assert_eq!(UserRepositoryImpl::gen_in_placeholder(4), "$1,$2,$3,$4",);
    }
}
