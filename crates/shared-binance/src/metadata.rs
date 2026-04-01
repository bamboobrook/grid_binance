use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SymbolMetadata {
    pub symbol: String,
    pub market: String,
    pub status: String,
}

impl SymbolMetadata {
    pub fn new(
        symbol: impl Into<String>,
        market: impl Into<String>,
        status: impl Into<String>,
    ) -> Self {
        Self {
            symbol: symbol.into(),
            market: market.into(),
            status: status.into(),
        }
    }
}

pub fn matches_symbol_query(symbol: &SymbolMetadata, query: &str) -> bool {
    let terms: Vec<String> = query
        .split_whitespace()
        .map(normalize)
        .filter(|term| !term.is_empty())
        .collect();
    if terms.is_empty() {
        return false;
    }

    let haystack = normalize(&format!(
        "{} {} {}",
        symbol.symbol, symbol.market, symbol.status
    ));
    terms.into_iter().all(|term| haystack.contains(&term))
}

fn normalize(input: &str) -> String {
    input
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}
