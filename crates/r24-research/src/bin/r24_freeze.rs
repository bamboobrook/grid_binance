use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, File};
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use chrono::{Days, NaiveDate};
use r24_research::{preregister_policies, BLOCKS, COLD_STARTS, UNIVERSE};
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;
use sha2::{Digest, Sha256};

fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    let output = PathBuf::from(
        args.get(1)
            .context("usage: r24_freeze <output> <spot-info> <perp-info>")?,
    );
    let spot_info = PathBuf::from(args.get(2).context("missing spot exchangeInfo")?);
    let perp_info = PathBuf::from(args.get(3).context("missing perp exchangeInfo")?);
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    if output.exists() {
        fs::remove_dir_all(&output)?;
    }
    fs::create_dir_all(output.join("data"))?;
    let protocol = protocol_json();
    write_json(output.join("round24-protocol.json"), &protocol)?;
    let market = market_coverage(&repo.join("data/market_data_full.db"))?;
    let funding = funding_coverage(&repo.join("data/funding_rates_round12.db"))?;
    let archives = scan_archives(
        &repo,
        &output.join("data/round24-external-archive-manifest.jsonl"),
    )?;
    let spot = filtered_exchange_info(&spot_info)?;
    let perp = filtered_exchange_info(&perp_info)?;
    write_json(output.join("data/spot-exchangeInfo-8-symbols.json"), &spot)?;
    write_json(output.join("data/perp-exchangeInfo-8-symbols.json"), &perp)?;
    let f1_ready = market.iter().all(|row| row.complete)
        && funding
            .iter()
            .all(|row| row["complete"].as_bool() == Some(true));
    let data_manifest = serde_json::json!({
        "manifest_version":1,
        "generated_from_local_data":true,
        "universe":UNIVERSE,
        "protocol_window":{"start":"2023-01-01","end":"2026-05-31"},
        "market_1m":market,
        "funding_8h":funding,
        "execution_constraints":{
            "mode":"current_snapshot_plus_conservative_static_model_and_event_time_stress",
            "historical_snapshot_claim":false,
            "spot_snapshot_sha256":sha_file(&spot_info)?,
            "perp_snapshot_sha256":sha_file(&perp_info)?,
            "maintenance_model":"per-symbol current maintMarginPercent, stressed upward at event time; authenticated historical leverage brackets unavailable",
            "filter_model":"current PRICE_FILTER/LOT_SIZE/MARKET_LOT_SIZE/NOTIONAL applied conservatively across history and stressed at G2",
            "signal_or_selection_input":false
        },
        "external_archives":archives,
        "family_data_gate":{
            "F1":{"status":if f1_ready {"ready"} else {"blocked_incomplete_data"},"uses":"1m spot/perp klines + funding + frozen conservative execution model"},
            "F3_M1":{"status":"blocked_incomplete_data","reason":"metrics start 2023-07-01, six fit months absent"},
            "F3_M2":{"status":"blocked_incomplete_data","reason":"bookDepth lacks full protocol/six assets; aggTrades only 2023-07-01..15"}
        },
        "no_short_window_annualization":true
    });
    write_json(output.join("round24-data-manifest.json"), &data_manifest)?;
    let policies = preregister_policies();
    write_json(
        output.join("round24-policy-manifest.json"),
        &serde_json::json!({
            "frozen_before_returns":true,"policy_count":policies.len(),"policies":policies
        }),
    )?;
    let mut fingerprints = BufWriter::new(File::create(
        output.join("round24-mechanism-fingerprints.jsonl"),
    )?);
    for policy in &policies {
        serde_json::to_writer(
            &mut fingerprints,
            &serde_json::json!({
                "policy_id":policy.policy_id,"family":policy.family,"mechanism_fingerprint":policy.mechanism_fingerprint,"status":policy.status
            }),
        )?;
        fingerprints.write_all(b"\n")?;
    }
    fingerprints.flush()?;
    write_json(
        output.join("round1-24-trial-ledger.json"),
        &serde_json::json!({
            "round1_22_trials":"unknown",
            "round23_visible_trial_floor":1080,
            "historical_global_trial_count":"unknown_not_zero",
            "round24_preregistered_policy_templates":policies.len(),
            "round24_executed_trials":0,
            "dsr_sensitivity_trial_counts":[10000,50000,100000,1000000],
            "rule":"DSR trial count may never be below the historical global ledger floor",
            "historical_prequential_only":true
        }),
    )?;
    write_json(
        output.join("gates-r2-r3.json"),
        &serde_json::json!({
            "R2":{"passed_for_F1":f1_ready,"F3_M1":"blocked_incomplete_data","F3_M2":"blocked_incomplete_data"},
            "R3":{"passed":true,"policy_count":policies.len(),"f1_count":16,"m1_count":8,"m2_count":8,"e1_conditional":8,"f2_conditional":12},
            "return_replays_run":0
        }),
    )?;
    println!(
        "R2/R3 freeze complete: F1_ready={f1_ready}, policies={}",
        policies.len()
    );
    Ok(())
}

