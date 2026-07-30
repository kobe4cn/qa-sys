/*
 *
 * app_debug: true # 是否开启调试模式
app_port: 50051 # grpc service运行端口
metrics_port: 2338 # prometheus metrics port
graceful_wait_time: 3 # 平滑退出等待时间，单位s
pgsql_conf:
  dsn: "postgresql://postgres:postgres@127.0.0.1:5432/qa_sys" # dsn连接句柄信息
  max_connections: 100 # 最大连接数
  min_connections: 10  # 最小连接数
  max_lifetime: 1800  # 连接池默认生命周期，单位s
  idle_timeout: 300   # 空闲连接生命周期超时，单位s
  connect_timeout: 10 # 连接超时时间，单位s

pulsar_conf:
  addr: pulsar://127.0.0.1:6650
  token: "" # pulsar auth token
redis_conf:
  dsn: "redis://:redis@127.0.0.1:6379/0"   # redis dsn信息，用于连接redis
  max_size: 300                       # 最大连接个数，默认为300
  min_idle: 3                         # 最小空闲数，默认为3
  max_lifetime: 1800                  # 过期时间，默认为1800s
  idle_timeout: 300                   # 连接池最大生存期，默认为300s
  connection_timeout: 10              # 连接超时时间，默认为10s

# aes加解密配置
aes_key: "YiBX0z9WnJjsS5aNXmi0AeT1yTPZZJYa"
aes_iv: "3ZQEpwP9DbK4h1Z0"
 */
use once_cell::sync::Lazy;
use pulsar::{Pulsar, TokioExecutor};
use qa_sys_core::{Config, ConfigTrait, RedisPool};
use sqlx::PgPool;

use crate::config::{
    pgsql::PgsqlConfig,
    xpulsar::{PulsarConfig, VoteMessagingConfig},
    xredis::RedisConfig,
};

#[derive(Debug, serde::Deserialize, serde::Serialize, PartialEq)]
pub struct AppConfig {
    pub app_debug: bool,
    pub app_port: u32,
    pub metrics_port: u16,
    pub graceful_wait_time: u64,
    pub pgsql_conf: PgsqlConfig,
    pub pulsar_conf: PulsarConfig,
    pub vote_messaging_conf: VoteMessagingConfig,
    pub redis_conf: RedisConfig,
    pub aes_key: String,
    pub aes_iv: String,
}
#[derive(Clone)]
pub struct AppState {
    pub pgsql_pool: PgPool,
    pub pulsar_client: Pulsar<TokioExecutor>,
    pub redis_pool: RedisPool,
}
#[derive(Clone)]
pub struct ReadCountJobAppState {
    pub pgsql_pool: PgPool,
    pub redis_pool: RedisPool,
}

#[derive(Clone)]
pub struct VoteJobAppState {
    pub pgsql_pool: PgPool,
    pub pulsar_client: Pulsar<TokioExecutor>,
}

pub static APP_CONFIG: Lazy<AppConfig> = Lazy::new(|| {
    let config_path = std::env::var("APP_CONFIG").unwrap_or_else(|_| "./app.yaml".to_string());
    let c = Config::load(&config_path).expect("Failed to load config file");
    serde_yaml::from_str(&c.contents().expect("Failed to read config file"))
        .expect("Failed to parse config file")
});
