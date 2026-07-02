use std::{
    collections::{BTreeMap, BTreeSet},
    env, fs,
    io::Write,
    path::{Path, PathBuf},
};

use backtest_engine::{
    market_data::{KlineBar, MarketDataSource},
    martingale::{
        kline_engine::run_kline_screening_with_funding,
        metrics::{DrawdownPoint, EquityPoint, MartingaleTradeDetail},
    },
    sqlite_market_data::{load_funding_rates_readonly, SqliteMarketDataSource},
};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use shared_domain::martingale::MartingalePortfolioConfig;

#[derive(Debug, Deserialize)]
struct RepriceRequest {
    candidate_id: String,
    portfolio_id: String,
    risk_profile: String,
    symbol: String,
    weight_pct: String,
    start_ms: i64,
    end_ms: i64,
    #[serde(default = "default_interval")]
    interval: String,
    config: Value,
}

#[derive(Debug, Serialize)]
struct RepriceResponse {
    candidate_id: String,
    portfolio_id: String,
    risk_profile: String,
    symbol: String,
    weight_pct: String,
    total_return_pct: f64,
    annualized_return_pct: Option<f64>,
    max_drawdown_pct: f64,
    trade_count: u64,
    total_fee_quote: Option<f64>,
    total_slippage_quote: Option<f64>,
    total_funding_quote: Option<f64>,
    planned_margin_quote: Option<f64>,
    max_leverage_used: Option<f64>,
    equity_curve: Vec<EquityPoint>,
    drawdown_curve: Vec<DrawdownPoint>,
    trades_preview: Vec<MartingaleTradeDetail>,
}

fn default_interval() -> String {
    "1m".to_string()
}

fn main() -> Result<(), String> {
    let args = Args::parse()?;
    let text = fs::read_to_string(&args.input_path)
        .map_err(|err| format!("read {}: {err}", args.input_path.display()))?;
    let requests: Vec<RepriceRequest> =
        serde_json::from_str(&text).map_err(|err| format!("parse input json: {err}"))?;
    let market = SqliteMarketDataSource::open_readonly(&args.market_data_path)?;
    let mut bar_cache: BTreeMap<(String, i64, i64, String), Vec<KlineBar>> = BTreeMap::new();
    let mut output = fs::File::create(&args.output_path)
        .map_err(|err| format!("create {}: {err}", args.output_path.display()))?;
    let all_symbols = requests
        .iter()
        .map(|request| request.symbol.trim().to_uppercase())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let min_start = requests
        .iter()
        .map(|request| request.start_ms)
        .min()
        .ok_or_else(|| "input is empty".to_string())?;
    let max_end = requests
        .iter()
        .map(|request| request.end_ms)
        .max()
        .ok_or_else(|| "input is empty".to_string())?;
    let funding_rates =
        load_funding_rates_readonly(&args.funding_data_path, &all_symbols, min_start, max_end)?;
    let request_count = requests.len();

    for (index, request) in requests.into_iter().enumerate() {
        eprintln!(
            "reprice {}/{} {} {}",
            index + 1,
            request_count,
            request.portfolio_id,
            request.candidate_id
        );
        let portfolio = parse_portfolio_config(&request.config)?;
        let symbols = portfolio
            .strategies
            .iter()
            .map(|strategy| strategy.symbol.trim().to_uppercase())
            .collect::<std::collections::BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        let mut bars = Vec::new();
        for symbol in &symbols {
            let key = (
                symbol.clone(),
                request.start_ms,
                request.end_ms,
                request.interval.clone(),
            );
            let cached = if let Some(cached) = bar_cache.get(&key) {
                cached.clone()
            } else {
                let loaded = market.load_klines(
                    symbol,
                    request.start_ms,
                    request.end_ms,
                    &request.interval,
                )?;
                bar_cache.insert(key, loaded.clone());
                loaded
            };
            bars.extend(cached);
        }
        bars.sort_by(|left, right| {
            left.open_time_ms
                .cmp(&right.open_time_ms)
                .then_with(|| left.symbol.cmp(&right.symbol))
        });
        let request_funding_rates = funding_rates
            .iter()
            .filter(|point| {
                point.funding_time_ms >= request.start_ms
                    && point.funding_time_ms <= request.end_ms
                    && symbols.contains(&point.symbol)
            })
            .cloned()
            .collect::<Vec<_>>();
        let result = run_kline_screening_with_funding(portfolio, &bars, &request_funding_rates, 0.0)?;
        let response = RepriceResponse {
            candidate_id: request.candidate_id,
            portfolio_id: request.portfolio_id,
            risk_profile: request.risk_profile,
            symbol: request.symbol,
            weight_pct: request.weight_pct,
            total_return_pct: result.metrics.total_return_pct,
            annualized_return_pct: result.metrics.annualized_return_pct,
            max_drawdown_pct: result.metrics.max_drawdown_pct,
            trade_count: result.metrics.trade_count,
            total_fee_quote: result.metrics.total_fee_quote,
            total_slippage_quote: result.metrics.total_slippage_quote,
            total_funding_quote: result.metrics.total_funding_quote,
            planned_margin_quote: result.metrics.planned_margin_quote,
            max_leverage_used: result.metrics.max_leverage_used,
            equity_curve: sampled_preview(&result.equity_curve, 5000),
            drawdown_curve: sampled_preview(&result.drawdown_curve, 5000),
            trades_preview: sampled_preview(&result.trades, 100),
        };
        writeln!(
            output,
            "{}",
            serde_json::to_string(&response)
                .map_err(|err| format!("serialize jsonl response: {err}"))?
        )
        .map_err(|err| format!("write {}: {err}", args.output_path.display()))?;
        output
            .flush()
            .map_err(|err| format!("flush {}: {err}", args.output_path.display()))?;
    }

    println!("{{\"written\":{request_count}}}");
    Ok(())
}

