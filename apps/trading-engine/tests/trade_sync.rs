use rust_decimal::Decimal;
use shared_binance::BinanceUserTrade;
use shared_db::SharedDb;
use shared_domain::strategy::{
    GridGeneration, GridLevel, PostTriggerAction, Strategy, StrategyAmountMode, StrategyMarket,
    StrategyMode,
    StrategyRevision, StrategyRuntime, StrategyRuntimeOrder, StrategyStatus,
};
use trading_engine::trade_sync::{sync_strategy_trades, BinanceTradeGateway};

struct FakeTradeGateway {
    trades: Vec<BinanceUserTrade>,
}

impl BinanceTradeGateway for FakeTradeGateway {
    fn user_trades(
        &self,
        _market: &str,
        _symbol: &str,
        _limit: usize,
    ) -> Result<Vec<BinanceUserTrade>, String> {
        Ok(self.trades.clone())
    }
}

#[test]
fn trade_sync_records_new_exchange_fill_and_notification() {
    let db = SharedDb::ephemeral().expect("db");
    let gateway = FakeTradeGateway {
        trades: vec![BinanceUserTrade {
            market: "spot".to_string(),
            trade_id: "1001".to_string(),
            order_id: Some("98765".to_string()),
            symbol: "BTCUSDT".to_string(),
            side: "BUY".to_string(),
            price: "42000".to_string(),
            quantity: "0.001".to_string(),
            fee_amount: Some("0.05".to_string()),
            fee_asset: Some("USDT".to_string()),
            realized_profit: None,
            traded_at_ms: 1_710_000_000_123,
        }],
    };
    let mut strategy = sample_strategy();

    let result = sync_strategy_trades(&db, &mut strategy, &gateway).expect("sync trades");

    assert_eq!(result.new_fills, 1);
    assert_eq!(strategy.runtime.orders[0].status, "Filled");
    assert_eq!(strategy.runtime.fills.len(), 1);
    assert_eq!(strategy.runtime.fills[0].fill_id, "exchange-trade-1001");
    assert_eq!(strategy.runtime.fills[0].fee_amount, Some(Decimal::new(5, 2)));
    let trade_history = db.list_exchange_trade_history("trader@example.com").expect("history");
    assert_eq!(trade_history.len(), 1);
    assert_eq!(trade_history[0].trade_id, "1001");
    let notifications = db.list_notification_logs("trader@example.com", 10).expect("notifications");
    assert_eq!(notifications.len(), 2);
    assert!(notifications.iter().any(|record| record.template_key.as_deref() == Some("GridFillExecuted")));
    assert!(notifications.iter().any(|record| record.template_key.as_deref() == Some("FillProfitReported")));
}

#[test]
fn fill_profit_notification_includes_running_cumulative_net_pnl() {
    let db = SharedDb::ephemeral().expect("db");
    let gateway = FakeTradeGateway {
        trades: vec![BinanceUserTrade {
            market: "spot".to_string(),
            trade_id: "1003".to_string(),
            order_id: Some("98765".to_string()),
            symbol: "BTCUSDT".to_string(),
            side: "BUY".to_string(),
            price: "42000".to_string(),
            quantity: "0.001".to_string(),
            fee_amount: Some("0.05".to_string()),
            fee_asset: Some("USDT".to_string()),
            realized_profit: None,
            traded_at_ms: 1_710_000_000_125,
        }],
    };
    let mut strategy = sample_strategy();
    strategy.runtime.fills.push(shared_domain::strategy::StrategyRuntimeFill {
        fill_id: "existing-fill".to_string(),
        order_id: Some("old-order".to_string()),
        level_index: Some(0),
        fill_type: "ExchangeFill".to_string(),
        price: Decimal::new(41000, 0),
        quantity: Decimal::new(1, 3),
        realized_pnl: Some(Decimal::new(1, 1)),
        fee_amount: Some(Decimal::new(2, 2)),
        fee_asset: Some("USDT".to_string()),
    });

    sync_strategy_trades(&db, &mut strategy, &gateway).expect("sync trades");

    let notifications = db.list_notification_logs("trader@example.com", 10).unwrap();
    let pnl = notifications
        .iter()
        .find(|record| record.template_key.as_deref() == Some("FillProfitReported"))
        .expect("fill profit record");
    let payload = pnl.payload.clone();
    assert_eq!(payload["net_pnl"], "-0.05");
    assert_eq!(payload["cumulative_net_pnl"], "0.03");
}

