use std::{str::FromStr, time::Duration};

use sqlx::postgres::{PgConnectOptions, PgPoolOptions};

use crate::SharedDbError;

#[derive(Debug, Clone)]
pub struct PostgresConfig {
    url: String,
    max_connections: u32,
    min_connections: u32,
    acquire_timeout: Duration,
}

impl PostgresConfig {
    pub fn new(url: impl Into<String>) -> Result<Self, SharedDbError> {
        let url = url.into();
        PgConnectOptions::from_str(&url)
            .map_err(SharedDbError::from)
            .map(|_| Self {
                url,
                max_connections: 10,
                min_connections: 1,
                acquire_timeout: Duration::from_secs(5),
            })
    }

    pub fn url(&self) -> &str {
        &self.url
    }

    pub fn connect_options(&self) -> Result<PgConnectOptions, SharedDbError> {
        PgConnectOptions::from_str(&self.url).map_err(SharedDbError::from)
    }

    pub fn pool_options(&self) -> PgPoolOptions {
        PgPoolOptions::new()
            .max_connections(self.max_connections)
            .min_connections(self.min_connections)
            .acquire_timeout(self.acquire_timeout)
    }
}
