use ::redis::{cmd, Client};

use crate::SharedDbError;

use super::config::RedisConfig;

#[derive(Clone)]
pub struct RedisStore {
    config: RedisConfig,
    client: Client,
}

impl RedisStore {
    pub async fn connect(config: RedisConfig) -> Result<Self, SharedDbError> {
        let client = Client::open(config.url().to_owned()).map_err(SharedDbError::from)?;
        let mut connection = client
            .get_multiplexed_async_connection()
            .await
            .map_err(SharedDbError::from)?;
        let _: String = cmd("PING")
            .query_async(&mut connection)
            .await
            .map_err(SharedDbError::from)?;
        Ok(Self { config, client })
    }

    pub fn config(&self) -> &RedisConfig {
        &self.config
    }

    pub fn client(&self) -> &Client {
        &self.client
    }

    pub async fn ping(&self) -> Result<String, SharedDbError> {
        let mut connection = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(SharedDbError::from)?;
        cmd("PING")
            .query_async(&mut connection)
            .await
            .map_err(SharedDbError::from)
    }
}