#[test]
fn trade_sync_uses_realized_profit_from_exchange_trade_payload() {
    let db = SharedDb::ephemeral().expect("db");
    let gateway = FakeTradeGateway {
        trades: vec![BinanceUserTrade {
            market: "usdm".to_string(),
            trade_id: "profit-1001".to_string(),
            order_id: Some("98765".to_string()),
            symbol: "BTCUSDT".to_string(),
            side: "SELL".to_string(),
            price: "42000".to_string(),
            quantity: "0.001".to_string(),
            fee_amount: Some("0.05".to_string()),
            fee_asset: Some("USDT".to_string()),
            realized_profit: Some("1.25".to_string()),
            traded_at_ms: 1_710_000_000_127,
        }],
    };
    let mut strategy = sample_strategy();

    sync_strategy_trades(&db, &mut strategy, &gateway).expect("sync trades");

    assert_eq!(strategy.runtime.fills[0].realized_pnl, Some(Decimal::new(125, 2)));
    let notifications = db.list_notification_logs("trader@example.com", 10).unwrap();
    let pnl = notifications
        .iter()
        .find(|record| record.template_key.as_deref() == Some("FillProfitReported"))
        .expect("fill profit record");
    assert_eq!(pnl.payload["realized_pnl"], "1.25");
    assert_eq!(pnl.payload["net_pnl"], "1.2");
    assert_eq!(pnl.payload["cumulative_net_pnl"], "1.2");
}

#[test]
fn trade_sync_dedupes_existing_trade_ids() {
    let db = SharedDb::ephemeral().expect("db");
    let gateway = FakeTradeGateway {
        trades: vec![BinanceUserTrade {
            market: "spot".to_string(),
            trade_id: "1001".to_string(),
            order_id: Some("98765".to_string()),
            symbol: "BTCUSDT".to_string(),
            side: "BUY".to_string(),
            price: "42000".to_string(),
            quantity: "0.001".to_string(),
            fee_amount: Some("0.05".to_string()),
            fee_asset: Some("USDT".to_string()),
            realized_profit: None,
            traded_at_ms: 1_710_000_000_123,
        }],
    };
    let mut strategy = sample_strategy();

    let first = sync_strategy_trades(&db, &mut strategy, &gateway).expect("first sync");
    let second = sync_strategy_trades(&db, &mut strategy, &gateway).expect("second sync");

    assert_eq!(first.new_fills, 1);
    assert_eq!(second.new_fills, 0);
    assert_eq!(strategy.runtime.fills.len(), 1);
    assert_eq!(db.list_exchange_trade_history("trader@example.com").unwrap().len(), 1);
}

