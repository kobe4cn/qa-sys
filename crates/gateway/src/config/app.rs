/*/
app_debug: false # 是否开启调试模式
app_port: 8090 # http gateway port
metrics_port: 1338 # prometheus metrics port
grpc_address: http://127.0.0.1:50051 # grpc service运行端口
graceful_wait_time: 3 # http service平滑退出等待时间，单位s

 */

use std::path::Path;

use once_cell::sync::Lazy;
use pb::qa_service_client::QaServiceClient;
use qa_sys_core::{Config, ConfigTrait};
use serde::Deserialize;

#[derive(Debug, Deserialize, Clone, PartialEq, Default)]
pub struct AppConfig {
    pub app_debug: bool,
    pub app_port: u16,
    pub metrics_port: u16,
    pub grpc_address: String,
    pub graceful_wait_time: u64,
}

pub static APP_CONFIG: Lazy<AppConfig> = Lazy::new(|| {
    let config_dir = std::env::var("QA_CONFIG_DIR").unwrap_or("./".to_string());
    let filename = Path::new(&config_dir).join("app-gw.yaml");
    let c = Config::load(filename).expect("yaml file load error");
    serde_yaml::from_str(&c.contents().expect("Failed to read config file"))
        .expect("Failed to parse config file")
});

#[derive(Debug, Clone)]
pub struct AppState {
    pub grpc_client: QaServiceClient<tonic::transport::Channel>,
}
