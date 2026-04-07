use std::{
    sync::{Arc, Mutex},
    thread,
    time::Duration,
};

use chrono::{DateTime, Utc};
use shared_binance::{BinanceClient, CredentialValidationError, SymbolMetadata};

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SymbolSyncRuntimeState {
    pub run_count: u64,
    pub failure_count: u64,
    pub last_run_at: Option<DateTime<Utc>>,
    pub last_failure_at: Option<DateTime<Utc>>,
    pub last_synced_symbols: usize,
    pub last_error: Option<String>,
}

pub fn spawn_hourly_symbol_sync_job(
    interval: Duration,
    state: Arc<Mutex<SymbolSyncRuntimeState>>,
) -> thread::JoinHandle<()> {
    thread::spawn(move || loop {
        let result = run_public_symbol_sync_once();
        let mut guard = state.lock().expect("symbol sync runtime state poisoned");
        record_public_symbol_sync_result(&mut guard, &result);
        drop(guard);

        if let Err(error) = result {
            eprintln!("scheduler public symbol sync failed: {error}");
        }

        thread::sleep(interval);
    })
}

pub fn run_public_symbol_sync_once() -> Result<Vec<SymbolMetadata>, CredentialValidationError> {
    let client = BinanceClient::new("public-symbol-sync", "public-symbol-sync");
    let mut symbols = Vec::new();
    symbols.extend(client.spot_symbols_strict()?);
    symbols.extend(client.usdm_symbols_strict()?);
    symbols.extend(client.coinm_symbols_strict()?);
    sort_and_dedup_symbols(&mut symbols);
    Ok(symbols)
}

fn record_public_symbol_sync_result(
    state: &mut SymbolSyncRuntimeState,
    result: &Result<Vec<SymbolMetadata>, CredentialValidationError>,
) {
    let now = Utc::now();
    state.run_count += 1;
    state.last_run_at = Some(now);

    match result {
        Ok(symbols) => {
            state.last_synced_symbols = symbols.len();
            state.last_error = None;
        }
        Err(error) => {
            state.failure_count += 1;
            state.last_failure_at = Some(now);
            state.last_error = Some(error.to_string());
        }
    }
}

fn sort_and_dedup_symbols(symbols: &mut Vec<SymbolMetadata>) {
    symbols.sort_by(|left, right| {
        left.symbol
            .cmp(&right.symbol)
            .then(left.market.cmp(&right.market))
    });
    symbols.dedup_by(|left, right| left.symbol == right.symbol && left.market == right.market);
}

#[cfg(test)]
mod tests {
    use super::{
        record_public_symbol_sync_result, run_public_symbol_sync_once, SymbolSyncRuntimeState,
    };
    use crate::test_support::env_lock;
    use shared_binance::CredentialValidationError;
    use std::{
        collections::VecDeque,
        io::{Read, Write},
        net::TcpListener,
        sync::{Arc, Mutex},
        thread,
    };

    #[test]
    fn public_symbol_sync_returns_three_market_catalog() {
        let state = SymbolSyncRuntimeState::default();
        assert_eq!(state.run_count, 0);

        let symbols = run_public_symbol_sync_once().expect("public symbol sync");

        assert_eq!(symbols.len(), 6);
        assert!(symbols.iter().any(|symbol| symbol.market == "spot"));
        assert!(symbols.iter().any(|symbol| symbol.market == "usdm"));
        assert!(symbols.iter().any(|symbol| symbol.market == "coinm"));
    }

    #[test]
    fn public_symbol_sync_surfaces_upstream_exchange_info_failures() {
        let _guard = env_lock().lock().expect("env lock");
        let server = spawn_test_server(vec![
            TestRoute {
                path_prefix: "/api/v3/exchangeInfo",
                status_line: "HTTP/1.1 200 OK",
                body: r#"{"symbols":[]}"#.to_string(),
            },
            TestRoute {
                path_prefix: "/fapi/v1/exchangeInfo",
                status_line: "HTTP/1.1 503 Service Unavailable",
                body: r#"{"code":-1001,"msg":"usdm unavailable"}"#.to_string(),
            },
        ]);
        std::env::set_var("BINANCE_LIVE_MODE", "1");
        std::env::set_var("BINANCE_SPOT_REST_BASE_URL", &server.base_url);
        std::env::set_var("BINANCE_USDM_REST_BASE_URL", &server.base_url);
        std::env::set_var("BINANCE_COINM_REST_BASE_URL", &server.base_url);

        let error = run_public_symbol_sync_once().expect_err("sync should fail");

        assert!(error.to_string().contains("503"));
        std::env::remove_var("BINANCE_LIVE_MODE");
        std::env::remove_var("BINANCE_SPOT_REST_BASE_URL");
        std::env::remove_var("BINANCE_USDM_REST_BASE_URL");
        std::env::remove_var("BINANCE_COINM_REST_BASE_URL");
    }

    #[test]
    fn failed_public_symbol_sync_attempts_are_recorded_without_fake_success() {
        let mut state = SymbolSyncRuntimeState {
            run_count: 3,
            failure_count: 0,
            last_run_at: None,
            last_failure_at: None,
            last_synced_symbols: 6,
            last_error: None,
        };
        let failure = Err(CredentialValidationError::new("boom"));

        record_public_symbol_sync_result(&mut state, &failure);

        assert_eq!(state.run_count, 4);
        assert_eq!(state.failure_count, 1);
        assert_eq!(state.last_synced_symbols, 6);
        assert_eq!(state.last_error.as_deref(), Some("boom"));
        assert!(state.last_run_at.is_some());
        assert!(state.last_failure_at.is_some());
    }

    #[derive(Clone)]
    struct TestRoute {
        path_prefix: &'static str,
        status_line: &'static str,
        body: String,
    }

    struct TestServer {
        base_url: String,
        join_handle: Option<thread::JoinHandle<()>>,
    }

    impl Drop for TestServer {
        fn drop(&mut self) {
            if let Some(handle) = self.join_handle.take() {
                handle
                    .join()
                    .expect("symbol sync test server thread should exit cleanly");
            }
        }
    }

    fn spawn_test_server(routes: Vec<TestRoute>) -> TestServer {
        let listener = TcpListener::bind("127.0.0.1:0").expect("bind local test server");
        let address = listener.local_addr().expect("test server address");
        let queue = Arc::new(Mutex::new(VecDeque::from(routes)));
        let queue_for_thread = queue.clone();
        let join_handle = thread::spawn(move || {
            while let Some(route) = queue_for_thread
                .lock()
                .expect("route queue poisoned")
                .pop_front()
            {
                let (mut stream, _) = listener.accept().expect("accept test request");
                let mut buffer = [0u8; 4096];
                let read = stream.read(&mut buffer).expect("read test request");
                let request = String::from_utf8_lossy(&buffer[..read]);
                let path = request
                    .lines()
                    .next()
                    .and_then(|line| line.split_whitespace().nth(1))
                    .expect("request path");
                assert!(
                    path.starts_with(route.path_prefix),
                    "expected path prefix {} but received {}",
                    route.path_prefix,
                    path
                );
                let response = format!(
                    "{}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{}",
                    route.status_line,
                    route.body.len(),
                    route.body
                );
                stream
                    .write_all(response.as_bytes())
                    .expect("write test response");
            }
        });

        TestServer {
            base_url: format!("http://{}", address),
            join_handle: Some(join_handle),
        }
    }
}
