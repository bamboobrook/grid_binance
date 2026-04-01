use crate::SharedDbError;

#[derive(Debug, Clone)]
pub struct RedisConfig {
    url: String,
}

impl RedisConfig {
    pub fn new(url: impl Into<String>) -> Result<Self, SharedDbError> {
        let url = url.into();
        ::redis::Client::open(url.clone())
            .map(|_| Self { url })
            .map_err(SharedDbError::from)
    }

    pub fn url(&self) -> &str {
        &self.url
    }
}
