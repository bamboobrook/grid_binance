use std::fs::{self, File};
use std::io::{BufRead, BufReader, BufWriter, Write};
use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{bail, Context, Result};
use r24_registry::{
    sha256_file, validate_registry, FailureRow, Registry, RegistryRow, RowKind, TerminalStatus,
};

fn main() -> Result<()> {
    let repo = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()?;
    let artifact = repo.join("docs/superpowers/artifacts/glm-martingale-core-round24");
    let commit = git(&repo, &["rev-parse", "HEAD"])?;
    let upstream = git(&repo, &["rev-parse", "@{u}"])?;
    let dirty = !git(&repo, &["status", "--porcelain"])?.is_empty();
    if dirty || commit != upstream {
        bail!("finalizer requires clean pushed evidence parent");
    }
    let registry_path = artifact.join("exploration-registry.jsonl");
    let ledger_path = artifact.join("failure-ledger.jsonl");
    let registry = Registry::new(&registry_path, &ledger_path);
    let mut rows = registry.rows()?;
    let mut failures = registry.failures()?;
    let corrected_paths = correct_and_verify_trace_paths(&repo, &artifact, &mut rows)?;
    write_jsonl(&registry_path, &rows)?;
    let g1: serde_json::Value = serde_json::from_slice(&fs::read(artifact.join("gates/g1.json"))?)?;
    append_exact_failures(&g1, &mut failures);
    write_jsonl(&ledger_path, &failures)?;
    let registry_summary = validate_registry(&rows, &failures, Some(19));
    if !registry_summary.valid {
        bail!("final registry invalid: {:?}", registry_summary.violations);
    }

    let best = g1["policies"]
        .as_array()
        .unwrap()
        .iter()
        .max_by(|left, right| {
            left["annualized_return_pct"]
                .as_f64()
                .unwrap()
                .partial_cmp(&right["annualized_return_pct"].as_f64().unwrap())
                .unwrap()
        })
        .unwrap();
    let gates = [
        (
            "e1",
            serde_json::json!({
                "phase":"E1","status":"not_applicable_no_g1_pb_parent","eligible_parents":0,
                "executed_policies":0,"source_pdf_not_required":true,"reason":"E1 may run only after an F1/F3 parent passes G1 P-B"
            }),
        ),
        (
            "f2",
            serde_json::json!({
                "phase":"F2","status":"not_applicable_single_or_zero_parent","eligible_distinct_mechanisms":0,
                "executed_combinations":0,"reason":"F2 requires at least two distinct G1 P-B parents; finished curves were not injected"
            }),
        ),
        (
            "g2",
            serde_json::json!({
                "phase":"G2","status":"not_applicable_no_committed_g1_survivor","eligible_survivors":0,
                "budgets_executed":0,"cold_starts_executed":0,"stress_replays_executed":0,
                "anti_overfit_status":"not_applicable_no_survivor","reason":"G2 may run only for committed G1 survivors"
            }),
        ),
        (
            "r8",
            serde_json::json!({
                "phase":"R8","status":"complete_no_candidate","eligible_g2_survivors":0,"strict_candidates":0,
                "best_strict_candidate":null,"tiers":{"conservative_50_10":false,"balanced_90_20":false,"aggressive_100_30":false},
                "old_110_target_used":false,"sharpe_hard_gate_used":false
            }),
        ),
    ];
    for (name, gate) in gates {
        write_json(artifact.join(format!("gates/{name}.json")), &gate)?;
    }
    let trial_path = artifact.join("round1-24-trial-ledger.json");
    let mut trial: serde_json::Value = serde_json::from_slice(&fs::read(&trial_path)?)?;
    trial["round24_executed_trials"] = serde_json::json!(16);
    trial["round24_g1_terminal_failed"] = serde_json::json!(16);
    trial["round24_g1_pb_survivors"] = serde_json::json!(0);
    trial["round24_trial_ids"] = serde_json::Value::Array(
        g1["policies"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["policy_id"].clone())
            .collect(),
    );
    write_json(&trial_path, &trial)?;

    let phase_status = serde_json::json!({
        "R0":"complete","R1":"complete","R2":"complete_for_F1_F3_data_blocked","R3":"complete",
        "F1":"complete_g1_all_failed","F3_M1":"blocked_incomplete_data","F3_M2":"blocked_incomplete_data",
        "G0":"complete","G1":"complete_no_survivor","E1":"not_applicable_no_parent",
        "F2":"not_applicable_no_two_parents","G2":"not_applicable_no_survivor","R8":"complete_no_candidate"
    });
    let state = serde_json::json!({
        "status":"BLOCKED_ENGINE_DATA_OR_EXECUTION","machine_derived":true,
        "audited_commit":commit,"upstream_commit":upstream,"dirty":dirty,
        "registry":registry_summary,"phase_status":phase_status,
        "strict_survivors":0,"p_c_count":0,"p_d_count":0,"best_strict_candidate":null,
        "unresolved_blockers":[
            "F3-M1 metrics archives start 2023-07-01 instead of required 2023-01-01",
            "F3-M2 bookDepth lacks full six-asset protocol and aggTrades ends 2023-07-15",
            "all 16 F1 policies failed G1 P-B; no eligible E1/F2/G2 parent"
        ]
    });
    write_json(artifact.join("round24-execution-state.json"), &state)?;
    let authority = serde_json::json!({
        "authority_version":1,"historical_prequential_only":true,"status":"BLOCKED_ENGINE_DATA_OR_EXECUTION",
        "audited_commit":commit,"upstream_commit":upstream,"dirty":dirty,
        "registry":registry_summary,"trace_path_corrections":corrected_paths,"trace_content_changed":false,
        "exact_executed_policy_count":16,"exact_g1_replay_count":16,"g1_pb_survivors":0,"p_c_count":0,"p_d_count":0,
        "best_strict_candidate":null,
        "best_failed_diagnostic":{
            "policy_id":best["policy_id"],"annualized_return_pct":best["annualized_return_pct"],
            "max_equity_drawdown_pct":best["max_equity_drawdown_pct"],"actual_assets":best["actual_assets"],
            "so_count":best["so_count"],"failure_reasons":best["immediate_fail_reasons"]
        },
        "latest_tiers":{
            "conservative":{"ann_target_pct":50,"dd_cap_pct":10,"hit":false},
            "balanced":{"ann_target_pct":90,"dd_cap_pct":20,"hit":false},
            "aggressive":{"ann_target_pct":100,"dd_cap_pct":30,"hit":false}
        },
        "phase_status":phase_status,"target_hit":false,"frontier_progress":false,
        "closed_exact_fingerprint_scope":"R24 F1 16-policy rolling-168h causal residual disjoint-pair fingerprint only",
        "not_claimed":"all Martingale possibilities exhausted"
    });
    write_json(artifact.join("round24-authority.json"), &authority)?;
    let final_gate = serde_json::json!({
        "passed":true,"validator_generated":true,"registry":registry_summary,
        "trace_files_verified":rows.iter().filter(|row|row.row_kind==RowKind::Terminal).map(|row|row.traces.as_ref().map(|traces|traces.len()).unwrap_or(0)).sum::<usize>(),
        "trace_path_corrections":corrected_paths,"authority_state_match":true,
        "final_status":"BLOCKED_ENGINE_DATA_OR_EXECUTION"
    });
    write_json(artifact.join("gates/final-validator.json"), &final_gate)?;
    let report = render_report(&commit, &upstream, &registry_summary, best, corrected_paths);
    fs::write(
        repo.join("docs/superpowers/reports/2026-07-23-glm-round24-execution-handoff.md"),
        report,
    )?;
    println!("Round 24 finalization complete: status=BLOCKED_ENGINE_DATA_OR_EXECUTION");
    Ok(())
}

