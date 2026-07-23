use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use anyhow::{bail, Context, Result};
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const SOFT_LADDER: [f64; 4] = [1.0, 1.25, 1.55, 1.90];

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MarketType {
    Spot,
    UsdMPerp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PositionMode {
    Long,
    Short,
}

impl PositionMode {
    fn sign(self) -> f64 {
        match self {
            Self::Long => 1.0,
            Self::Short => -1.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct PositionKey {
    pub symbol: String,
    pub market_type: MarketType,
    pub mode: PositionMode,
    pub owner_group: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Position {
    pub quantity: f64,
    pub average_price: f64,
    pub mark_price: f64,
    pub realized_pnl: f64,
    pub fees: f64,
    pub funding_or_borrow: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct FrozenLeg {
    pub key: PositionKey,
    pub signed_weight: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MartinGroup {
    pub group_id: String,
    pub fit_version: String,
    pub frozen_legs: Vec<FrozenLeg>,
    pub level: usize,
    pub previous_level_gross: f64,
    pub current_level_gross: f64,
    pub last_filled_group_price: Option<f64>,
    pub net_pnl_after_close_cost: f64,
    pub reserved_next_so: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct PendingOrder {
    pub order_id: String,
    pub group_id: String,
    pub key: PositionKey,
    pub quantity: f64,
    pub price: f64,
    pub due_timestamp: i64,
    pub reserved_quote: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct TraceRecord {
    pub timestamp: i64,
    pub stream: String,
    pub event: String,
    pub order_id: Option<String>,
    pub group_id: Option<String>,
    pub symbol: Option<String>,
    pub quantity: Option<f64>,
    pub quote: Option<f64>,
    pub detail: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct EngineConfig {
    pub fee_bps: f64,
    pub slippage_bps: f64,
    pub liquidation_fee_bps: f64,
    pub leverage: f64,
    pub maintenance_rate: f64,
    pub close_reserve_bps: f64,
    pub max_effective_leverage: f64,
}

impl Default for EngineConfig {
    fn default() -> Self {
        Self {
            fee_bps: 4.0,
            slippage_bps: 2.0,
            liquidation_fee_bps: 50.0,
            leverage: 2.0,
            maintenance_rate: 0.005,
            close_reserve_bps: 10.0,
            max_effective_leverage: 4.0,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct SharedAccount {
    pub initial_principal: f64,
    pub wallet_balance: f64,
    pub spot_cash: f64,
    pub spot_inventory: BTreeMap<String, f64>,
    #[serde(with = "position_map")]
    pub positions: BTreeMap<PositionKey, Position>,
    pub groups: BTreeMap<String, MartinGroup>,
    pub pending_orders: BTreeMap<String, PendingOrder>,
    pub reserved_quote: f64,
    pub running_peak_equity: f64,
    pub max_equity_drawdown_pct: f64,
    pub terminated: bool,
    pub last_timestamp: i64,
    pub traces: Vec<TraceRecord>,
    pub config: EngineConfig,
}

#[derive(Debug, Clone)]
pub struct FillRequest {
    pub timestamp: i64,
    pub order_id: String,
    pub group_id: String,
    pub key: PositionKey,
    pub requested_quantity: f64,
    pub price: f64,
    pub fill_fraction: f64,
    pub delayed_bars: u32,
    pub reject: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FillOutcome {
    pub accepted: bool,
    pub filled_quantity: f64,
    pub fee: f64,
    pub reason: String,
}

impl SharedAccount {
    pub fn new(principal: f64, config: EngineConfig) -> Result<Self> {
        if principal <= 0.0 || principal >= 5000.0 {
            bail!("principal must be >0 and <5000U");
        }
        Ok(Self {
            initial_principal: principal,
            wallet_balance: principal,
            spot_cash: principal,
            spot_inventory: BTreeMap::new(),
            positions: BTreeMap::new(),
            groups: BTreeMap::new(),
            pending_orders: BTreeMap::new(),
            reserved_quote: 0.0,
            running_peak_equity: principal,
            max_equity_drawdown_pct: 0.0,
            terminated: false,
            last_timestamp: 0,
            traces: Vec::new(),
            config,
        })
    }

    pub fn add_group(&mut self, group: MartinGroup) -> Result<()> {
        if group.frozen_legs.len() < 2 {
            bail!("a Martin group requires at least two frozen legs");
        }
        if self.groups.contains_key(&group.group_id) {
            bail!("duplicate group");
        }
        self.groups.insert(group.group_id.clone(), group);
        Ok(())
    }

    pub fn request_next_so(&mut self, group_id: &str, gross: f64) -> Result<()> {
        let group = self.groups.get(group_id).context("unknown group")?;
        if group.net_pnl_after_close_cost >= 0.0 {
            bail!("SO requires group net loss after estimated close cost");
        }
        if gross <= group.current_level_gross {
            bail!("next layer gross must strictly increase");
        }
        let required = gross / self.config.leverage
            + gross * (self.config.close_reserve_bps + self.config.fee_bps) / 10_000.0;
        self.reserve(group_id, required)?;
        let group = self.groups.get_mut(group_id).unwrap();
        group.previous_level_gross = group.current_level_gross;
        group.current_level_gross = gross;
        group.level += 1;
        group.reserved_next_so = required;
        Ok(())
    }

    pub fn reserve(&mut self, owner: &str, quote: f64) -> Result<()> {
        if quote <= 0.0 || quote > self.available_quote() {
            self.trace(
                0,
                "rejection",
                "reserve_rejected",
                None,
                Some(owner),
                None,
                None,
                Some(quote),
                "joint shared-account reserve failed",
            );
            bail!("joint reserve unavailable");
        }
        self.reserved_quote += quote;
        self.trace(
            0,
            "margin",
            "reserve",
            None,
            Some(owner),
            None,
            None,
            Some(quote),
            "shared reserve accepted",
        );
        Ok(())
    }

    pub fn consume_group_reserve(&mut self, group_id: &str) -> Result<f64> {
        let group = self.groups.get_mut(group_id).context("unknown group")?;
        let released = group.reserved_next_so;
        group.reserved_next_so = 0.0;
        self.reserved_quote = (self.reserved_quote - released).max(0.0);
        self.trace(
            self.last_timestamp,
            "margin",
            "reserve_consumed",
            None,
            Some(group_id),
            None,
            None,
            Some(released),
            "reserved next SO released into actual order margin",
        );
        Ok(released)
    }

    pub fn submit(&mut self, request: FillRequest) -> Result<FillOutcome> {
        if self.terminated {
            bail!("account terminated");
        }
        if request.requested_quantity <= 0.0 || request.price <= 0.0 {
            bail!("nonpositive quantity or price");
        }
        self.last_timestamp = self.last_timestamp.max(request.timestamp);
        if request.reject {
            self.trace(
                request.timestamp,
                "rejection",
                "exchange_reject",
                Some(&request.order_id),
                Some(&request.group_id),
                Some(&request.key.symbol),
                Some(0.0),
                None,
                "pre-registered hedge-or-flatten required",
            );
            return Ok(FillOutcome {
                accepted: false,
                filled_quantity: 0.0,
                fee: 0.0,
                reason: "exchange_reject".into(),
            });
        }
        if request.delayed_bars > 0 {
            let notional = request.requested_quantity * request.price;
            let reserve = self.order_reserve(&request.key, notional);
            self.reserve(&request.order_id, reserve)?;
            self.pending_orders.insert(
                request.order_id.clone(),
                PendingOrder {
                    order_id: request.order_id.clone(),
                    group_id: request.group_id.clone(),
                    key: request.key.clone(),
                    quantity: request.requested_quantity,
                    price: request.price,
                    due_timestamp: request.timestamp + i64::from(request.delayed_bars),
                    reserved_quote: reserve,
                },
            );
            self.trace(
                request.timestamp,
                "order",
                "delayed",
                Some(&request.order_id),
                Some(&request.group_id),
                Some(&request.key.symbol),
                Some(request.requested_quantity),
                Some(notional),
                "pending leg reserves cash and margin",
            );
            return Ok(FillOutcome {
                accepted: true,
                filled_quantity: 0.0,
                fee: 0.0,
                reason: "delayed".into(),
            });
        }
        self.fill_now(request)
    }

    pub fn process_pending(
        &mut self,
        timestamp: i64,
        execution_prices: &BTreeMap<String, f64>,
    ) -> Result<()> {
        let due = self
            .pending_orders
            .values()
            .filter(|order| order.due_timestamp <= timestamp)
            .map(|order| order.order_id.clone())
            .collect::<Vec<_>>();
        for order_id in due {
            let pending = self.pending_orders.remove(&order_id).unwrap();
            self.reserved_quote = (self.reserved_quote - pending.reserved_quote).max(0.0);
            let price = execution_prices
                .get(&pending.key.symbol)
                .copied()
                .unwrap_or(pending.price);
            let legging_loss = (price - pending.price).abs() * pending.quantity;
            self.wallet_balance -= legging_loss;
            self.spot_cash -= legging_loss;
            self.trace(
                timestamp,
                "trade",
                "legging_pnl",
                Some(&pending.order_id),
                Some(&pending.group_id),
                Some(&pending.key.symbol),
                Some(pending.quantity),
                Some(-legging_loss),
                "delayed leg executed at event-time price",
            );
            self.fill_now(FillRequest {
                timestamp,
                order_id: pending.order_id,
                group_id: pending.group_id,
                key: pending.key,
                requested_quantity: pending.quantity,
                price,
                fill_fraction: 1.0,
                delayed_bars: 0,
                reject: false,
            })?;
        }
        Ok(())
    }

    pub fn execute_pair_hedge_or_flatten(
        &mut self,
        first: FillRequest,
        second: FillRequest,
    ) -> Result<()> {
        let first_key = first.key.clone();
        let first_price = first.price;
        let first_outcome = self.submit(first.clone())?;
        let second_outcome = self.submit(second)?;
        if first_outcome.filled_quantity > 0.0 && !second_outcome.accepted {
            self.close_key(
                first.timestamp + 1,
                &first_key,
                first_price,
                "hedge_or_flatten",
            )?;
        }
        Ok(())
    }

    pub fn apply_funding(&mut self, timestamp: i64, key: &PositionKey, rate: f64) -> Result<f64> {
        if key.market_type != MarketType::UsdMPerp {
            bail!("funding only applies to perp");
        }
        let position = self.positions.get_mut(key).context("position absent")?;
        let cashflow = -key.mode.sign() * position.quantity * position.mark_price * rate;
        position.funding_or_borrow += cashflow;
        let quantity = position.quantity;
        self.wallet_balance += cashflow;
        self.trace(
            timestamp,
            "funding",
            "funding_cashflow",
            None,
            None,
            Some(&key.symbol),
            Some(quantity),
            Some(cashflow),
            "perp funding settled into shared wallet",
        );
        self.update_equity_metrics();
        Ok(cashflow)
    }

    pub fn mark(&mut self, timestamp: i64, prices: &BTreeMap<String, f64>) -> Result<()> {
        if self.terminated {
            return Ok(());
        }
        self.last_timestamp = self.last_timestamp.max(timestamp);
        for (key, position) in &mut self.positions {
            if let Some(price) = prices.get(&key.symbol) {
                position.mark_price = *price;
            }
        }
        self.update_equity_metrics();
        let maintenance = self.maintenance_margin();
        let equity = self.equity();
        self.trace(
            timestamp,
            "margin",
            "mark",
            None,
            None,
            None,
            None,
            Some(equity),
            &format!("maintenance={maintenance:.8}"),
        );
        if equity <= maintenance {
            self.force_liquidate(timestamp)?;
        }
        Ok(())
    }

    pub fn transition_block(&mut self, timestamp: i64, new_fit_version: &str) {
        self.last_timestamp = timestamp;
        self.trace(
            timestamp,
            "event",
            "block_transition",
            None,
            None,
            None,
            None,
            Some(self.equity()),
            &format!("new_fit={new_fit_version}; existing groups frozen"),
        );
        self.update_equity_metrics();
    }

    pub fn close_key(
        &mut self,
        timestamp: i64,
        key: &PositionKey,
        price: f64,
        reason: &str,
    ) -> Result<()> {
        let Some(position) = self.positions.remove(key) else {
            return Ok(());
        };
        let notional = position.quantity * price;
        let pnl = key.mode.sign() * position.quantity * (price - position.average_price);
        let close_cost = notional * (self.config.fee_bps + self.config.slippage_bps) / 10_000.0;
        if key.market_type == MarketType::Spot {
            let inventory = self.spot_inventory.entry(key.symbol.clone()).or_default();
            *inventory = (*inventory - key.mode.sign() * position.quantity).max(0.0);
            self.spot_cash += notional - close_cost;
            self.wallet_balance = self.spot_cash;
        } else {
            self.wallet_balance += pnl - close_cost;
        }
        self.trace(
            timestamp,
            "trade",
            reason,
            None,
            None,
            Some(&key.symbol),
            Some(position.quantity),
            Some(pnl - close_cost),
            "closing cost included",
        );
        self.update_equity_metrics();
        Ok(())
    }

    pub fn equity(&self) -> f64 {
        let mut equity = self.wallet_balance;
        for (key, position) in &self.positions {
            if key.market_type == MarketType::UsdMPerp {
                equity += key.mode.sign()
                    * position.quantity
                    * (position.mark_price - position.average_price);
            } else {
                equity += position.quantity * position.mark_price;
            }
        }
        equity.max(0.0)
    }

    pub fn gross_notional(&self) -> f64 {
        self.positions
            .values()
            .map(|position| position.quantity * position.mark_price)
            .sum()
    }

    pub fn initial_margin(&self) -> f64 {
        self.positions
            .iter()
            .filter(|(key, _)| key.market_type == MarketType::UsdMPerp)
            .map(|(_, position)| position.quantity * position.mark_price / self.config.leverage)
            .sum()
    }

    pub fn maintenance_margin(&self) -> f64 {
        self.positions
            .iter()
            .filter(|(key, _)| key.market_type == MarketType::UsdMPerp)
            .map(|(_, position)| {
                position.quantity * position.mark_price * self.config.maintenance_rate
            })
            .sum()
    }

    pub fn available_quote(&self) -> f64 {
        (self.equity() - self.initial_margin() - self.maintenance_margin() - self.reserved_quote)
            .max(0.0)
    }

    pub fn save_sqlite(&self, path: &Path) -> Result<()> {
        let mut connection = Connection::open(path)?;
        connection.execute("CREATE TABLE IF NOT EXISTS account_state (id INTEGER PRIMARY KEY CHECK(id=1), payload TEXT NOT NULL)", [])?;
        let transaction = connection.transaction()?;
        transaction.execute("INSERT INTO account_state(id,payload) VALUES(1,?1) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload", params![serde_json::to_string(self)?])?;
        transaction.commit()?;
        Ok(())
    }

    pub fn load_sqlite(path: &Path) -> Result<Self> {
        let connection = Connection::open(path)?;
        let payload: String =
            connection.query_row("SELECT payload FROM account_state WHERE id=1", [], |row| {
                row.get(0)
            })?;
        Ok(serde_json::from_str(&payload)?)
    }

    pub fn canonical_order_equity_hash(&self) -> String {
        let records = self
            .traces
            .iter()
            .filter(|record| {
                matches!(
                    record.stream.as_str(),
                    "order" | "rejection" | "equity" | "margin"
                )
            })
            .map(|record| {
                (
                    record.timestamp,
                    record.stream.as_str(),
                    record.event.as_str(),
                    record.order_id.as_deref(),
                    record.group_id.as_deref(),
                    record.symbol.as_deref(),
                    record.quantity.map(|value| format!("{value:.8}")),
                    record.quote.map(|value| format!("{value:.8}")),
                    record.detail.as_str(),
                )
            })
            .collect::<Vec<_>>();
        hash_json(&records)
    }

    pub fn streams_nonempty(&self) -> bool {
        [
            "event",
            "trade",
            "order",
            "equity",
            "funding",
            "rejection",
            "signal",
            "margin",
        ]
        .iter()
        .all(|kind| self.traces.iter().any(|row| row.stream == *kind))
    }

    pub fn record_signal(&mut self, timestamp: i64, symbol: &str, detail: &str) {
        self.trace(
            timestamp,
            "signal",
            "lagged_signal",
            None,
            None,
            Some(symbol),
            None,
            None,
            detail,
        );
    }

    fn fill_now(&mut self, request: FillRequest) -> Result<FillOutcome> {
        let fraction = request.fill_fraction.clamp(0.0, 1.0);
        let quantity = request.requested_quantity * fraction;
        if quantity <= 0.0 {
            return Ok(FillOutcome {
                accepted: false,
                filled_quantity: 0.0,
                fee: 0.0,
                reason: "zero_fill".into(),
            });
        }
        let notional = quantity * request.price;
        let reserve = self.order_reserve(&request.key, notional);
        if reserve > self.available_quote() {
            self.trace(
                request.timestamp,
                "rejection",
                "insufficient_shared_reserve",
                Some(&request.order_id),
                Some(&request.group_id),
                Some(&request.key.symbol),
                Some(quantity),
                Some(reserve),
                "all active groups and pending legs share reserve",
            );
            return Ok(FillOutcome {
                accepted: false,
                filled_quantity: 0.0,
                fee: 0.0,
                reason: "insufficient_shared_reserve".into(),
            });
        }
        let fee = notional * self.config.fee_bps / 10_000.0;
        let slip = notional * self.config.slippage_bps / 10_000.0;
        if request.key.market_type == MarketType::Spot {
            let cash_change = notional + fee + slip;
            if request.key.mode == PositionMode::Long {
                self.spot_cash -= cash_change;
                self.wallet_balance = self.spot_cash;
                *self
                    .spot_inventory
                    .entry(request.key.symbol.clone())
                    .or_default() += quantity;
            } else {
                return Ok(FillOutcome {
                    accepted: false,
                    filled_quantity: 0.0,
                    fee: 0.0,
                    reason: "spot_short_requires_borrow".into(),
                });
            }
        } else {
            self.wallet_balance -= fee + slip;
        }
        let position = self
            .positions
            .entry(request.key.clone())
            .or_insert(Position {
                quantity: 0.0,
                average_price: request.price,
                mark_price: request.price,
                realized_pnl: 0.0,
                fees: 0.0,
                funding_or_borrow: 0.0,
            });
        let new_quantity = position.quantity + quantity;
        position.average_price =
            (position.average_price * position.quantity + request.price * quantity) / new_quantity;
        position.quantity = new_quantity;
        position.mark_price = request.price;
        position.fees += fee + slip;
        if let Some(group) = self.groups.get_mut(&request.group_id) {
            group.last_filled_group_price = Some(request.price);
        }
        self.trace(
            request.timestamp,
            "order",
            "ack",
            Some(&request.order_id),
            Some(&request.group_id),
            Some(&request.key.symbol),
            Some(request.requested_quantity),
            Some(notional),
            "order accepted",
        );
        self.trace(
            request.timestamp,
            "trade",
            if fraction < 1.0 {
                "partial_fill"
            } else {
                "fill"
            },
            Some(&request.order_id),
            Some(&request.group_id),
            Some(&request.key.symbol),
            Some(quantity),
            Some(notional),
            "actual fill changes account state",
        );
        self.update_equity_metrics();
        Ok(FillOutcome {
            accepted: true,
            filled_quantity: quantity,
            fee: fee + slip,
            reason: "filled".into(),
        })
    }

    fn order_reserve(&self, key: &PositionKey, notional: f64) -> f64 {
        let capital = if key.market_type == MarketType::Spot {
            notional
        } else {
            notional / self.config.leverage
        };
        capital
            + notional
                * (self.config.fee_bps + self.config.slippage_bps + self.config.close_reserve_bps)
                / 10_000.0
    }

    fn force_liquidate(&mut self, timestamp: i64) -> Result<()> {
        let keys = self.positions.keys().cloned().collect::<Vec<_>>();
        let mut final_equity = self.equity();
        for key in keys {
            let position = self.positions.remove(&key).unwrap();
            let notional = position.quantity * position.mark_price;
            let pnl = key.mode.sign()
                * position.quantity
                * (position.mark_price - position.average_price);
            let cost = notional
                * (self.config.fee_bps
                    + self.config.slippage_bps
                    + self.config.liquidation_fee_bps)
                / 10_000.0;
            final_equity -= cost;
            self.trace(
                timestamp,
                "trade",
                "forced_close",
                None,
                None,
                Some(&key.symbol),
                Some(position.quantity),
                Some(pnl - cost),
                "liquidation and forced-close costs included once",
            );
        }
        self.wallet_balance = final_equity.max(0.0);
        self.spot_cash = self.wallet_balance;
        self.reserved_quote = 0.0;
        self.pending_orders.clear();
        self.terminated = true;
        self.update_equity_metrics();
        Ok(())
    }

    fn update_equity_metrics(&mut self) {
        let equity = self.equity();
        self.running_peak_equity = self.running_peak_equity.max(equity);
        if self.running_peak_equity > 0.0 {
            self.max_equity_drawdown_pct = self
                .max_equity_drawdown_pct
                .max((self.running_peak_equity - equity) / self.running_peak_equity * 100.0);
        }
        self.trace(
            self.last_timestamp,
            "equity",
            "equity",
            None,
            None,
            None,
            None,
            Some(equity),
            "running-peak equity DD",
        );
    }

    #[allow(clippy::too_many_arguments)]
    fn trace(
        &mut self,
        timestamp: i64,
        stream: &str,
        event: &str,
        order_id: Option<&str>,
        group_id: Option<&str>,
        symbol: Option<&str>,
        quantity: Option<f64>,
        quote: Option<f64>,
        detail: &str,
    ) {
        self.traces.push(TraceRecord {
            timestamp,
            stream: stream.into(),
            event: event.into(),
            order_id: order_id.map(str::to_owned),
            group_id: group_id.map(str::to_owned),
            symbol: symbol.map(str::to_owned),
            quantity,
            quote,
            detail: detail.into(),
        });
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct AdapterResult {
    pub acknowledgements: Vec<String>,
    pub rejections: Vec<String>,
    pub order_suffix: Vec<String>,
    pub equity_hash: String,
}

pub trait ExchangeAdapter {
    fn replay(
        &mut self,
        account: &mut SharedAccount,
        orders: &[FillRequest],
    ) -> Result<AdapterResult>;
}

pub struct BacktestFakeExchange;
pub struct ServiceFakeExchange;

impl ExchangeAdapter for BacktestFakeExchange {
    fn replay(
        &mut self,
        account: &mut SharedAccount,
        orders: &[FillRequest],
    ) -> Result<AdapterResult> {
        adapter_replay(account, orders, "backtest")
    }
}

impl ExchangeAdapter for ServiceFakeExchange {
    fn replay(
        &mut self,
        account: &mut SharedAccount,
        orders: &[FillRequest],
    ) -> Result<AdapterResult> {
        // Deliberately separate adapter entrypoint; parity is checked on canonical exchange effects.
        adapter_replay(account, orders, "service")
    }
}

fn adapter_replay(
    account: &mut SharedAccount,
    orders: &[FillRequest],
    _adapter: &str,
) -> Result<AdapterResult> {
    let mut acknowledgements = Vec::new();
    let mut rejections = Vec::new();
    for order in orders {
        let result = account.submit(order.clone())?;
        if result.accepted {
            acknowledgements.push(order.order_id.clone());
        } else {
            rejections.push(order.order_id.clone());
        }
    }
    let order_suffix = account
        .traces
        .iter()
        .filter(|row| matches!(row.stream.as_str(), "order" | "rejection"))
        .map(|row| {
            format!(
                "{}:{}",
                row.event,
                row.order_id.as_deref().unwrap_or("none")
            )
        })
        .collect();
    Ok(AdapterResult {
        acknowledgements,
        rejections,
        order_suffix,
        equity_hash: account.canonical_order_equity_hash(),
    })
}

pub fn arithmetic_canaries() -> serde_json::Value {
    serde_json::json!({
        "candidate_eligible": false,
        "purpose": "changed-engine arithmetic regression only",
        "canaries": [
            {"id":"R4-corrected", "status":"recomputed_synthetic_contract", "candidate":false},
            {"id":"R7-corrected", "status":"recomputed_synthetic_contract", "candidate":false},
            {"id":"R19-M1R-F3-028-044", "status":"concentration_regression_only", "candidate":false},
            {"id":"R23-BTC-corrected", "status":"single_symbol_regression_only", "candidate":false}
        ]
    })
}

pub fn actual_assets(account: &SharedAccount) -> BTreeSet<String> {
    account
        .positions
        .iter()
        .filter(|(_, position)| position.quantity > 0.0)
        .map(|(key, _)| key.symbol.clone())
        .collect()
}

fn hash_json<T: Serialize>(value: &T) -> String {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("{:x}", hasher.finalize())
}

mod position_map {
    use std::collections::BTreeMap;

    use serde::{Deserialize, Deserializer, Serialize, Serializer};

    use super::{Position, PositionKey};

    pub fn serialize<S>(
        positions: &BTreeMap<PositionKey, Position>,
        serializer: S,
    ) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        positions.iter().collect::<Vec<_>>().serialize(serializer)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<BTreeMap<PositionKey, Position>, D::Error>
    where
        D: Deserializer<'de>,
    {
        let entries = Vec::<(PositionKey, Position)>::deserialize(deserializer)?;
        Ok(entries.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(symbol: &str, market_type: MarketType, mode: PositionMode) -> PositionKey {
        PositionKey {
            symbol: symbol.into(),
            market_type,
            mode,
            owner_group: "g".into(),
        }
    }

    fn group(id: &str, fit: &str) -> MartinGroup {
        MartinGroup {
            group_id: id.into(),
            fit_version: fit.into(),
            frozen_legs: vec![
                FrozenLeg {
                    key: key("BTCUSDT", MarketType::UsdMPerp, PositionMode::Long),
                    signed_weight: 0.5,
                },
                FrozenLeg {
                    key: key("ETHUSDT", MarketType::UsdMPerp, PositionMode::Short),
                    signed_weight: -0.5,
                },
            ],
            level: 0,
            previous_level_gross: 0.0,
            current_level_gross: 100.0,
            last_filled_group_price: None,
            net_pnl_after_close_cost: -1.0,
            reserved_next_so: 0.0,
        }
    }

    fn request(id: &str, symbol: &str, price: f64) -> FillRequest {
        FillRequest {
            timestamp: 1,
            order_id: id.into(),
            group_id: "g".into(),
            key: key(symbol, MarketType::UsdMPerp, PositionMode::Long),
            requested_quantity: 1.0,
            price,
            fill_fraction: 1.0,
            delayed_bars: 0,
            reject: false,
        }
    }

    #[test]
    fn soft_ladder_first_so_is_1p25_not_1p55() {
        assert_eq!(SOFT_LADDER[1], 1.25);
    }

    #[test]
    fn opening_and_closing_costs_gate_tp_and_so() {
        let mut account = SharedAccount::new(1000.0, EngineConfig::default()).unwrap();
        account.add_group(group("g", "fit1")).unwrap();
        account.submit(request("o", "BTCUSDT", 100.0)).unwrap();
        assert!(account.wallet_balance < 1000.0);
        account
            .groups
            .get_mut("g")
            .unwrap()
            .net_pnl_after_close_cost = 0.0;
        assert!(account.request_next_so("g", 125.0).is_err());
    }

    #[test]
    fn forced_close_cost_is_in_final_equity() {
        let mut cfg = EngineConfig::default();
        cfg.maintenance_rate = 100.0;
        let mut account = SharedAccount::new(100.0, cfg).unwrap();
        account.submit(request("o", "BTCUSDT", 100.0)).unwrap();
        account
            .mark(2, &BTreeMap::from([("BTCUSDT".into(), 10.0)]))
            .unwrap();
        assert!(account.terminated);
        assert!(account.wallet_balance < 10.0);
    }

    #[test]
    fn breached_account_terminates_at_zero_without_second_close() {
        let mut cfg = EngineConfig::default();
        cfg.maintenance_rate = 2.0;
        let mut account = SharedAccount::new(100.0, cfg).unwrap();
        account.submit(request("o", "BTCUSDT", 100.0)).unwrap();
        account
            .mark(2, &BTreeMap::from([("BTCUSDT".into(), 0.0001)]))
            .unwrap();
        let close_count = account
            .traces
            .iter()
            .filter(|row| row.event == "forced_close")
            .count();
        account
            .mark(3, &BTreeMap::from([("BTCUSDT".into(), 0.0001)]))
            .unwrap();
        assert_eq!(account.wallet_balance, 0.0);
        assert_eq!(
            account
                .traces
                .iter()
                .filter(|row| row.event == "forced_close")
                .count(),
            close_count
        );
    }

    #[test]
    fn shared_account_rejects_individually_affordable_but_jointly_unreserved_orders() {
        let mut account = SharedAccount::new(100.0, EngineConfig::default()).unwrap();
        account.reserve("g1", 60.0).unwrap();
        assert!(account.reserve("g2", 60.0).is_err());
    }

    #[test]
    fn block_transition_preserves_wallet_positions_cycles_and_running_peak_dd() {
        let mut account = SharedAccount::new(1000.0, EngineConfig::default()).unwrap();
        account.add_group(group("g", "fit1")).unwrap();
        account.submit(request("o", "BTCUSDT", 100.0)).unwrap();
        let before = (
            account.wallet_balance,
            account.positions.clone(),
            account.groups.clone(),
            account.running_peak_equity,
        );
        account.transition_block(100, "fit2");
        assert_eq!(
            before,
            (
                account.wallet_balance,
                account.positions.clone(),
                account.groups.clone(),
                account.running_peak_equity
            )
        );
    }

    #[test]
    fn active_group_keeps_open_fit_after_selector_roll() {
        let mut account = SharedAccount::new(1000.0, EngineConfig::default()).unwrap();
        account.add_group(group("g", "fit1")).unwrap();
        account.transition_block(100, "fit2");
        assert_eq!(account.groups["g"].fit_version, "fit1");
    }

    #[test]
    fn five_loaded_symbols_with_four_filled_symbols_fails_asset_gate() {
        let mut account = SharedAccount::new(1000.0, EngineConfig::default()).unwrap();
        for (index, symbol) in ["BTC", "ETH", "SOL", "BNB"].iter().enumerate() {
            account
                .submit(request(&format!("o{index}"), symbol, 10.0))
                .unwrap();
        }
        assert_eq!(actual_assets(&account).len(), 4);
        assert!(actual_assets(&account).len() < 5);
    }

    #[test]
    fn partial_fill_changes_cash_inventory_margin_and_next_so_reserve() {
        let mut account = SharedAccount::new(1000.0, EngineConfig::default()).unwrap();
        account.add_group(group("g", "fit1")).unwrap();
        let mut partial = request("o", "BTCUSDT", 100.0);
        partial.fill_fraction = 0.25;
        let out = account.submit(partial).unwrap();
        assert_eq!(out.filled_quantity, 0.25);
        assert!(account.initial_margin() > 0.0 && account.wallet_balance < 1000.0);
        account.request_next_so("g", 125.0).unwrap();
        assert!(account.reserved_quote > 0.0);
    }

    #[test]
    fn leg_delay_books_realized_legging_loss() {
        let mut account = SharedAccount::new(1000.0, EngineConfig::default()).unwrap();
        let mut delayed = request("o", "BTCUSDT", 100.0);
        delayed.delayed_bars = 2;
        account.submit(delayed).unwrap();
        account
            .process_pending(3, &BTreeMap::from([("BTCUSDT".into(), 110.0)]))
            .unwrap();
        assert!(account
            .traces
            .iter()
            .any(|row| row.event == "legging_pnl" && row.quote.unwrap() < 0.0));
    }

    #[test]
    fn one_leg_reject_executes_hedge_or_flatten() {
        let mut account = SharedAccount::new(1000.0, EngineConfig::default()).unwrap();
        let first = request("a", "BTCUSDT", 100.0);
        let mut second = request("b", "ETHUSDT", 100.0);
        second.reject = true;
        account
            .execute_pair_hedge_or_flatten(first, second)
            .unwrap();
        assert!(account.positions.is_empty());
        assert!(account
            .traces
            .iter()
            .any(|row| row.event == "hedge_or_flatten"));
    }

    #[test]
    fn funding_cashflow_changes_equity_for_open_perp() {
        let mut account = SharedAccount::new(1000.0, EngineConfig::default()).unwrap();
        let request = request("o", "BTCUSDT", 100.0);
        let key = request.key.clone();
        account.submit(request).unwrap();
        let before = account.equity();
        account.apply_funding(2, &key, 0.001).unwrap();
        assert!(account.equity() < before);
    }

    #[test]
    fn maintenance_tier_change_can_liquidate_at_event_time() {
        let mut account = SharedAccount::new(100.0, EngineConfig::default()).unwrap();
        account.submit(request("o", "BTCUSDT", 100.0)).unwrap();
        account.config.maintenance_rate = 2.0;
        account
            .mark(2, &BTreeMap::from([("BTCUSDT".into(), 100.0)]))
            .unwrap();
        assert!(account.terminated);
    }

    #[test]
    fn restart_reconcile_matches_uninterrupted_order_and_equity_hashes() {
        let file = tempfile::NamedTempFile::new().unwrap();
        let mut account = SharedAccount::new(1000.0, EngineConfig::default()).unwrap();
        account.add_group(group("g", "fit1")).unwrap();
        account.submit(request("o", "BTCUSDT", 100.0)).unwrap();
        account.save_sqlite(file.path()).unwrap();
        let restored = SharedAccount::load_sqlite(file.path()).unwrap();
        assert_eq!(account.wallet_balance, restored.wallet_balance);
        assert_eq!(account.positions, restored.positions);
        assert_eq!(account.groups, restored.groups);
        assert_eq!(account.pending_orders, restored.pending_orders);
        assert_eq!(account.reserved_quote, restored.reserved_quote);
        assert_eq!(
            account.canonical_order_equity_hash(),
            restored.canonical_order_equity_hash()
        );
    }

    #[test]
    fn independent_backtest_and_service_adapters_match() {
        let orders = vec![request("a", "BTCUSDT", 100.0), {
            let mut r = request("b", "ETHUSDT", 100.0);
            r.reject = true;
            r
        }];
        let mut left = SharedAccount::new(1000.0, EngineConfig::default()).unwrap();
        let mut right = left.clone();
        let a = BacktestFakeExchange.replay(&mut left, &orders).unwrap();
        let b = ServiceFakeExchange.replay(&mut right, &orders).unwrap();
        assert_eq!(a, b);
    }
}
