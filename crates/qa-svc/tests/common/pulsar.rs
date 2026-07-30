use anyhow::{Context, Result};
use pulsar::{Pulsar, TokioExecutor};
use qa_sys_core::PulsarService;
use serde::Deserialize;

const APP_CONFIG: &str = include_str!("../../../../app.yaml");

#[derive(Debug, Deserialize)]
struct AppConfig {
    pulsar_conf: PulsarConfig,
}

#[derive(Debug, Deserialize)]
struct PulsarConfig {
    addr: String,
    token: String,
}

pub async fn pulsar_client() -> Result<Pulsar<TokioExecutor>> {
    let config: AppConfig =
        serde_yaml::from_str(APP_CONFIG).context("parse app.yaml for Pulsar integration tests")?;
    let mut service = PulsarService::builder(config.pulsar_conf.addr)?;
    if !config.pulsar_conf.token.is_empty() {
        service = service.with_token(config.pulsar_conf.token)?;
    }
    service
        .client()
        .await
        .context("connect to Pulsar for vote repository integration tests")
}
