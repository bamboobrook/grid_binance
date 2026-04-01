use shared_binance::{BinanceClient, ExchangeCredentialCheck, SymbolMetadata};

pub fn sync_symbols(
    client: &BinanceClient,
    check: &ExchangeCredentialCheck,
) -> Vec<SymbolMetadata> {
    let mut symbols = Vec::new();

    if check.can_read_spot {
        symbols.extend(client.spot_symbols());
    }

    if check.can_read_futures {
        symbols.extend(client.futures_symbols());
    }

    symbols.sort_by(|left, right| {
        left.symbol
            .cmp(&right.symbol)
            .then(left.market.cmp(&right.market))
    });
    symbols.dedup_by(|left, right| left.symbol == right.symbol && left.market == right.market);
    symbols
}
