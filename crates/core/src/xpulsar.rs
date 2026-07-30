/*
* pulsar_conf:
 addr: pulsar://127.0.0.1:6650
 token: "" # pulsar auth token
*/
use anyhow::Result;
use pulsar::{Authentication, Pulsar, TokioExecutor};
use thiserror::Error;
pub struct PulsarService {
    addr: String,
    token: Option<String>,
}

impl PulsarService {
    pub fn builder(dsn: String) -> Result<Self> {
        if dsn.is_empty() {
            return Err(anyhow::anyhow!("pulsar dsn is empty"));
        }
        Ok(Self {
            addr: dsn,
            token: None,
        })
    }

    pub fn with_token(mut self, token: String) -> Result<Self> {
        self.token = Some(token);
        Ok(self)
    }

    pub async fn client(self) -> Result<Pulsar<TokioExecutor>, PulsarError> {
        let mut builder = Pulsar::builder(&self.addr, TokioExecutor);
        if let Some(token) = self.token {
            let auth = Authentication {
                name: "token".to_string(),
                data: token.to_string().into_bytes(),
            };
            builder = builder.with_auth(auth);
        }
        let client = builder.build().await?;
        Ok(client)
    }
}
#[derive(Debug, Error)]
pub enum PulsarError {
    #[error("pulsar connection error: {}", _0)]
    ConnectionError(#[from] pulsar::Error),
}

#[cfg(test)]
mod tests {
    use anyhow::Result;

    use super::PulsarService;

    #[test]
    fn test_should_reject_empty_pulsar_dsn() {
        assert!(PulsarService::builder(String::new()).is_err());
    }

    #[test]
    fn test_should_build_pulsar_configuration_with_optional_token() -> Result<()> {
        let without_token = PulsarService::builder("pulsar://127.0.0.1:6650".to_string())?;
        assert_eq!(without_token.addr, "pulsar://127.0.0.1:6650");
        assert!(without_token.token.is_none());

        let with_token = PulsarService::builder("pulsar://127.0.0.1:6650".to_string())?
            .with_token("secret".to_string())?;
        assert_eq!(with_token.token.as_deref(), Some("secret"));
        Ok(())
    }
}
