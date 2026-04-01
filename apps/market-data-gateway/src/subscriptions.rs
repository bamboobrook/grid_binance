#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolActivity {
    pub symbol: String,
    pub is_active: bool,
}

impl SymbolActivity {
    pub fn new(symbol: impl Into<String>, is_active: bool) -> Self {
        Self {
            symbol: symbol.into(),
            is_active,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SymbolSubscription {
    pub symbol: String,
    pub stream_name: String,
}

impl SymbolSubscription {
    pub fn trade(symbol: impl Into<String>) -> Self {
        let symbol = symbol.into();
        let stream_name = format!("{}@trade", symbol.to_ascii_lowercase());

        Self {
            symbol,
            stream_name,
        }
    }
}

pub fn active_symbol_subscriptions(symbols: &[SymbolActivity]) -> Vec<SymbolSubscription> {
    symbols
        .iter()
        .filter(|symbol| symbol.is_active)
        .map(|symbol| SymbolSubscription::trade(symbol.symbol.clone()))
        .collect()
}
