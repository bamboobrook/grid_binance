use rust_decimal::Decimal;
use shared_binance::BinanceExecutionUpdate;
use shared_db::SharedDb;
use shared_domain::strategy::{
    GridGeneration, GridLevel, PostTriggerAction, Strategy, StrategyAmountMode,
    StrategyMarket, StrategyMode, StrategyRevision, StrategyRuntime, StrategyRuntimeOrder,
    StrategyStatus,
};
use trading_engine::execution_effects::persist_execution_effects;
use trading_engine::execution_sync::apply_execution_update;

#[test]
fn execution_effects_persist_trade_history_and_notifications_once() {
    let _guard = env_lock().lock().unwrap();
    std::env::remove_var("TELEGRAM_BOT_TOKEN");
    std::env::remove_var("TELEGRAM_API_BASE_URL");
    let db = SharedDb::ephemeral().expect("db");
    let mut strategy = sample_strategy();
    let update = BinanceExecutionUpdate {
        market: "usdm".to_string(),
        symbol: "BTCUSDT".to_string(),
        order_id: "999".to_string(),
        client_order_id: Some("strategy-1-order-0".to_string()),
        side: Some("SELL".to_string()),
        order_type: Some("LIMIT".to_string()),
        status: "FILLED".to_string(),
        execution_type: Some("TRADE".to_string()),
        order_price: Some("43000".to_string()),
        last_fill_price: Some("43000".to_string()),
        last_fill_quantity: Some("0.001".to_string()),
        cumulative_fill_quantity: Some("0.001".to_string()),
        fee_amount: Some("0.04".to_string()),
        fee_asset: Some("USDT".to_string()),
        position_side: Some("SHORT".to_string()),
        trade_id: Some("321".to_string()),
        realized_profit: Some("1.25".to_string()),
        event_time_ms: 1_710_002,
    };

    assert!(apply_execution_update(&mut strategy, &update));
    let first = persist_execution_effects(&db, &strategy, &update).expect("first persist");
    let second = persist_execution_effects(&db, &strategy, &update).expect("second persist");

    assert_eq!(first.new_trades, 1);
    assert_eq!(second.new_trades, 0);
    let trades = db.list_exchange_trade_history("trader@example.com").unwrap();
    assert_eq!(trades.len(), 1);
    assert_eq!(trades[0].trade_id, "321");
    let notifications = db.list_notification_logs("trader@example.com", 10).unwrap();
    assert!(notifications.iter().any(|record| record.template_key.as_deref() == Some("GridFillExecuted")));
    let profit_log = notifications
        .iter()
        .find(|record| record.template_key.as_deref() == Some("FillProfitReported") && record.channel == "in_app")
        .expect("fill profit log");
    assert_eq!(profit_log.payload["cumulative_net_pnl"], "1.21");
}

#[test]
fn execution_effects_emit_telegram_logs_when_bound_and_configured() {
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
    let mut strategy = sample_strategy();
    let update = BinanceExecutionUpdate {
        market: "usdm".to_string(),
        symbol: "BTCUSDT".to_string(),
        order_id: "999".to_string(),
        client_order_id: Some("strategy-1-order-0".to_string()),
        side: Some("SELL".to_string()),
        order_type: Some("LIMIT".to_string()),
        status: "FILLED".to_string(),
        execution_type: Some("TRADE".to_string()),
        order_price: Some("43000".to_string()),
        last_fill_price: Some("43000".to_string()),
        last_fill_quantity: Some("0.001".to_string()),
        cumulative_fill_quantity: Some("0.001".to_string()),
        fee_amount: Some("0.04".to_string()),
        fee_asset: Some("USDT".to_string()),
        position_side: Some("SHORT".to_string()),
        trade_id: Some("322".to_string()),
        realized_profit: Some("1.25".to_string()),
        event_time_ms: 1_710_003,
    };

    assert!(apply_execution_update(&mut strategy, &update));
    persist_execution_effects(&db, &strategy, &update).unwrap();

    let notifications = db.list_notification_logs("trader@example.com", 10).unwrap();
    assert!(notifications.iter().any(|record| record.channel == "telegram" && record.template_key.as_deref() == Some("GridFillExecuted") && record.status == "delivered"));
    assert!(notifications.iter().any(|record| record.channel == "telegram" && record.template_key.as_deref() == Some("FillProfitReported") && record.status == "delivered"));
}

#[test]
fn execution_effects_record_failed_telegram_logs_when_binding_exists_without_bot_token() {
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
    let mut strategy = sample_strategy();
    let update = BinanceExecutionUpdate {
        market: "usdm".to_string(),
        symbol: "BTCUSDT".to_string(),
        order_id: "999".to_string(),
        client_order_id: Some("strategy-1-order-0".to_string()),
        side: Some("SELL".to_string()),
        order_type: Some("LIMIT".to_string()),
        status: "FILLED".to_string(),
        execution_type: Some("TRADE".to_string()),
        order_price: Some("43000".to_string()),
        last_fill_price: Some("43000".to_string()),
        last_fill_quantity: Some("0.001".to_string()),
        cumulative_fill_quantity: Some("0.001".to_string()),
        fee_amount: Some("0.04".to_string()),
        fee_asset: Some("USDT".to_string()),
        position_side: Some("SHORT".to_string()),
        trade_id: Some("323".to_string()),
        realized_profit: Some("1.25".to_string()),
        event_time_ms: 1_710_004,
    };

    assert!(apply_execution_update(&mut strategy, &update));
    persist_execution_effects(&db, &strategy, &update).unwrap();

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
        market: StrategyMarket::FuturesUsdM,
        mode: StrategyMode::FuturesShort,
        draft_revision: StrategyRevision {
            revision_id: "rev-1".to_string(),
            version: 1,
            generation: GridGeneration::Custom,
            amount_mode: StrategyAmountMode::Quote,
            futures_margin_mode: None,
            leverage: None,
            levels: vec![GridLevel {
                level_index: 0,
                entry_price: Decimal::new(43000, 0),
                quantity: Decimal::new(1, 3),
                take_profit_bps: 100,
                trailing_bps: None,
            }],
            overall_take_profit_bps: None,
            overall_stop_loss_bps: None,
            post_trigger_action: PostTriggerAction::Stop,
        },
        active_revision: None,
        runtime: StrategyRuntime {
            positions: Vec::new(),
            orders: vec![StrategyRuntimeOrder {
                order_id: "strategy-1-order-0".to_string(),
                exchange_order_id: Some("999".to_string()),
                level_index: Some(0),
                side: "Sell".to_string(),
                order_type: "Limit".to_string(),
                price: Some(Decimal::new(43000, 0)),
                quantity: Decimal::new(1, 3),
                status: "Filled".to_string(),
            }],
            fills: Vec::new(),
            events: Vec::new(),
            last_preflight: None,
        },
        archived_at: None,
    }
}
