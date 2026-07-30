/*
* pgsql_conf:
 dsn: "postgresql://postgres:postgres@127.0.0.1:5432/qa_sys" # dsn连接句柄信息
 max_connections: 100 # 最大连接数
 min_connections: 10  # 最小连接数
 max_lifetime: 1800  # 连接池默认生命周期，单位s
 idle_timeout: 300   # 空闲连接生命周期超时，单位s
 connect_timeout: 10 # 连接超时时间，单位s
*/
use std::time::Duration;

use anyhow::Result;
use sqlx::{PgPool, postgres::PgPoolOptions};
#[derive(Debug, Default)]
pub struct PgsqlService {
    dsn: String,
    max_connections: u32,
    min_connections: u32,
    max_lifetime: Duration,
    idle_timeout: Duration,
    connect_timeout: Duration,
}

impl PgsqlService {
    pub fn build(dsn: String) -> Result<Self> {
        if dsn.is_empty() {
            return Err(anyhow::anyhow!("pgsql dsn is empty"));
        }
        Ok(Self {
            dsn,
            max_connections: 100,
            min_connections: 10,
            max_lifetime: Duration::from_secs(1800),
            idle_timeout: Duration::from_secs(300),
            connect_timeout: Duration::from_secs(10),
        })
    }

    pub fn with_max_connections(mut self, max_connections: u32) -> Result<Self> {
        self.max_connections = max_connections;
        Ok(self)
    }

    pub fn with_min_connections(mut self, min_connections: u32) -> Result<Self> {
        self.min_connections = min_connections;
        Ok(self)
    }

    pub fn with_max_lifetime(mut self, max_lifetime: Duration) -> Result<Self> {
        self.max_lifetime = max_lifetime;
        Ok(self)
    }

    pub fn with_idle_timeout(mut self, idle_timeout: Duration) -> Result<Self> {
        self.idle_timeout = idle_timeout;
        Ok(self)
    }

    pub fn with_connect_timeout(mut self, connect_timeout: Duration) -> Result<Self> {
        self.connect_timeout = connect_timeout;
        Ok(self)
    }

    pub async fn pool(&self) -> Result<PgPool> {
        let pool_config = PgPoolOptions::new()
            .max_connections(self.max_connections)
            .min_connections(self.min_connections)
            .max_lifetime(self.max_lifetime)
            .idle_timeout(self.idle_timeout)
            .acquire_timeout(self.connect_timeout)
            .connect(&self.dsn)
            .await?;
        Ok(pool_config)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use anyhow::Result;

    use super::PgsqlService;

    #[test]
    fn test_should_reject_empty_postgres_dsn() {
        assert!(PgsqlService::build(String::new()).is_err());
    }

    #[test]
    fn test_should_apply_postgres_pool_settings() -> Result<()> {
        let service = PgsqlService::build("postgresql://localhost/test".to_string())?
            .with_max_connections(12)?
            .with_min_connections(2)?
            .with_max_lifetime(Duration::from_secs(90))?
            .with_idle_timeout(Duration::from_secs(30))?
            .with_connect_timeout(Duration::from_secs(5))?;

        assert_eq!(service.dsn, "postgresql://localhost/test");
        assert_eq!(service.max_connections, 12);
        assert_eq!(service.min_connections, 2);
        assert_eq!(service.max_lifetime, Duration::from_secs(90));
        assert_eq!(service.idle_timeout, Duration::from_secs(30));
        assert_eq!(service.connect_timeout, Duration::from_secs(5));
        Ok(())
    }
}
