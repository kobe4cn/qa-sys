use anyhow::{Context, Result};
use qa_sys_core::{RedisPool, RedisService};
use serde::Deserialize;

const APP_CONFIG: &str = include_str!("../../../../app.yaml");
const TEST_REDIS_DATABASE: &str = "15";

#[derive(Debug, Deserialize)]
struct AppConfig {
    redis_conf: RedisConfig,
}

#[derive(Debug, Deserialize)]
struct RedisConfig {
    dsn: String,
}

pub fn redis_pool() -> Result<RedisPool> {
    let config: AppConfig =
        serde_yaml::from_str(APP_CONFIG).context("parse app.yaml for Redis integration tests")?;
    let (server_url, _) = config
        .redis_conf
        .dsn
        .rsplit_once('/')
        .context("Redis DSN must include a database number")?;
    RedisService::builder(format!("{server_url}/{TEST_REDIS_DATABASE}"))?
        .with_max_size(5)?
        .with_min_idle(0)?
        .pool()
}