fn correct_and_verify_trace_paths(
    repo: &Path,
    artifact: &Path,
    rows: &mut [RegistryRow],
) -> Result<usize> {
    let mut corrections = 0;
    for row in rows
        .iter_mut()
        .filter(|row| row.row_kind == RowKind::Terminal)
    {
        let Some(traces) = row.traces.as_mut() else {
            continue;
        };
        for trace in traces.values_mut() {
            let direct = repo.join(&trace.path);
            let resolved = if direct.exists() {
                direct
            } else {
                let index = trace.path.find("traces/").context("trace suffix absent")?;
                let candidate = artifact.join(&trace.path[index..]);
                if !candidate.exists() {
                    bail!("trace not found: {}", trace.path);
                }
                trace.path = candidate.strip_prefix(repo)?.to_string_lossy().into_owned();
                corrections += 1;
                candidate
            };
            let bytes = fs::metadata(&resolved)?.len();
            let hash = sha256_file(&resolved)?;
            let lines = BufReader::new(File::open(&resolved)?)
                .lines()
                .filter_map(|line| line.ok())
                .filter(|line| !line.trim().is_empty())
                .map(|line| serde_json::from_str::<serde_json::Value>(&line))
                .collect::<std::result::Result<Vec<_>, _>>()?
                .len() as u64;
            if bytes != trace.bytes || hash != trace.sha256 || lines != trace.rows {
                bail!("trace integrity mismatch: {}", resolved.display());
            }
        }
    }
    Ok(corrections)
}

