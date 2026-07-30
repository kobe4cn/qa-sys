use std::time::Duration;

/*
* pgsql_conf:
 dsn: "postgresql://postgres:postgres@127.0.0.1:5432/qa_sys" # dsn连接句柄信息
 max_connections: 100 # 最大连接数
 min_connections: 10  # 最小连接数
 max_lifetime: 1800  # 连接池默认生命周期，单位s
 idle_timeout: 300   # 空闲连接生命周期超时，单位s
 connect_timeout: 10 # 连接超时时间，单位s
*/
use anyhow::Result;
use qa_sys_core::PgsqlService;

#[derive(Debug, Default, serde::Deserialize, serde::Serialize, PartialEq)]
pub struct PgsqlConfig {
    dsn: String,
    max_connections: u32,
    min_connections: u32,
    max_lifetime: u64,
    idle_timeout: u64,
    connect_timeout: u64,
}

pub async fn pool(config: &PgsqlConfig) -> Result<sqlx::PgPool> {
    PgsqlService::build(config.dsn.clone())?
        .with_max_connections(config.max_connections)?
        .with_min_connections(config.min_connections)?
        .with_max_lifetime(Duration::from_secs(config.max_lifetime))?
        .with_idle_timeout(Duration::from_secs(config.idle_timeout))?
        .with_connect_timeout(Duration::from_secs(config.connect_timeout))?
        .pool()
        .await
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use super::PgsqlConfig;

    #[test]
    fn test_should_deserialize_postgres_durations_as_seconds() -> Result<()> {
        let config: PgsqlConfig = serde_yaml::from_str(
            r#"
dsn: postgresql://postgres:postgres@127.0.0.1:5432/qa_sys
max_connections: 100
min_connections: 10
max_lifetime: 1800
idle_timeout: 300
connect_timeout: 10
"#,
        )?;

        assert_eq!(config.max_lifetime, 1800);
        assert_eq!(config.idle_timeout, 300);
        assert_eq!(config.connect_timeout, 10);
        Ok(())
    }
}
