use ::redis::{cmd, Client};
use shared_events::MarketTick;

use crate::SharedDbError;

const MARKET_TICK_QUEUE_KEY: &str = "market_ticks";
const MARKET_TICK_CHANNEL: &str = "market_ticks:live";

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

    pub async fn enqueue_market_tick(&self, tick: &MarketTick) -> Result<(), SharedDbError> {
        let payload =
            serde_json::to_string(tick).map_err(|error| SharedDbError::new(error.to_string()))?;
        let mut connection = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(SharedDbError::from)?;
        let _: i64 = cmd("RPUSH")
            .arg(MARKET_TICK_QUEUE_KEY)
            .arg(payload)
            .query_async(&mut connection)
            .await
            .map_err(SharedDbError::from)?;
        Ok(())
    }

    pub async fn drain_market_ticks(&self, limit: usize) -> Result<Vec<MarketTick>, SharedDbError> {
        let mut connection = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(SharedDbError::from)?;
        let mut ticks = Vec::new();
        for _ in 0..limit {
            let payload: Option<String> = cmd("LPOP")
                .arg(MARKET_TICK_QUEUE_KEY)
                .query_async(&mut connection)
                .await
                .map_err(SharedDbError::from)?;
            let Some(payload) = payload else {
                break;
            };
            let tick = serde_json::from_str::<MarketTick>(&payload)
                .map_err(|error| SharedDbError::new(error.to_string()))?;
            ticks.push(tick);
        }
        Ok(ticks)
    }

    pub async fn publish_market_tick(&self, tick: &MarketTick) -> Result<(), SharedDbError> {
        let payload =
            serde_json::to_string(tick).map_err(|error| SharedDbError::new(error.to_string()))?;
        let mut connection = self
            .client
            .get_multiplexed_async_connection()
            .await
            .map_err(SharedDbError::from)?;
        let _: i64 = cmd("PUBLISH")
            .arg(MARKET_TICK_CHANNEL)
            .arg(&payload)
            .query_async(&mut connection)
            .await
            .map_err(SharedDbError::from)?;
        Ok(())
    }

    pub async fn run_market_tick_subscriber(
        &self,
        tx: tokio::sync::mpsc::UnboundedSender<MarketTick>,
    ) -> Result<(), SharedDbError> {
        use futures::StreamExt;
        let connection = self
            .client
            .get_async_connection()
            .await
            .map_err(SharedDbError::from)?;
        let mut pubsub = connection.into_pubsub();
        pubsub
            .subscribe(MARKET_TICK_CHANNEL)
            .await
            .map_err(SharedDbError::from)?;
        let mut stream = pubsub.into_on_message();
        loop {
            let msg = match stream.next().await {
                Some(msg) => msg,
                None => {
                    return Err(SharedDbError::new("market tick pubsub stream ended"));
                }
            };
            let payload: String = match msg.get_payload() {
                Ok(payload) => payload,
                Err(_) => continue,
            };
            let tick = match serde_json::from_str::<MarketTick>(&payload) {
                Ok(tick) => tick,
                Err(_) => continue,
            };
            if tx.send(tick).is_err() {
                break;
            }
        }
        Ok(())
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
