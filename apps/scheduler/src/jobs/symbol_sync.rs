use std::{
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use chrono::{DateTime, Utc};
use shared_binance::{
    sync_symbol_metadata, BinanceClient, CredentialValidationRequest, SymbolMetadata,
};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolSyncRuntimeState {
    pub run_count: u64,
    pub last_run_at: Option<DateTime<Utc>>,
    pub last_synced_symbols: usize,
}

pub fn spawn_hourly_symbol_sync_job(
    interval: Duration,
    state: Arc<Mutex<SymbolSyncRuntimeState>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || loop {
        let symbols = run_public_symbol_sync_once();
        let mut guard = state.lock().expect("symbol sync runtime state poisoned");
        guard.run_count += 1;
        guard.last_run_at = Some(Utc::now());
        guard.last_synced_symbols = symbols.len();
        drop(guard);

        thread::sleep(interval);
    })
}

pub fn run_public_symbol_sync_once() -> Vec<SymbolMetadata> {
    let client = BinanceClient::new("public-symbol-sync", "public-symbol-sync");
    let request = CredentialValidationRequest::new(
        true,
        &["spot".to_owned(), "usdm".to_owned(), "coinm".to_owned()],
    );
    let check = client.check_credentials_for(&request);

    sync_symbol_metadata(&client, &check)
}

#[cfg(test)]
mod tests {
    use super::{run_public_symbol_sync_once, SymbolSyncRuntimeState};

    #[test]
    fn public_symbol_sync_returns_three_market_catalog() {
        let state = SymbolSyncRuntimeState::default();
        assert_eq!(state.run_count, 0);

        let symbols = run_public_symbol_sync_once();

        assert_eq!(symbols.len(), 6);
        assert!(symbols.iter().any(|symbol| symbol.market == "spot"));
        assert!(symbols.iter().any(|symbol| symbol.market == "usdm"));
        assert!(symbols.iter().any(|symbol| symbol.market == "coinm"));
    }
}
