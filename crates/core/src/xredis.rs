use std::time::Duration;

use anyhow::{Ok, Result};
use r2d2::ManageConnection;
use redis::{Client, cluster::ClusterClient};
/*
* redis_conf:
 dsn: "redis://:@127.0.0.1:6379/0"   # redis dsn信息，用于连接redis
 max_size: 300                       # 最大连接个数，默认为300
 min_idle: 3                         # 最小空闲数，默认为3
 max_lifetime: 1800                  # 过期时间，默认为1800s
 idle_timeout: 300                   # 连接池最大生存期，默认为300s
 connection_timeout: 10
*/
#[derive(Default, Debug)]
pub struct RedisService {
    dsn: String,
    max_size: u32,
    min_idle: u32,
    max_lifetime: Duration,
    idle_timeout: Duration,
    connection_timeout: Duration,
    cluster_nodes: Option<Vec<String>>,
}

#[derive(Clone)]
pub enum RedisPool {
    Single(r2d2::Pool<Client>),         // 單節點redis
    Cluster(r2d2::Pool<ClusterClient>), // 集群redis
}

impl RedisService {
    pub fn builder(dsn: String) -> Result<Self> {
        if dsn.is_empty() {
            return Err(anyhow::anyhow!("redis dsn is empty"));
        }
        Ok(Self {
            dsn,
            max_size: 300,
            min_idle: 3,
            max_lifetime: Duration::from_secs(1800),
            idle_timeout: Duration::from_secs(300),
            connection_timeout: Duration::from_secs(10),
            ..Default::default()
        })
    }

    pub fn with_max_size(mut self, max_size: u32) -> Result<Self> {
        self.max_size = max_size;
        Ok(self)
    }

    pub fn with_min_idle(mut self, min_idle: u32) -> Result<Self> {
        self.min_idle = min_idle;
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

    pub fn with_connection_timeout(mut self, connection_timeout: Duration) -> Result<Self> {
        self.connection_timeout = connection_timeout;
        Ok(self)
    }

    pub fn client(&self) -> Result<Client> {
        if self.dsn.is_empty() {
            return Err(anyhow::anyhow!("redis dsn is empty"));
        }
        let client = Client::open(self.dsn.clone())?;
        Ok(client)
    }

    pub fn cluster_client(&self) -> Result<ClusterClient> {
        if let Some(nodes) = &self.cluster_nodes {
            if nodes.is_empty() {
                return Err(anyhow::anyhow!("redis cluster nodes is empty"));
            }
            let client = ClusterClient::new(nodes.clone())?;
            return Ok(client);
        }
        Err(anyhow::anyhow!("redis cluster nodes is empty"))
    }

    pub fn pool(&self) -> Result<RedisPool> {
        if let Some(nodes) = &self.cluster_nodes {
            if nodes.is_empty() {
                return Err(anyhow::anyhow!("redis cluster nodes is empty"));
            }
            let client = self.cluster_client()?;
            let pool = self.init_pool(client)?;
            return Ok(RedisPool::Cluster(pool));
        }
        let client = self.client()?;
        let pool = self.init_pool(client)?;
        Ok(RedisPool::Single(pool))
    }

    fn init_pool<P: ManageConnection>(&self, client: P) -> Result<r2d2::Pool<P>> {
        let pool = r2d2::Pool::builder()
            .max_size(self.max_size)
            .min_idle(Some(self.min_idle))
            .max_lifetime(Some(self.max_lifetime))
            .idle_timeout(Some(self.idle_timeout))
            .connection_timeout(self.connection_timeout)
            .build(client)?;
        Ok(pool)
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use anyhow::Result;

    use super::RedisService;

    #[test]
    fn test_should_reject_empty_redis_dsn() {
        assert!(RedisService::builder(String::new()).is_err());
    }

    #[test]
    fn test_should_apply_redis_pool_settings() -> Result<()> {
        let service = RedisService::builder("redis://127.0.0.1:6379/15".to_string())?
            .with_max_size(20)?
            .with_min_idle(2)?
            .with_max_lifetime(Duration::from_secs(90))?
            .with_idle_timeout(Duration::from_secs(30))?
            .with_connection_timeout(Duration::from_secs(5))?;

        assert_eq!(service.dsn, "redis://127.0.0.1:6379/15");
        assert_eq!(service.max_size, 20);
        assert_eq!(service.min_idle, 2);
        assert_eq!(service.max_lifetime, Duration::from_secs(90));
        assert_eq!(service.idle_timeout, Duration::from_secs(30));
        assert_eq!(service.connection_timeout, Duration::from_secs(5));
        Ok(())
    }

    #[test]
    fn test_should_build_single_node_client_for_valid_url() -> Result<()> {
        let service = RedisService::builder("redis://127.0.0.1:6379/15".to_string())?;

        assert!(service.client().is_ok());
        Ok(())
    }

    #[test]
    fn test_should_reject_invalid_redis_url() -> Result<()> {
        let service = RedisService::builder("not-a-redis-url".to_string())?;

        assert!(service.client().is_err());
        Ok(())
    }

    #[test]
    fn test_should_reject_cluster_client_without_nodes() -> Result<()> {
        let service = RedisService::builder("redis://127.0.0.1:6379/15".to_string())?;

        assert!(service.cluster_client().is_err());
        Ok(())
    }
}
