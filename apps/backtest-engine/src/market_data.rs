#[derive(Debug, Clone, PartialEq)]
pub struct KlineBar {
    pub symbol: String,
    pub open_time_ms: i64,
    pub open: f64,
    pub high: f64,
    pub low: f64,
    pub close: f64,
    pub volume: f64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct AggTrade {
    pub symbol: String,
    pub trade_time_ms: i64,
    pub price: f64,
    pub quantity: f64,
    pub is_buyer_maker: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct DataQualityReport {
    pub missing_bars: u64,
    pub duplicate_bars: u64,
    pub zero_price_bars: u64,
    pub completeness_score: f64,
}

pub trait MarketDataSource {
    fn list_symbols(&self) -> Result<Vec<String>, String>;

    fn load_klines(
        &self,
        symbol: &str,
        start_ms: i64,
        end_ms: i64,
        interval: &str,
    ) -> Result<Vec<KlineBar>, String>;

    fn load_agg_trades(
        &self,
        symbol: &str,
        start_ms: i64,
        end_ms: i64,
    ) -> Result<Vec<AggTrade>, String>;

    fn schema_fingerprint(&self) -> Result<String, String>;
}
