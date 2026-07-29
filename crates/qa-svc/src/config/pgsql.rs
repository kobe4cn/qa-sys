use std::time::Duration;

/**
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
    max_lifetime: Duration,
    idle_timeout: Duration,
    connect_timeout: Duration,
}

pub async fn pool(config: &PgsqlConfig) -> Result<sqlx::PgPool> {
    PgsqlService::build(config.dsn.clone())?
        .with_max_connections(config.max_connections)?
        .with_min_connections(config.min_connections)?
        .with_max_lifetime(config.max_lifetime)?
        .with_idle_timeout(config.idle_timeout)?
        .with_connect_timeout(config.connect_timeout)?
        .pool()
        .await
}
