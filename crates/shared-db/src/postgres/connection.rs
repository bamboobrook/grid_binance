use sqlx::PgPool;

use crate::SharedDbError;

use super::{config::PostgresConfig, migrations};

#[derive(Clone)]
pub struct PostgresStore {
    config: PostgresConfig,
    pool: PgPool,
}

impl PostgresStore {
    pub async fn connect(config: PostgresConfig) -> Result<Self, SharedDbError> {
        let pool = config
            .pool_options()
            .connect_with(config.connect_options()?)
            .await
            .map_err(SharedDbError::from)?;
        migrations::run(&pool).await?;
        Ok(Self { config, pool })
    }

    pub fn config(&self) -> &PostgresConfig {
        &self.config
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}
