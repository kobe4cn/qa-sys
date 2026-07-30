use anyhow::{Context, Result};
use serde::Deserialize;
use sqlx::{Connection, PgConnection, PgPool, postgres::PgPoolOptions};
use uuid::Uuid;

const APP_CONFIG: &str = include_str!("../../../../app.yaml");
const MIGRATION: &str = include_str!("../../../../migrations/20260725064428_db.sql");

#[derive(Debug, Deserialize)]
struct AppConfig {
    pgsql_conf: PostgresConfig,
}

#[derive(Debug, Deserialize)]
struct PostgresConfig {
    dsn: String,
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
        let database_name = format!("qa_sys_test_{}", Uuid::new_v4().simple());
        let database_url = format!("{server_url}/{database_name}");
        let mut admin = PgConnection::connect(&admin_url)
            .await
            .context("connect to PostgreSQL administrative database")?;
        let create_database = format!(r#"CREATE DATABASE "{database_name}""#);
        sqlx::query(sqlx::AssertSqlSafe(create_database))
            .execute(&mut admin)
            .await
            .context("create isolated PostgreSQL test database")?;

        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(&database_url)
            .await
            .context("connect to isolated PostgreSQL test database")?;
        sqlx::raw_sql(MIGRATION)
            .execute(&pool)
            .await
            .context("apply migrations to isolated PostgreSQL test database")?;

        Ok(Self {
            admin_url,
            database_name,
            pool,
        })
    }

    pub async fn cleanup(self) -> Result<()> {
        self.pool.close().await;
        let mut admin = PgConnection::connect(&self.admin_url)
            .await
            .context("reconnect to PostgreSQL administrative database")?;
        let drop_database = format!(
            r#"DROP DATABASE IF EXISTS "{}" WITH (FORCE)"#,
            self.database_name
        );
        sqlx::query(sqlx::AssertSqlSafe(drop_database))
            .execute(&mut admin)
            .await
            .context("drop isolated PostgreSQL test database")?;
        Ok(())
    }
}

fn config() -> Result<AppConfig> {
    serde_yaml::from_str(APP_CONFIG).context("parse app.yaml for integration tests")
}
