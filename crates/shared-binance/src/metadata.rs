use serde::{Deserialize, Serialize};

use crate::{BinanceClient, ExchangeCredentialCheck};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct SymbolMetadata {
    pub symbol: String,
    pub market: String,
    pub status: String,
    pub base_asset: String,
    pub quote_asset: String,
    pub price_precision: u32,
    pub quantity_precision: u32,
    pub min_quantity: String,
    pub min_notional: String,
    pub keywords: Vec<String>,
}

impl SymbolMetadata {
    pub fn new<I, S>(
        symbol: impl Into<String>,
        market: impl Into<String>,
        status: impl Into<String>,
        base_asset: impl Into<String>,
        quote_asset: impl Into<String>,
        price_precision: u32,
        quantity_precision: u32,
        min_quantity: impl Into<String>,
        min_notional: impl Into<String>,
        keywords: I,
    ) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        Self {
            symbol: symbol.into(),
            market: market.into(),
            status: status.into(),
            base_asset: base_asset.into(),
            quote_asset: quote_asset.into(),
            price_precision,
            quantity_precision,
            min_quantity: min_quantity.into(),
            min_notional: min_notional.into(),
            keywords: keywords.into_iter().map(Into::into).collect(),
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
        "{} {} {} {} {} {}",
        symbol.symbol,
        symbol.market,
        symbol.status,
        symbol.base_asset,
        symbol.quote_asset,
        symbol.keywords.join(" ")
    ));
    terms.into_iter().all(|term| haystack.contains(&term))
}

pub fn sync_symbol_metadata(
    client: &BinanceClient,
    check: &ExchangeCredentialCheck,
) -> Vec<SymbolMetadata> {
    let mut symbols = Vec::new();

    if check.can_read_spot {
        symbols.extend(client.spot_symbols());
    }

    if check.can_read_usdm {
        symbols.extend(client.usdm_symbols());
    }

    if check.can_read_coinm {
        symbols.extend(client.coinm_symbols());
    }

    symbols.sort_by(|left, right| {
        left.symbol
            .cmp(&right.symbol)
            .then(left.market.cmp(&right.market))
    });
    symbols.dedup_by(|left, right| left.symbol == right.symbol && left.market == right.market);
    symbols
}

fn normalize(input: &str) -> String {
    input
        .chars()
        .filter(|ch| ch.is_ascii_alphanumeric())
        .flat_map(|ch| ch.to_lowercase())
        .collect()
}