#[test]
fn trade_sync_sends_telegram_for_bound_user_when_configured() {
    let _guard = env_lock().lock().unwrap();
    let _token = EnvGuard::set("TELEGRAM_BOT_TOKEN", "bot-test-token");
    let server = TestServer::start(vec![
        TestRoute {
            path_prefix: "/botbot-test-token/sendMessage",
            status_line: "HTTP/1.1 200 OK",
            body: r#"{"ok":true,"result":{"message_id":1}}"#,
        },
        TestRoute {
            path_prefix: "/botbot-test-token/sendMessage",
            status_line: "HTTP/1.1 200 OK",
            body: r#"{"ok":true,"result":{"message_id":2}}"#,
        },
    ]);
    let _base = EnvGuard::set("TELEGRAM_API_BASE_URL", &server.base_url);
    let db = SharedDb::ephemeral().expect("db");
    db.upsert_telegram_binding(&shared_db::TelegramBindingRecord {
        user_email: "trader@example.com".to_string(),
        telegram_user_id: "tg-1".to_string(),
        telegram_chat_id: "chat-1".to_string(),
        bound_at: chrono::Utc::now(),
    }).unwrap();
    let gateway = FakeTradeGateway {
        trades: vec![BinanceUserTrade {
            market: "spot".to_string(),
            trade_id: "1002".to_string(),
            order_id: Some("98765".to_string()),
            symbol: "BTCUSDT".to_string(),
            side: "BUY".to_string(),
            price: "42000".to_string(),
            quantity: "0.001".to_string(),
            fee_amount: Some("0.05".to_string()),
            fee_asset: Some("USDT".to_string()),
            realized_profit: None,
            traded_at_ms: 1_710_000_000_124,
        }],
    };
    let mut strategy = sample_strategy();

    let result = sync_strategy_trades(&db, &mut strategy, &gateway).expect("sync trades");

    assert_eq!(result.new_fills, 1);
    let notifications = db.list_notification_logs("trader@example.com", 10).unwrap();
    assert!(notifications.iter().any(|record| record.channel == "telegram" && record.template_key.as_deref() == Some("GridFillExecuted") && record.status == "delivered"));
    assert!(notifications.iter().any(|record| record.channel == "telegram" && record.template_key.as_deref() == Some("FillProfitReported") && record.status == "delivered"));
}

#[test]
fn trade_sync_records_failed_telegram_logs_when_binding_exists_without_bot_token() {
    let _guard = env_lock().lock().unwrap();
    std::env::remove_var("TELEGRAM_BOT_TOKEN");
    std::env::remove_var("TELEGRAM_API_BASE_URL");
    std::env::remove_var("TELEGRAM_BOT_TOKEN");
    let db = SharedDb::ephemeral().expect("db");
    db.upsert_telegram_binding(&shared_db::TelegramBindingRecord {
        user_email: "trader@example.com".to_string(),
        telegram_user_id: "tg-1".to_string(),
        telegram_chat_id: "chat-1".to_string(),
        bound_at: chrono::Utc::now(),
    }).unwrap();
    let gateway = FakeTradeGateway {
        trades: vec![BinanceUserTrade {
            market: "spot".to_string(),
            trade_id: "1004".to_string(),
            order_id: Some("98765".to_string()),
            symbol: "BTCUSDT".to_string(),
            side: "BUY".to_string(),
            price: "42000".to_string(),
            quantity: "0.001".to_string(),
            fee_amount: Some("0.05".to_string()),
            fee_asset: Some("USDT".to_string()),
            realized_profit: None,
            traded_at_ms: 1_710_000_000_126,
        }],
    };
    let mut strategy = sample_strategy();

    sync_strategy_trades(&db, &mut strategy, &gateway).unwrap();

    let notifications = db.list_notification_logs("trader@example.com", 10).unwrap();
    assert!(notifications.iter().any(|record| record.channel == "telegram" && record.template_key.as_deref() == Some("GridFillExecuted") && record.status == "failed"));
    assert!(notifications.iter().any(|record| record.channel == "telegram" && record.template_key.as_deref() == Some("FillProfitReported") && record.status == "failed"));
}

#[derive(Clone)]
struct TestRoute {
    path_prefix: &'static str,
    status_line: &'static str,
    body: &'static str,
}

struct TestServer {
    base_url: String,
    join_handle: Option<std::thread::JoinHandle<()>>,
}

