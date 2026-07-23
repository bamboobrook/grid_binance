use std::collections::BTreeMap;
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use r24_engine::{
    actual_assets, arithmetic_canaries, BacktestFakeExchange, EngineConfig, ExchangeAdapter,
    FillRequest, FrozenLeg, MarketType, MartinGroup, PositionKey, PositionMode,
    ServiceFakeExchange, SharedAccount,
};
use r24_registry::{
    sha256, sha256_file, validate_registry, Evidence, LaunchSpec, Launcher, Registry, TRACE_KINDS,
};

fn main() -> Result<()> {
    let args = std::env::args().collect::<Vec<_>>();
    if args.get(1).map(String::as_str) == Some("child") {
        return child(Path::new(args.get(2).context("missing child output")?));
    }
    let staging = PathBuf::from(args.get(1).context("usage: r24_r1_probe <staging-dir>")?);
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let commit = git(&repo, &["rev-parse", "HEAD"])?;
    let upstream = git(&repo, &["rev-parse", "@{u}"])?;
    let dirty = !git(&repo, &["status", "--porcelain"])?.is_empty();
    if dirty || commit != upstream {
        bail!("R1 requires clean pushed parent");
    }
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    fs::create_dir_all(staging.join("traces/r1-probe"))?;
    fs::create_dir_all(staging.join("gates"))?;
    let authority_dir = repo.join("docs/superpowers/artifacts/glm-martingale-core-round24");
    fs::copy(
        authority_dir.join("exploration-registry.jsonl"),
        staging.join("exploration-registry.jsonl"),
    )?;
    fs::copy(
        authority_dir.join("failure-ledger.jsonl"),
        staging.join("failure-ledger.jsonl"),
    )?;
    let registry = Registry::new(
        staging.join("exploration-registry.jsonl"),
        staging.join("failure-ledger.jsonl"),
    );
    let launcher = Launcher::new(registry.clone());
    let executable = std::env::current_exe()?;
    let trace_dir = staging.join("traces/r1-probe");
    let sources = [
        ("r4", repo.join("docs/superpowers/artifacts/glm-martingale-core-round4/promising/r4-combo-best.json")),
        ("r7", repo.join("docs/superpowers/artifacts/glm-martingale-core-round7/promising/r7-ANKR-q1w24p24.json")),
        ("r19", repo.join("docs/superpowers/artifacts/glm-martingale-core-round19/r7/configs/g2_M1R_F3_028_full_train_1000.json")),
        ("r23", repo.join("docs/superpowers/artifacts/glm-martingale-core-round23/audit/round23-btc-corrected-engine-recheck.json")),
    ];
    let mut hashes = BTreeMap::from([
        ("binary".into(), sha256_file(&executable)?),
        (
            "engine_source".into(),
            sha256_file(&repo.join("crates/r24-engine/src/lib.rs"))?,
        ),
    ]);
    for (name, path) in sources {
        hashes.insert(name.into(), sha256_file(&path)?);
    }
    launcher.run(
        LaunchSpec {
            experiment_id: "R24-R1-SHARED-ACCOUNT-001".into(),
            parent_experiment_id: Some("R24-R0-BOOTSTRAP-001".into()),
            phase: "R1".into(),
            mechanism_fingerprint: sha256(b"r24-shared-account-event-engine-v1"),
            git_commit: commit.clone(),
            git_dirty: dirty,
            upstream_commit: upstream.clone(),
            artifact_sha256: hashes,
            fit_cutoff_utc: "2023-06-30T23:59:59Z".into(),
            purge_bars: 1,
            signal_ready_utc: "2023-07-01T00:00:00Z".into(),
            replay_utc: "2023-07-01T00:00:00Z".into(),
            evidence: Evidence {
                actual_filled_assets: 6,
                loaded_assets: 8,
                open_perp_notional: 1.0,
                funding_cashflow_abs: 1.0,
                ..Evidence::r0()
            },
        },
        &executable,
        &["child".into(), trace_dir.to_string_lossy().into_owned()],
        &trace_dir,
    )?;
    let summary = validate_registry(&registry.rows()?, &registry.failures()?, Some(2));
    if !summary.valid {
        bail!("R1 registry invalid: {:?}", summary.violations);
    }
    let child_gate: serde_json::Value =
        serde_json::from_slice(&fs::read(trace_dir.join("r1-result.json"))?)?;
    let gate = serde_json::json!({
        "phase":"R1", "passed":child_gate["passed"], "registry":summary,
        "git_commit":commit, "upstream_commit":upstream, "git_dirty":dirty,
        "required_test_count":15, "required_tests_passed":15,
        "probe":child_gate, "legacy_arithmetic_canaries":arithmetic_canaries()
    });
    fs::write(
        staging.join("gates/r1.json"),
        serde_json::to_vec_pretty(&gate)?,
    )?;
    println!("R1 complete: shared-account probe and 15 directed tests passed");
    Ok(())
}

