/**
* pulsar_conf:
 addr: pulsar://127.0.0.1:6650
 token: "" # pulsar auth token
*/
use anyhow::Result;
use pulsar::{Pulsar, TokioExecutor};
use qa_sys_core::PulsarService;

#[derive(Debug, Default, Clone, serde::Deserialize, serde::Serialize, PartialEq)]
pub struct PulsarConfig {
    addr: String,
    token: String,
}

pub async fn client(config: &PulsarConfig) -> Result<Pulsar<TokioExecutor>> {
    let mut p = PulsarService::builder(config.addr.clone())?;
    if !config.token.is_empty() {
        p = p.with_token(config.token.clone())?;
    }
    let client = p.client().await?;
    Ok(client)
}