impl TestServer {
    fn start(routes: Vec<TestRoute>) -> Self {
        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind test server");
        let address = listener.local_addr().unwrap();
        let queue = std::sync::Arc::new(std::sync::Mutex::new(std::collections::VecDeque::from(routes)));
        let queue_for_thread = queue.clone();
        let join_handle = std::thread::spawn(move || {
            while let Some(route) = queue_for_thread.lock().unwrap().pop_front() {
                let (mut stream, _) = listener.accept().unwrap();
                let mut buffer = [0u8; 4096];
                let read = std::io::Read::read(&mut stream, &mut buffer).unwrap();
                let request = String::from_utf8_lossy(&buffer[..read]);
                let path = request.lines().next().and_then(|line| line.split_whitespace().nth(1)).unwrap();
                assert!(path.starts_with(route.path_prefix), "expected path prefix {} but received {}", route.path_prefix, path);
                let response = format!(
                    "{}
content-type: application/json
content-length: {}
connection: close

{}",
                    route.status_line,
                    route.body.len(),
                    route.body,
                );
                std::io::Write::write_all(&mut stream, response.as_bytes()).unwrap();
            }
        });
        Self { base_url: format!("http://{}", address), join_handle: Some(join_handle) }
    }
}

impl Drop for TestServer {
    fn drop(&mut self) {
        if let Some(handle) = self.join_handle.take() {
            handle.join().unwrap();
        }
    }
}

fn env_lock() -> &'static std::sync::Mutex<()> {
    static LOCK: std::sync::OnceLock<std::sync::Mutex<()>> = std::sync::OnceLock::new();
    LOCK.get_or_init(|| std::sync::Mutex::new(()))
}

struct EnvGuard {
    key: &'static str,
    previous: Option<String>,
}

impl EnvGuard {
    fn set(key: &'static str, value: impl Into<String>) -> Self {
        let previous = std::env::var(key).ok();
        std::env::set_var(key, value.into());
        Self { key, previous }
    }
}

impl Drop for EnvGuard {
    fn drop(&mut self) {
        match &self.previous {
            Some(value) => std::env::set_var(self.key, value),
            None => std::env::remove_var(self.key),
        }
    }
}

fn sample_strategy() -> Strategy {
    Strategy {
        id: "strategy-1".to_string(),
        owner_email: "trader@example.com".to_string(),
        name: "Grid".to_string(),
        symbol: "BTCUSDT".to_string(),
        budget: "1000".to_string(),
        grid_spacing_bps: 100,
        status: StrategyStatus::Running,
        source_template_id: None,
        membership_ready: true,
        exchange_ready: true,
        permissions_ready: true,
        withdrawals_disabled: true,
        hedge_mode_ready: true,
        symbol_ready: true,
        filters_ready: true,
        margin_ready: true,
        conflict_ready: true,
        balance_ready: true,
        market: StrategyMarket::Spot,
        mode: StrategyMode::SpotClassic,
        draft_revision: revision(),
        active_revision: Some(revision()),
        runtime: StrategyRuntime {
            positions: Vec::new(),
            orders: vec![StrategyRuntimeOrder {
                order_id: "strategy-1-order-0".to_string(),
                exchange_order_id: Some("98765".to_string()),
                level_index: Some(0),
                side: "Buy".to_string(),
                order_type: "Limit".to_string(),
                price: Some(Decimal::new(42000, 0)),
                quantity: Decimal::new(1, 3),
                status: "Placed".to_string(),
            }],
            fills: Vec::new(),
            events: Vec::new(),
            last_preflight: None,
        },
        archived_at: None,
    }
}

fn revision() -> StrategyRevision {
    StrategyRevision {
        revision_id: "rev-1".to_string(),
        version: 1,
        generation: GridGeneration::Custom,
        amount_mode: StrategyAmountMode::Quote,
        futures_margin_mode: None,
        leverage: None,
        levels: vec![GridLevel {
            level_index: 0,
            entry_price: Decimal::new(42000, 0),
            quantity: Decimal::new(1, 3),
            take_profit_bps: 100,
            trailing_bps: None,
        }],
        overall_take_profit_bps: None,
        overall_stop_loss_bps: None,
        post_trigger_action: PostTriggerAction::Stop,
    }
}