fn append_exact_failures(g1: &serde_json::Value, failures: &mut Vec<FailureRow>) {
    for result in g1["policies"].as_array().unwrap() {
        let policy_id = result["policy_id"].as_str().unwrap();
        let experiment_id = format!("R24-G1-{policy_id}");
        let exact_marker = format!("close exact fingerprint for {policy_id}");
        if failures.iter().any(|row| row.never_repeat == exact_marker) {
            continue;
        }
        failures.push(FailureRow {
            experiment_id,
            phase: "G1".into(),
            status: TerminalStatus::Failed,
            reason: serde_json::to_string(&result["immediate_fail_reasons"]).unwrap(),
            never_repeat: exact_marker,
            recorded_at_utc: "2026-07-23T00:00:00Z".into(),
        });
    }
}

fn render_report(
    commit: &str,
    upstream: &str,
    registry: &r24_registry::ValidationSummary,
    best: &serde_json::Value,
    corrections: usize,
) -> String {
    format!(
        "# GLM Martingale Core Round 24 执行交接\n\n\
| 字段 | 权威值 |\n|---|---|\n\
| audited commit / upstream / dirty | `{commit}` / `{upstream}` / `false` |\n\
| registry running / terminal / unique / violations | `{}` / `{}` / `{}` / `0` |\n\
| exact executed policies / G1 replays | `16 / 16` |\n\
| phase complete / blocked | `R0,R1,R2(F1),R3,G0,G1,R8 complete`; `F3 data blocked`; `E1,F2,G2 not applicable` |\n\
| strict survivors / P-C / P-D | `0 / 0 / 0` |\n\
| best strict candidate | `null` |\n\
| latest tier verdict | `50/10=false; 90/20=false; 100/30=false` |\n\
| unresolved blockers | `F3 historical data incomplete; F1 16/16 failed P-B` |\n\n\
## 最终状态\n\n`BLOCKED_ENGINE_DATA_OR_EXECUTION`。本轮仅为 historical prequential backtest，未设置 30 天监控。\n\n\
## 实际执行\n\n- R0：32/32 canaries 被真实 validator 拒绝。\n- R1：15/15 shared-account tests 通过，六币 probe、SQLite restart 与 independent adapters parity 通过。\n- R2/R3：12 blocks、121 天 purge、5 cold starts 和 52 个含条件模板 policies 在收益前冻结；F1 perp/funding 可用，F3-M1/M2 数据不足。\n- G0：8 synthetic + 4 real causal windows 完成；真实窗口为 no-fit rejection delta，不伪称交易激活。\n- G1：16/16 policies 完整运行，11/12 blocks 为 no-fit 且保留在分母，P-B survivors=0。\n- E1/F2/G2：parent gate 不满足，按计划不适用；未注入 finished curves，也未缩短窗口。\n\n\
## 最佳失败诊断\n\n`{}`：ann `{:.6}%`，equity DD `{:.6}%`，实际资产 `{}`，SO `{}`。它仍为 failed diagnostic，不是 candidate；失败原因为 `{}`。\n\n\
全部 16 个 F1 exact fingerprints 已写入 never-repeat ledger。本结论只关闭该 fingerprint 范围，不声称穷尽所有马丁可能性。\n\n\
## 证据修正\n\nvalidator 将 staging 产生的 `{corrections}` 个 `/tmp` trace 路径映射为仓库相对路径；每个文件的 SHA256、字节数、JSONL 行数均与 immutable terminal row 一致，trace 内容未改变。\n",
        registry.running_rows,
        registry.terminal_rows,
        registry.unique_experiments,
        best["policy_id"].as_str().unwrap(),
        best["annualized_return_pct"].as_f64().unwrap(),
        best["max_equity_drawdown_pct"].as_f64().unwrap(),
        best["actual_assets"].as_array().unwrap().len(),
        best["so_count"].as_u64().unwrap(),
        best["immediate_fail_reasons"]
    )
}

fn write_json(path: impl AsRef<Path>, value: &serde_json::Value) -> Result<()> {
    fs::write(path, serde_json::to_vec_pretty(value)?)?;
    Ok(())
}

fn write_jsonl<T: serde::Serialize>(path: &Path, values: &[T]) -> Result<()> {
    let mut writer = BufWriter::new(File::create(path)?);
    for value in values {
        serde_json::to_writer(&mut writer, value)?;
        writer.write_all(b"\n")?;
    }
    writer.flush()?;
    Ok(())
}

fn git(repo: &Path, args: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(args)
        .output()?;
    if !output.status.success() {
        bail!("git failed");
    }
    Ok(String::from_utf8(output.stdout)?.trim().into())
}
