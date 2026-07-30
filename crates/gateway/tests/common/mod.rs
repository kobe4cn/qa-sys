use anyhow::{Context, Result};
use pulsar::{Pulsar, TokioExecutor};
use qa_sys_core::{PulsarService, RedisPool, RedisService};
use serde::Deserialize;
use sqlx::{Connection, PgConnection, PgPool, postgres::PgPoolOptions};
use uuid::Uuid;

const APP_CONFIG: &str = include_str!("../../../../app.yaml");
const MIGRATION: &str = include_str!("../../../../migrations/20260725064428_db.sql");
const TEST_REDIS_DATABASE: &str = "15";

#[derive(Debug, Deserialize)]
struct AppConfig {
    pgsql_conf: PostgresConfig,
    redis_conf: RedisConfig,
    pulsar_conf: PulsarConfig,
}

#[derive(Debug, Deserialize)]
struct PostgresConfig {
    dsn: String,
}

#[derive(Debug, Deserialize)]
struct RedisConfig {
    dsn: String,
}

#[derive(Debug, Deserialize)]
struct PulsarConfig {
    addr: String,
    token: String,
}

#[derive(Debug)]
pub struct TestDatabase {
    admin_url: String,
    database_name: String,
    pub pool: PgPool,
}

impl TestDatabase {
    pub async fn create() -> Result<Self> {
        let config = config()?;
        let (server_url, _) = config
            .pgsql_conf
            .dsn
            .rsplit_once('/')
            .context("PostgreSQL DSN must include a database name")?;
        let admin_url = format!("{server_url}/postgres");
        let database_name = format!("qa_sys_gateway_test_{}", Uuid::new_v4().simple());
        let database_url = format!("{server_url}/{database_name}");
        let mut admin = PgConnection::connect(&admin_url).await?;
        let create_database = format!(r#"CREATE DATABASE "{database_name}""#);
        sqlx::query(sqlx::AssertSqlSafe(create_database))
            .execute(&mut admin)
            .await?;
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await?;
        sqlx::raw_sql(MIGRATION).execute(&pool).await?;
        Ok(Self {
            admin_url,
            database_name,
            pool,
        })
    }

    pub async fn cleanup(self) -> Result<()> {
        self.pool.close().await;
        let mut admin = PgConnection::connect(&self.admin_url).await?;
        let drop_database = format!(
            r#"DROP DATABASE IF EXISTS "{}" WITH (FORCE)"#,
            self.database_name
        );
        sqlx::query(sqlx::AssertSqlSafe(drop_database))
            .execute(&mut admin)
            .await?;
        Ok(())
    }
}

pub fn redis_pool() -> Result<RedisPool> {
    let config = config()?;
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

pub fn clear_redis_database(pool: &RedisPool) -> Result<()> {
    match pool {
        RedisPool::Single(pool) => {
            let mut connection = pool.get()?;
            redis::cmd("FLUSHDB").query::<()>(&mut *connection)?;
        }
        RedisPool::Cluster(pool) => {
            let mut connection = pool.get()?;
            redis::cmd("FLUSHDB").query::<()>(&mut *connection)?;
        }
    }
    Ok(())
}

pub async fn pulsar_client() -> Result<Pulsar<TokioExecutor>> {
    let config = config()?;
    let mut service = PulsarService::builder(config.pulsar_conf.addr)?;
    if !config.pulsar_conf.token.is_empty() {
        service = service.with_token(config.pulsar_conf.token)?;
    }
    Ok(service.client().await?)
}

fn config() -> Result<AppConfig> {
    serde_yaml::from_str(APP_CONFIG).context("parse app.yaml for Gateway integration tests")
}
