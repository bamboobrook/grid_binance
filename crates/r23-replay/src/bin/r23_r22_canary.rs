//! R22 non-candidate arithmetic canary (plan §5.3 last line).
//! Re-runs the R22 budget500/m3/fo120 config through the production-conservative
//! R23 driver to PROVE the old 31.2%/9.9% frontier is revoked and does NOT enter
//! ranking. This is recorded with experiment_id prefix "r22-canary-NONCANDIDATE".
use std::collections::BTreeMap;
use backtest_engine::market_data::KlineBar;
use r23_replay::continuous::{run_continuous_replay, BlockReplayInput, ContinuousReplayConfig, ReplayMode};
use r23_replay::fit::fit_pair_ols;
use backtest_engine::sqlite_market_data::SqliteMarketDataSource;
use r23_registry::launcher::{LaunchRequest, Launcher};
use r23_registry::{FailureLedger, Registry};
use shared_domain::martingale::SynchronizedCycleConfig;

fn main() {
    let art = std::env::args().nth(1).expect("artifact dir");
    let src = SqliteMarketDataSource::open_readonly("data/market_data_full.db").unwrap();
    let fs = 1672531200000i64; let tb = 1688169600000i64; let te = 1748649600000i64;
    let btc = src.load_klines_with_market_type("BTCUSDT","futures_usdt_perp",fs,te,"1m").unwrap();
    let eth = src.load_klines_with_market_type("ETHUSDT","futures_usdt_perp",fs,te,"1m").unwrap();
    let b5=resample(&btc,5); let e5=resample(&eth,5);
    let bm:BTreeMap<i64,f64>=b5.iter().map(|b|(b.open_time_ms,b.close)).collect();
    let em:BTreeMap<i64,f64>=e5.iter().map(|b|(b.open_time_ms,b.close)).collect();
    let (mut ta,mut tbv,mut bars)=(Vec::new(),Vec::new(),Vec::new());
    for (ts,a) in &bm {
        if let Some(b)=em.get(ts){
            if *ts<=tb-300000 { ta.push(*a); tbv.push(*b); }
            else if *ts>=tb && *ts<=te {
                bars.push(KlineBar{symbol:"BTCUSDT".into(),open_time_ms:*ts,open:*a,high:*a,low:*a,close:*a,volume:1.0});
                bars.push(KlineBar{symbol:"ETHUSDT".into(),open_time_ms:*ts,open:*b,high:*b,low:*b,close:*b,volume:1.0});
            }
        }
    }
    let pf=fit_pair_ols("M1_BTC_ETH","BTCUSDT","ETHUSDT",&ta,&tbv,5).unwrap();
    // R22 canary config: budget500, multiplier=3 (m3), group_fo=120 (fo120), entry_z=1 (ez1), max_legs=4 (ml4)
    let cfg=SynchronizedCycleConfig{family:"M1_pair".into(),bar_boundary_minutes:5,entry_z:1.0,so_residual_step_z:0.5,group_fo_quote:120.0,multiplier:3.0,max_legs:4,exit_z:0.5,tp_net_bps_floor:10,leverage:3,group_gross_cap_pct:50.0,..SynchronizedCycleConfig::default()};
    let rcfg=ContinuousReplayConfig{mode:ReplayMode::ContinuousMerged,martingale_cfg:cfg.clone(),budget_quote:500.0,funding_rates:vec![],fee_bps_override:Some(2.0),slippage_bps_override:Some(1.0)};
    let out=run_continuous_replay(&rcfg,&[BlockReplayInput{block_index:1,train_fits:vec![pf.fit.clone()],test_bars:bars}]);
    let res=out.merged_result.as_ref();
    if let Some(r)=res {
        let se=r.equity_curve.first().map(|e|e.equity_quote).unwrap_or(500.0);
        let ee=r.equity_curve.last().map(|e|e.equity_quote).unwrap_or(500.0);
        let dd=r.drawdown_curve.iter().map(|d|d.drawdown_pct).fold(0.0f64,f64::max);
        println!("R22-CANARY-NONCANDIDATE budget500/m3/fo120/ez1/ml4:");
        println!("  start_eq={:.2} end_eq={:.2} total_return_pct={:.4} max_dd_pct={:.4} trade_count={}",se,ee,(ee/se-1.0)*100.0,dd,r.metrics.trade_count);
        println!("  NOTE: this is the R22 config run through the production-conservative R23 driver.");
        println!("  The R22-reported ann=31.2%/dd=9.9% came from the INVALID Python PnL engine and is REVOKED.");
        println!("  This row MUST NOT enter any G1/G2/R8 ranking.");
    } else { println!("engine error: {:?}", out.engine_error); }
    // record as NONCANDIDATE
    let reg=std::path::Path::new(&art).join("exploration-registry.jsonl");
    let led=std::path::Path::new(&art).join("failure-ledger.jsonl");
    let launcher=Launcher::new(Registry::new(&reg),FailureLedger::new(&led));
    let h=|s:&str|r23_registry::hash::sha256_str(s);
    let req=LaunchRequest{experiment_id:format!("r22-canary-NONCANDIDATE-budget500-m3-fo120-{}",chrono::Utc::now().format("%Y%m%d%H%M%S")),phase:"R1".into(),parent_experiment_id:None,git_commit:h("canary"),git_dirty:false,upstream_remote_commit:h("canary"),config_budget_u:500.0,argv_budget_u:500.0,binary_sha256:h("b"),source_sha256:h("s"),market_sha256:h("m"),metrics_sha256:h("m"),depth_sha256:h("d"),aggtrades_sha256:h("a"),funding_sha256:h("f"),filter_sha256:h("f"),maintenance_sha256:h("m"),borrow_sha256:h("b"),cost_sha256:h("c"),config_sha256:h(&serde_json::to_string(&cfg).unwrap_or_default()),fit_sha256:pf.fit.fit_sha256.clone(),protocol_sha256:h("p")};
    let st=r23_replay::continuous::record_outcome(&launcher,&req,&out).unwrap();
    println!("recorded {} -> {:?}",req.experiment_id,st);
}
fn resample(bars:&[KlineBar],bm:u32)->Vec<KlineBar>{let mut o:BTreeMap<i64,KlineBar>=BTreeMap::new();let bk=(bm as i64)*60000;for b in bars{let bu=(b.open_time_ms/bk)*bk;let e=o.entry(bu).or_insert_with(||KlineBar{symbol:b.symbol.clone(),open_time_ms:bu,open:b.open,high:b.high,low:b.low,close:b.close,volume:0.0});e.high=e.high.max(b.high);e.low=e.low.min(b.low);e.close=b.close;e.volume+=b.volume;}o.into_values().collect()}
