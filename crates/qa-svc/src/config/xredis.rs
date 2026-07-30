/*
redis_conf:
  dsn: "redis://:redis@127.0.0.1:6379/0"   # redis dsn信息，用于连接redis
  max_size: 300                       # 最大连接个数，默认为300
  min_idle: 3                         # 最小空闲数，默认为3
  max_lifetime: 1800                  # 过期时间，默认为1800s
  idle_timeout: 300                   # 连接池最大生存期，默认为300s
  connection_timeout: 10              # 连接超时时间，默认为10s
   */

use std::time::Duration;

use anyhow::Result;
use qa_sys_core::{RedisPool, RedisService};

#[derive(Debug, Default, Clone, serde::Deserialize, serde::Serialize, PartialEq)]
pub struct RedisConfig {
    dsn: String,
    max_size: u32,
    min_idle: u32,
    max_lifetime: u64,
    idle_timeout: u64,
    connection_timeout: u64,
}

pub async fn pool(config: &RedisConfig) -> Result<RedisPool> {
    let redis_service = RedisService::builder(config.dsn.clone())?
        .with_max_size(config.max_size)?
        .with_connection_timeout(Duration::from_secs(config.connection_timeout))?
        .with_idle_timeout(Duration::from_secs(config.idle_timeout))?
        .with_max_lifetime(Duration::from_secs(config.max_lifetime))?
        .with_min_idle(config.min_idle)?;
    redis_service.pool()
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use super::RedisConfig;

    #[test]
    fn test_should_deserialize_redis_durations_as_seconds() -> Result<()> {
        let config: RedisConfig = serde_yaml::from_str(
            r#"
dsn: redis://:redis@127.0.0.1:6379/0
max_size: 300
min_idle: 3
max_lifetime: 1800
idle_timeout: 300
connection_timeout: 10
"#,
        )?;

        assert_eq!(config.max_lifetime, 1800);
        assert_eq!(config.idle_timeout, 300);
        assert_eq!(config.connection_timeout, 10);
        Ok(())
    }
}