#[derive(Debug, Serialize)]
struct Coverage {
    symbol: String,
    market_type: String,
    rows: u64,
    unique_timestamps: u64,
    expected_rows: u64,
    duplicate_rows: u64,
    missing_minutes: u64,
    min_timestamp: i64,
    max_timestamp: i64,
    sample_sha256: String,
    complete: bool,
}

fn market_coverage(path: &Path) -> Result<Vec<Coverage>> {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let start = utc_ms("2023-01-01");
    let end_exclusive = utc_ms("2026-06-01");
    let expected = ((end_exclusive - start) / 60_000) as u64;
    let mut rows = Vec::new();
    for symbol in UNIVERSE {
        for market_type in ["spot", "futures_usdt_perp"] {
            let query = "SELECT COUNT(*),COUNT(DISTINCT open_time),MIN(open_time),MAX(open_time) FROM klines WHERE symbol=?1 AND market_type=?2 AND timeframe='1m' AND open_time>=?3 AND open_time<?4";
            let (count, unique, min, max): (u64, u64, Option<i64>, Option<i64>) = connection
                .query_row(query, (symbol, market_type, start, end_exclusive), |row| {
                    Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
                })?;
            let sample: String = connection.query_row("SELECT printf('%s|%s|%s|%.8f|%.8f|%.8f|%.8f',symbol,market_type,open_time,open,high,low,close) FROM klines WHERE symbol=?1 AND market_type=?2 AND timeframe='1m' AND open_time>=?3 AND open_time<?4 ORDER BY open_time LIMIT 1", (symbol,market_type,start,end_exclusive), |row| row.get(0))?;
            rows.push(Coverage {
                symbol: symbol.into(),
                market_type: market_type.into(),
                rows: count,
                unique_timestamps: unique,
                expected_rows: expected,
                duplicate_rows: count.saturating_sub(unique),
                missing_minutes: expected.saturating_sub(unique),
                min_timestamp: min.unwrap_or(0),
                max_timestamp: max.unwrap_or(0),
                sample_sha256: r24_registry::sha256(sample.as_bytes()),
                complete: unique == expected
                    && count == unique
                    && min == Some(start)
                    && max == Some(end_exclusive - 60_000),
            });
        }
    }
    Ok(rows)
}

fn funding_coverage(path: &Path) -> Result<Vec<serde_json::Value>> {
    let connection = Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let start = utc_ms("2023-01-01");
    let end_exclusive = utc_ms("2026-06-01");
    let expected = ((end_exclusive - start) / 28_800_000) as u64;
    let mut rows = Vec::new();
    for symbol in UNIVERSE {
        let (count, unique, min, max): (u64,u64,Option<i64>,Option<i64>) = connection.query_row("SELECT COUNT(*),COUNT(DISTINCT funding_time),MIN(funding_time),MAX(funding_time) FROM funding_rates WHERE symbol=?1 AND funding_time>=?2 AND funding_time<?3", (symbol,start,end_exclusive), |row| Ok((row.get(0)?,row.get(1)?,row.get(2)?,row.get(3)?)))?;
        rows.push(serde_json::json!({"symbol":symbol,"rows":count,"unique_timestamps":unique,"expected_rows":expected,"min_timestamp":min,"max_timestamp":max,"complete":unique==expected&&count==unique&&min==Some(start)&&max==Some(end_exclusive-28_800_000)}));
    }
    Ok(rows)
}