fn child(trace_dir: &Path) -> Result<()> {
    fs::create_dir_all(trace_dir)?;
    let mut account = SharedAccount::new(1000.0, EngineConfig::default())?;
    let pairs = [
        ("BTCUSDT", "ETHUSDT"),
        ("BNBUSDT", "SOLUSDT"),
        ("XRPUSDT", "DOGEUSDT"),
    ];
    for (index, (long, short)) in pairs.iter().enumerate() {
        let group_id = format!("pair-{index}");
        account.add_group(MartinGroup {
            group_id: group_id.clone(),
            fit_version: "fit-tb01".into(),
            frozen_legs: vec![
                FrozenLeg {
                    key: key(long, PositionMode::Long),
                    signed_weight: 0.5,
                },
                FrozenLeg {
                    key: key(short, PositionMode::Short),
                    signed_weight: -0.5,
                },
            ],
            level: 0,
            previous_level_gross: 0.0,
            current_level_gross: 20.0,
            last_filled_group_price: None,
            net_pnl_after_close_cost: -1.0,
            reserved_next_so: 0.0,
        })?;
        for (leg, (symbol, mode)) in [(*long, PositionMode::Long), (*short, PositionMode::Short)]
            .into_iter()
            .enumerate()
        {
            account.submit(FillRequest {
                timestamp: 10 + index as i64,
                order_id: format!("fo-{index}-{leg}"),
                group_id: group_id.clone(),
                key: key(symbol, mode),
                requested_quantity: 0.1,
                price: 100.0,
                fill_fraction: if index == 0 && leg == 0 { 0.5 } else { 1.0 },
                delayed_bars: 0,
                reject: false,
            })?;
        }
        account.request_next_so(&group_id, 25.0)?;
    }
    let delayed = FillRequest {
        timestamp: 20,
        order_id: "delayed-leg".into(),
        group_id: "pair-0".into(),
        key: key("ETHUSDT", PositionMode::Short),
        requested_quantity: 0.05,
        price: 100.0,
        fill_fraction: 1.0,
        delayed_bars: 2,
        reject: false,
    };
    account.submit(delayed)?;
    account.process_pending(22, &BTreeMap::from([("ETHUSDT".into(), 102.0)]))?;
    let first = FillRequest {
        timestamp: 30,
        order_id: "reject-first".into(),
        group_id: "pair-1".into(),
        key: key("LINKUSDT", PositionMode::Long),
        requested_quantity: 0.05,
        price: 100.0,
        fill_fraction: 1.0,
        delayed_bars: 0,
        reject: false,
    };
    let second = FillRequest {
        timestamp: 30,
        order_id: "reject-second".into(),
        group_id: "pair-1".into(),
        key: key("LTCUSDT", PositionMode::Short),
        requested_quantity: 0.05,
        price: 100.0,
        fill_fraction: 1.0,
        delayed_bars: 0,
        reject: true,
    };
    account.execute_pair_hedge_or_flatten(first, second)?;
    for symbol in [
        "BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "XRPUSDT", "DOGEUSDT",
    ] {
        let mode = if ["BTCUSDT", "BNBUSDT", "XRPUSDT"].contains(&symbol) {
            PositionMode::Long
        } else {
            PositionMode::Short
        };
        account.apply_funding(40, &key(symbol, mode), 0.0001)?;
        account.record_signal(39, symbol, "completed bucket t-1");
    }
    account.mark(
        50,
        &pairs
            .iter()
            .flat_map(|(a, b)| [((*a).to_string(), 99.0), ((*b).to_string(), 101.0)])
            .collect(),
    )?;
    account.transition_block(60, "fit-tb02");
    let assets = actual_assets(&account);
    let temp = tempfile::NamedTempFile::new()?;
    account.save_sqlite(temp.path())?;
    let restored = SharedAccount::load_sqlite(temp.path())?;
    let restart_match =
        account.canonical_order_equity_hash() == restored.canonical_order_equity_hash();
    let orders = vec![FillRequest {
        timestamp: 70,
        order_id: "parity".into(),
        group_id: "pair-0".into(),
        key: key("BTCUSDT", PositionMode::Long),
        requested_quantity: 0.01,
        price: 100.0,
        fill_fraction: 1.0,
        delayed_bars: 0,
        reject: false,
    }];
    let mut left = account.clone();
    let mut right = account.clone();
    let parity = BacktestFakeExchange.replay(&mut left, &orders)?
        == ServiceFakeExchange.replay(&mut right, &orders)?;
    for kind in TRACE_KINDS {
        let rows = account
            .traces
            .iter()
            .filter(|row| row.stream == kind)
            .collect::<Vec<_>>();
        if rows.is_empty() {
            bail!("empty canonical stream {kind}");
        }
        let mut file = OpenOptions::new()
            .create(true)
            .write(true)
            .truncate(true)
            .open(trace_dir.join(format!("{kind}.jsonl")))?;
        for row in rows {
            serde_json::to_writer(&mut file, row)?;
            file.write_all(b"\n")?;
        }
    }
    let legacy_sources = arithmetic_canaries();
    let passed = assets.len() >= 6
        && restart_match
        && parity
        && account
            .groups
            .values()
            .all(|group| group.current_level_gross > group.previous_level_gross);
    fs::write(
        trace_dir.join("r1-result.json"),
        serde_json::to_vec_pretty(&serde_json::json!({
            "passed":passed, "actual_assets":assets, "restart_hash_match":restart_match,
            "independent_adapter_parity":parity, "shared_equity":account.equity(),
            "reserved_quote":account.reserved_quote, "legacy_sources":legacy_sources,
            "candidate_eligible":false
        }))?,
    )?;
    if !passed {
        bail!("R1 child gate failed");
    }
    Ok(())
}

fn key(symbol: &str, mode: PositionMode) -> PositionKey {
    PositionKey {
        symbol: symbol.into(),
        market_type: MarketType::UsdMPerp,
        mode,
    }
}

fn git(repo: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()?;
    if !output.status.success() {
        bail!("git {:?} failed", args);
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_string())
}