fn parse_portfolio_config(value: &Value) -> Result<MartingalePortfolioConfig, String> {
    let portfolio_value = value
        .get("portfolio_config")
        .cloned()
        .unwrap_or_else(|| value.clone());
    serde_json::from_value(portfolio_value).map_err(|err| format!("parse portfolio config: {err}"))
}

fn sampled_preview<T: Clone>(items: &[T], max_points: usize) -> Vec<T> {
    if items.len() <= max_points || max_points == 0 {
        return items.to_vec();
    }
    if max_points == 1 {
        return vec![items[0].clone()];
    }
    (0..max_points)
        .map(|index| {
            let source_index = index * (items.len() - 1) / (max_points - 1);
            items[source_index].clone()
        })
        .collect()
}

struct Args {
    input_path: PathBuf,
    output_path: PathBuf,
    market_data_path: PathBuf,
    funding_data_path: PathBuf,
}

impl Args {
    fn parse() -> Result<Self, String> {
        let mut input_path = None;
        let mut output_path = None;
        let mut market_data_path = None;
        let mut funding_data_path = None;
        let mut args = env::args().skip(1);
        while let Some(arg) = args.next() {
            let value = args
                .next()
                .ok_or_else(|| format!("missing value for argument {arg}"))?;
            match arg.as_str() {
                "--input" => input_path = Some(PathBuf::from(value)),
                "--output" => output_path = Some(PathBuf::from(value)),
                "--market-data" => market_data_path = Some(PathBuf::from(value)),
                "--funding-data" => funding_data_path = Some(PathBuf::from(value)),
                _ => return Err(format!("unknown argument {arg}")),
            }
        }
        Ok(Self {
            input_path: required_path(input_path, "--input")?,
            output_path: output_path.ok_or_else(|| "--output is required".to_string())?,
            market_data_path: required_path(market_data_path, "--market-data")?,
            funding_data_path: required_path(funding_data_path, "--funding-data")?,
        })
    }
}

fn required_path(value: Option<PathBuf>, name: &str) -> Result<PathBuf, String> {
    let path = value.ok_or_else(|| format!("{name} is required"))?;
    if !Path::new(&path).exists() {
        return Err(format!("{} does not exist: {}", name, path.display()));
    }
    Ok(path)
}