fn scan_archives(repo: &Path, output: &Path) -> Result<serde_json::Value> {
    let root = repo.join("docs/superpowers/artifacts/glm-martingale-core-round23/r3/data");
    let mut writer = BufWriter::new(File::create(output)?);
    let mut summary = BTreeMap::new();
    for kind in ["metrics", "bookDepth", "aggTrades"] {
        let mut files = Vec::new();
        walk_zip(&root.join(kind), &mut files)?;
        files.sort();
        let mut keys = BTreeSet::new();
        let mut symbols = BTreeSet::new();
        let mut dates = Vec::new();
        for path in files {
            let name = path.file_name().unwrap().to_string_lossy();
            let symbol = path
                .parent()
                .and_then(Path::file_name)
                .unwrap()
                .to_string_lossy()
                .to_string();
            let date = name
                .strip_suffix(".zip")
                .and_then(|value| value.get(value.len().saturating_sub(10)..))
                .unwrap_or("unknown")
                .to_string();
            let key = format!("{kind}|{symbol}|{date}");
            let unique = keys.insert(key.clone());
            symbols.insert(symbol.clone());
            dates.push(date.clone());
            let row = serde_json::json!({
                "key":key,"kind":kind,"symbol":symbol,"date":date,
                "path":path.strip_prefix(repo).unwrap_or(&path),"size_bytes":fs::metadata(&path)?.len(),
                "sha256":sha_file(&path)?,"unique_key":unique,
                "sidecar_url":format!("https://data.binance.vision/data/futures/um/daily/{kind}/{symbol}/{name}.CHECKSUM"),
                "sidecar_local":false,"status":"blocked_sidecar_missing"
            });
            serde_json::to_writer(&mut writer, &row)?;
            writer.write_all(b"\n")?;
        }
        dates.sort();
        summary.insert(kind.to_string(), serde_json::json!({
            "archive_count":keys.len(),"symbols":symbols,"min_date":dates.first(),"max_date":dates.last(),
            "all_sidecars_present":false,"status":"blocked_incomplete_data"
        }));
    }
    writer.flush()?;
    Ok(serde_json::to_value(summary)?)
}

fn walk_zip(root: &Path, output: &mut Vec<PathBuf>) -> Result<()> {
    if !root.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(root)? {
        let path = entry?.path();
        if path.is_dir() {
            walk_zip(&path, output)?;
        } else if path.extension().is_some_and(|value| value == "zip") {
            output.push(path);
        }
    }
    Ok(())
}

fn filtered_exchange_info(path: &Path) -> Result<serde_json::Value> {
    let value: serde_json::Value = serde_json::from_slice(&fs::read(path)?)?;
    let symbols = value["symbols"]
        .as_array()
        .context("symbols absent")?
        .iter()
        .filter(|row| {
            row["symbol"]
                .as_str()
                .is_some_and(|symbol| UNIVERSE.contains(&symbol))
        })
        .cloned()
        .collect::<Vec<_>>();
    Ok(
        serde_json::json!({"serverTime":value["serverTime"],"symbols":symbols,"source_sha256":sha_file(path)?}),
    )
}

fn protocol_json() -> serde_json::Value {
    let blocks = BLOCKS.iter().enumerate().map(|(index,(start,end))| {
        let start_date = NaiveDate::parse_from_str(start,"%Y-%m-%d").unwrap();
        let cutoff = start_date.checked_sub_days(Days::new(121)).unwrap();
        serde_json::json!({"block":format!("tb{:02}",index+1),"test_start":start,"test_end":end,"fit_cutoff":cutoff.to_string(),"purge_days":121,"empty_block_kept":true})
    }).collect::<Vec<_>>();
    serde_json::json!({
        "fit_start":"2023-01-01","test_start":"2023-07-01","test_end":"2026-05-31",
        "blocks":blocks,"cold_starts":COLD_STARTS,"continuous_shared_account":true,
        "block_boundary_resets":false,"active_group_open_fit_frozen":true,
        "annualize_only_stitched_days_gte":365,"historical_prequential_only":true
    })
}

fn write_json(path: PathBuf, value: &impl Serialize) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}

fn sha_file(path: &Path) -> Result<String> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    std::io::copy(&mut file, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}

fn utc_ms(date: &str) -> i64 {
    NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .unwrap()
        .and_hms_opt(0, 0, 0)
        .unwrap()
        .and_utc()
        .timestamp_millis()
}
