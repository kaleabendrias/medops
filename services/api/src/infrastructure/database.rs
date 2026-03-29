use std::time::Duration;

use sqlx::mysql::MySqlPoolOptions;

use crate::config::DatabaseConfig;

pub async fn connect_pool(cfg: &DatabaseConfig) -> Result<sqlx::MySqlPool, sqlx::Error> {
    MySqlPoolOptions::new()
        .max_connections(cfg.max_connections)
        .min_connections(cfg.min_connections)
        .acquire_timeout(Duration::from_secs(cfg.connect_timeout_secs))
        .connect(&cfg.url)
        .await
}

pub async fn run_migrations(pool: &sqlx::MySqlPool) -> Result<(), sqlx::migrate::MigrateError> {
    sqlx::migrate!("./migrations").run(pool).await
}
