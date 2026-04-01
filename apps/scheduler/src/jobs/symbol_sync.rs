use shared_binance::{
    sync_symbol_metadata, BinanceClient, ExchangeCredentialCheck, SymbolMetadata,
};

pub fn sync_symbols(
    client: &BinanceClient,
    check: &ExchangeCredentialCheck,
) -> Vec<SymbolMetadata> {
    sync_symbol_metadata(client, check)
}
