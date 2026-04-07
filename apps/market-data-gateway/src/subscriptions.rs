use std::collections::BTreeMap;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolActivity {
    pub symbol: String,
    pub market: String,
    pub is_active: bool,
}

impl SymbolActivity {
    pub fn new(symbol: impl Into<String>, is_active: bool) -> Self {
        Self::new_with_market(symbol, "spot", is_active)
    }

    pub fn new_with_market(
        symbol: impl Into<String>,
        market: impl Into<String>,
        is_active: bool,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            market: normalize_market(&market.into()),
            is_active,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolSubscription {
    pub symbol: String,
    pub market: String,
    pub stream_name: String,
}

impl SymbolSubscription {
    pub fn trade(symbol: impl Into<String>, market: impl Into<String>) -> Self {
        let symbol = symbol.into();
        let market = normalize_market(&market.into());
        let stream_name = format!("{}@trade", symbol.to_ascii_lowercase());

        Self {
            symbol,
            market,
            stream_name,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MarketStreamPlan {
    pub market: String,
    pub streams: Vec<String>,
    pub url: String,
}

pub fn active_symbol_subscriptions(symbols: &[SymbolActivity]) -> Vec<SymbolSubscription> {
    symbols
        .iter()
        .filter(|symbol| symbol.is_active)
        .map(|symbol| SymbolSubscription::trade(symbol.symbol.clone(), symbol.market.clone()))
        .collect()
}

pub fn market_stream_plans(symbols: &[SymbolActivity]) -> Vec<MarketStreamPlan> {
    let subscriptions = active_symbol_subscriptions(symbols);
    let mut grouped = BTreeMap::<String, Vec<String>>::new();
    for subscription in subscriptions {
        grouped
            .entry(subscription.market)
            .or_default()
            .push(subscription.stream_name);
    }

    grouped
        .into_iter()
        .map(|(market, streams)| MarketStreamPlan {
            url: format!("{}{}", market_ws_base_url(&market), streams.join("/")),
            market,
            streams,
        })
        .collect()
}

pub fn market_ws_base_url(market: &str) -> &'static str {
    match normalize_market(market).as_str() {
        "usdm" => "wss://fstream.binance.com/stream?streams=",
        "coinm" => "wss://dstream.binance.com/stream?streams=",
        _ => "wss://stream.binance.com:9443/stream?streams=",
    }
}

pub fn normalize_market(market: &str) -> String {
    match market.trim().to_ascii_lowercase().as_str() {
        "usdm" | "futuresusdm" | "futures_usdm" => "usdm".to_string(),
        "coinm" | "futurescoinm" | "futures_coinm" => "coinm".to_string(),
        _ => "spot".to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::{market_stream_plans, SymbolActivity};

    #[test]
    fn market_stream_plans_build_market_specific_urls() {
        let plans = market_stream_plans(&[
            SymbolActivity::new_with_market("BTCUSDT", "spot", true),
            SymbolActivity::new_with_market("BTCUSDT", "usdm", true),
            SymbolActivity::new_with_market("ETHUSDT", "usdm", true),
            SymbolActivity::new_with_market("BTCUSD_PERP", "coinm", false),
        ]);

        assert_eq!(plans.len(), 2);
        assert_eq!(plans[0].market, "spot");
        assert_eq!(
            plans[0].url,
            "wss://stream.binance.com:9443/stream?streams=btcusdt@trade"
        );
        assert_eq!(plans[1].market, "usdm");
        assert_eq!(
            plans[1].url,
            "wss://fstream.binance.com/stream?streams=btcusdt@trade/ethusdt@trade"
        );
    }
}
