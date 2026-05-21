# 马丁扩展币种收益优先深搜与组合降波动 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 扩展马丁回测币种池、提升单策略收益搜索质量，并把组合器升级为基于完整资金曲线的风险约束组合优化，尽量在硬回撤限制内提高年化收益。

**Architecture:** 保持现有 Rust 回测核心和 worker 架构，在 `backtest-engine` 内增强搜索空间与组合优化器，在 `backtest-worker` 内增加扩展币种准入、候选池分层、输出摘要，在前端只做结果解释和展示增强。所有正式指标继续来自完整 1m K 线状态机，不引入未来函数、不跳过手续费/滑点/杠杆保证金语义。

**Tech Stack:** Rust workspace (`backtest-engine`, `backtest-worker`, `api-server`, `shared-db`), SQLite readonly market data, React/Next.js frontend components, Cargo tests, existing Docker deployment.

---

## File Map

- `apps/backtest-engine/src/search.rs`：定义 staged search space、fine search 邻域、参数范围测试。
- `apps/backtest-engine/src/portfolio_search.rs`：组合候选结构、组合枚举/评分、完整资金曲线合成、组合 TopN 输出。
- `apps/backtest-engine/src/martingale/metrics.rs`：如需新增月度/分段指标或曲线采样工具，在这里保持指标语义集中。
- `apps/backtest-worker/src/main.rs`：任务配置归一化、市场数据加载、单币种深搜流程、候选池分层、组合 artifact 写入。
- `apps/api-server/src/services/backtest_service.rs`：自动搜索默认配置、最大币种数和请求归一化。
- `apps/web/components/backtest/backtest-wizard.tsx`：如果前端需要入口，增加“扩展币种深搜”选项与说明。
- `apps/web/components/backtest/backtest-console.tsx`：展示单策略 Top10、组合候选池、组合 Top10 与诊断文案。
- `apps/web/components/backtest/portfolio-candidate-review.tsx`：展示组合成员权重、单币种总权重、入选原因。
- `apps/web/lib/api-types.ts` 或现有 API 类型文件：如后端新增字段，补齐类型。
- `docs/superpowers/specs/2026-05-21-martingale-expanded-universe-profit-portfolio-design.md`：本计划对应 spec，实施中发现范围冲突先更新 spec。

---

### Task 1: 扩展币种池准入与任务配置

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`
- Modify: `apps/api-server/src/services/backtest_service.rs`
- Test: `apps/backtest-worker/src/main.rs` inline tests
- Test: `apps/api-server/src/services/backtest_service.rs` inline tests

- [ ] **Step 1: Add failing worker tests for expanded universe defaults**

在 `apps/backtest-worker/src/main.rs` 的 `#[cfg(test)] mod tests` 中增加测试，要求扩展池只包含完整历史候选，并保持 7 币种显式输入不被替换。

```rust
#[test]
fn expanded_universe_defaults_include_only_full_history_futures_symbols() {
    let symbols = default_expanded_universe_symbols();
    assert!(symbols.len() >= 18, "expected at least 18 symbols, got {symbols:?}");
    for required in [
        "BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "DOGEUSDT", "XRPUSDT", "ADAUSDT",
        "ZECUSDT", "DASHUSDT", "NEARUSDT", "BCHUSDT", "LINKUSDT", "AVAXUSDT", "UNIUSDT",
        "FILUSDT", "DOTUSDT", "AAVEUSDT", "INJUSDT",
    ] {
        assert!(symbols.contains(&required.to_owned()), "missing {required}: {symbols:?}");
    }
    for excluded in ["SUIUSDT", "1000PEPEUSDT", "ONDOUSDT", "TONUSDT", "WLDUSDT", "ENAUSDT"] {
        assert!(!symbols.contains(&excluded.to_owned()), "short-history symbol should not be default: {excluded}");
    }
}

#[test]
fn explicit_symbols_are_not_replaced_by_expanded_universe() {
    let mut config = worker_task_config_fixture();
    config.symbols = vec!["BTCUSDT".to_owned(), "ETHUSDT".to_owned()];
    config.extended_universe = Some(true);

    let effective = effective_search_symbols(&config);
    assert_eq!(effective, vec!["BTCUSDT".to_owned(), "ETHUSDT".to_owned()]);
}
```

如果当前没有 `worker_task_config_fixture()`，新增一个只在 tests 内使用的 fixture，字段按现有 `WorkerTaskConfig` 最小可编译值填写。

- [ ] **Step 2: Run worker tests and confirm failure**

Run: `cargo test -p backtest-worker expanded_universe -- --nocapture`

Expected before implementation: FAIL because `default_expanded_universe_symbols` / `effective_search_symbols` does not exist.

- [ ] **Step 3: Implement default expanded universe helpers**

在 `apps/backtest-worker/src/main.rs` 增加纯函数，避免直接散落常量。

```rust
fn default_expanded_universe_symbols() -> Vec<String> {
    [
        "BTCUSDT", "ETHUSDT", "BNBUSDT", "SOLUSDT", "DOGEUSDT", "XRPUSDT", "ADAUSDT",
        "ZECUSDT", "DASHUSDT", "NEARUSDT", "BCHUSDT", "LINKUSDT", "AVAXUSDT", "UNIUSDT",
        "FILUSDT", "DOTUSDT", "AAVEUSDT", "INJUSDT",
    ]
    .into_iter()
    .map(str::to_owned)
    .collect()
}

fn effective_search_symbols(config: &WorkerTaskConfig) -> Vec<String> {
    if !config.symbols.is_empty() {
        return config.symbols.clone();
    }
    if config.extended_universe.unwrap_or(false) {
        return default_expanded_universe_symbols();
    }
    config.symbols.clone()
}
```

如果 `WorkerTaskConfig` 没有 `extended_universe` 字段，新增：

```rust
#[serde(default)]
extended_universe: Option<bool>,
```

然后把 worker 中所有直接使用 `task.config.symbols` 作为待搜索币种入口的位置改为 `effective_search_symbols(&task.config)`，但不要改变 artifact 中原始请求字段。

- [ ] **Step 4: Add API normalization test**

在 `apps/api-server/src/services/backtest_service.rs` 测试模块增加：

```rust
#[test]
fn auto_search_allows_extended_universe_without_explicit_symbols() {
    let mut config = serde_json::json!({
        "mode": "auto_search",
        "extended_universe": true,
        "market": "futures",
        "direction": "long_short",
        "risk_profile": "aggressive"
    });

    let normalized = normalize_martingale_auto_search_config(config.take()).unwrap();
    assert_eq!(normalized.get("extended_universe").and_then(|v| v.as_bool()), Some(true));
    assert_eq!(normalized.get("market").and_then(|v| v.as_str()), Some("futures"));
}
```

- [ ] **Step 5: Implement API normalization**

在 `normalize_martingale_auto_search_config()` 中保留 `extended_universe: true`，并调整 `effective_task_symbols()` 的校验：当 `strategy_type == "martingale_grid"` 且配置里 `extended_universe == true` 时，允许 `request.symbols` 为空，由 worker 注入默认扩展池。仍保持普通任务必须有 symbols。

- [ ] **Step 6: Verify Task 1**

Run:

```bash
cargo test -p backtest-worker expanded_universe -- --nocapture
cargo test -p api-server auto_search_allows_extended_universe_without_explicit_symbols -- --nocapture
```

Expected: both PASS.

- [ ] **Step 7: Commit Task 1**

```bash
git add apps/backtest-worker/src/main.rs apps/api-server/src/services/backtest_service.rs
git commit -m "feat: 增加马丁扩展币种池准入" -m "问题描述：扩展深搜需要在不手动输入 18 个币种的情况下使用完整历史合约币种池。" -m "修复思路：新增 extended_universe 配置和 worker 默认完整历史币种池，显式 symbols 优先。"
```

---

### Task 2: 单策略收益优先深搜 v2

**Files:**
- Modify: `apps/backtest-engine/src/search.rs`
- Modify: `apps/backtest-worker/src/main.rs`
- Test: `apps/backtest-engine/src/search.rs` inline tests
- Test: `apps/backtest-worker/src/main.rs` inline tests

- [ ] **Step 1: Add failing tests for widened parameter coverage**

在 `apps/backtest-engine/src/search.rs` 的 staged tests 增加：

```rust
#[test]
fn aggressive_profit_search_v2_covers_wide_spacing_and_profit_targets() {
    let space = StagedMartingaleSearchSpace::profit_optimized_v2("aggressive", "long_short");
    assert!(space.leverage.contains(&10));
    assert!(space.spacing_bps.iter().any(|v| *v <= 35));
    assert!(space.spacing_bps.iter().any(|v| *v >= 600));
    assert!(space.take_profit_bps.iter().any(|v| *v <= 30));
    assert!(space.take_profit_bps.iter().any(|v| *v >= 300));
    assert!(space.max_legs.contains(&9));
    assert!(space.multiplier_x100.iter().any(|v| *v <= 115));
    assert!(space.multiplier_x100.iter().any(|v| *v >= 240));
}
```

如果字段名与当前不同，使用当前 `StagedMartingaleSearchSpace` 实际字段名，但断言含义不能变。

- [ ] **Step 2: Add failing worker test for tail coverage in selection**

在 `apps/backtest-worker/src/main.rs` tests 增加：

```rust
#[test]
fn profit_optimized_v2_selection_keeps_tail_parameter_candidates() {
    let space = StagedMartingaleSearchSpace::profit_optimized_v2("aggressive", "long_short");
    let candidates = generate_long_short_staged_candidates_for_test(&space, "BTCUSDT", 96);

    assert!(candidates.iter().any(|candidate| candidate_has_spacing_at_least(candidate, 600)));
    assert!(candidates.iter().any(|candidate| candidate_has_take_profit_at_least(candidate, 300)));
    assert!(candidates.iter().any(|candidate| candidate_has_leverage(candidate, 10)));
}
```

如果没有 test helper，新增只在 tests 内使用的 helpers：遍历 `candidate.config.strategies`，读取 spacing/take_profit/leverage。

- [ ] **Step 3: Run tests and confirm failure**

Run:

```bash
cargo test -p backtest-engine aggressive_profit_search_v2_covers_wide_spacing_and_profit_targets -- --nocapture
cargo test -p backtest-worker profit_optimized_v2_selection_keeps_tail_parameter_candidates -- --nocapture
```

Expected before implementation: FAIL because `profit_optimized_v2` or helpers do not exist / parameter tail missing.

- [ ] **Step 4: Implement profit optimized v2 search space**

在 `apps/backtest-engine/src/search.rs` 给 `StagedMartingaleSearchSpace` 增加构造函数：

```rust
impl StagedMartingaleSearchSpace {
    pub fn profit_optimized_v2(risk_profile: &str, direction_mode: &str) -> Self {
        let mut space = Self::for_profile(risk_profile, direction_mode);
        space.leverage = vec![2, 3, 4, 5, 6, 8, 10];
        space.spacing_bps = vec![35, 50, 70, 90, 120, 160, 220, 300, 420, 600];
        space.multiplier_x100 = vec![115, 125, 140, 160, 180, 200, 220, 240];
        space.max_legs = vec![3, 4, 5, 6, 7, 8, 9];
        space.take_profit_bps = vec![30, 45, 60, 80, 100, 140, 200, 300];
        if direction_mode == "long_short" {
            space.long_short_weight_pct = vec![20, 30, 40, 50, 60, 70, 80];
        }
        space.tail_stop_bps = vec![800, 1200, 1800, 2400, 3000, 4000, 5500, 7000];
        space
    }
}
```

如果当前结构字段命名不同，按实际字段改写。不要删除现有 `for_profile()`，避免影响普通任务。

- [ ] **Step 5: Wire worker to opt into v2 for extended/profit mode**

在 `apps/backtest-worker/src/main.rs` 中新增判断：

```rust
fn should_use_profit_optimized_v2(config: &WorkerTaskConfig) -> bool {
    config.extended_universe.unwrap_or(false)
        || config.search_mode.as_deref() == Some("profit_optimized_v2")
}
```

在构造 staged space 的位置改为：

```rust
let staged = if should_use_profit_optimized_v2(&config) {
    StagedMartingaleSearchSpace::profit_optimized_v2(&config.risk_profile, direction_mode)
} else {
    StagedMartingaleSearchSpace::for_profile(&config.risk_profile, direction_mode)
};
```

如果 `WorkerTaskConfig` 没有 `search_mode` 字段，新增：

```rust
#[serde(default)]
search_mode: Option<String>,
```

- [ ] **Step 6: Improve survivor selection without increasing memory unboundedly**

在 `run_long_short_staged_search()` 或相关 selection 函数中，保证 survivor 至少覆盖：

```rust
fn profit_v2_survivor_limit(task: &WorkerTaskConfig) -> usize {
    if should_use_profit_optimized_v2(task) {
        task.per_symbol_top_n.max(20).min(80)
    } else {
        long_short_survivor_limit(task)
    }
}
```

使用该函数替换 v2 路径 survivor truncate。保留现有全局超时和 `max_threads` 控制。

- [ ] **Step 7: Verify Task 2**

Run:

```bash
cargo test -p backtest-engine aggressive_profit_search_v2_covers_wide_spacing_and_profit_targets -- --nocapture
cargo test -p backtest-worker profit_optimized_v2_selection_keeps_tail_parameter_candidates -- --nocapture
cargo test -p backtest-worker long_short_candidate_selection_prioritizes_profit_potential_within_budget -- --nocapture
```

Expected: all PASS.

- [ ] **Step 8: Commit Task 2**

```bash
git add apps/backtest-engine/src/search.rs apps/backtest-worker/src/main.rs
git commit -m "feat: 增强马丁收益优先深搜空间" -m "问题描述：现有参数空间可能漏掉高收益尾部组合，扩展币种深搜需要覆盖更宽间隔、止盈、层数和杠杆。" -m "修复思路：新增 profit_optimized_v2 搜索空间并在扩展深搜模式下提高 survivor 覆盖。"
```

---

### Task 3: 组合候选池分层输出

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`
- Modify: `apps/backtest-engine/src/portfolio_search.rs`
- Test: `apps/backtest-worker/src/main.rs` inline tests

- [ ] **Step 1: Add failing tests for candidate pool tiers**

在 `apps/backtest-worker/src/main.rs` tests 增加：

```rust
#[test]
fn portfolio_pool_keeps_qualified_high_return_and_low_drawdown_tiers_per_symbol() {
    let outputs = vec![
        candidate_output_fixture("btc-safe", "BTCUSDT", 18.0, 8.0, 100.0),
        candidate_output_fixture("btc-growth", "BTCUSDT", 80.0, 42.0, 100.0),
        candidate_output_fixture("btc-loss", "BTCUSDT", -5.0, 4.0, 100.0),
        candidate_output_fixture("eth-safe", "ETHUSDT", 12.0, 6.0, 100.0),
        candidate_output_fixture("eth-growth", "ETHUSDT", 70.0, 38.0, 100.0),
    ];

    let pool = select_portfolio_pool_outputs_v2(outputs, 25.0, 10, 10, 5);
    let ids: std::collections::BTreeSet<_> = pool.iter().map(|o| o.candidate_id.as_str()).collect();

    assert!(ids.contains("btc-safe"));
    assert!(ids.contains("btc-growth"));
    assert!(ids.contains("eth-safe"));
    assert!(ids.contains("eth-growth"));
    assert!(!ids.contains("btc-loss"));
}
```

若 `candidate_output_fixture` 不存在，新增 tests-only fixture，填充 `CandidateOutput` 所需字段，`total_return_pct`、`annualized_return_pct`、`max_drawdown_pct`、`planned_margin_quote`、`equity_curve` 使用最小有效值。

- [ ] **Step 2: Run test and confirm failure**

Run: `cargo test -p backtest-worker portfolio_pool_keeps_qualified_high_return_and_low_drawdown_tiers_per_symbol -- --nocapture`

Expected: FAIL because `select_portfolio_pool_outputs_v2` does not exist.

- [ ] **Step 3: Implement pool tier selector**

在 `apps/backtest-worker/src/main.rs` 增加：

```rust
fn select_portfolio_pool_outputs_v2(
    outputs: Vec<CandidateOutput>,
    drawdown_limit_pct: f64,
    qualified_top_n: usize,
    growth_top_n: usize,
    low_drawdown_top_n: usize,
) -> Vec<CandidateOutput> {
    let mut by_symbol = std::collections::BTreeMap::<String, Vec<CandidateOutput>>::new();
    for output in outputs {
        if output.total_return_pct <= 0.0 {
            continue;
        }
        by_symbol.entry(output.symbol.clone()).or_default().push(output);
    }

    let mut selected = Vec::<CandidateOutput>::new();
    let mut seen = std::collections::BTreeSet::<String>::new();
    for (_symbol, mut items) in by_symbol {
        items.sort_by(|a, b| output_profit_score(b).total_cmp(&output_profit_score(a)));
        for output in items.iter().filter(|o| o.max_drawdown_pct <= drawdown_limit_pct).take(qualified_top_n) {
            if seen.insert(output.candidate_id.clone()) {
                selected.push(output.clone());
            }
        }

        items.sort_by(|a, b| b.annualized_return_pct.unwrap_or(b.total_return_pct).total_cmp(&a.annualized_return_pct.unwrap_or(a.total_return_pct)));
        for output in items.iter().take(growth_top_n) {
            if seen.insert(output.candidate_id.clone()) {
                selected.push(output.clone());
            }
        }

        items.sort_by(|a, b| a.max_drawdown_pct.total_cmp(&b.max_drawdown_pct));
        for output in items.iter().take(low_drawdown_top_n) {
            if seen.insert(output.candidate_id.clone()) {
                selected.push(output.clone());
            }
        }
    }
    selected
}
```

如果已有 `output_profit_score`，复用；否则新增简单版本：年化优先、回撤惩罚。

- [ ] **Step 4: Replace portfolio pool selection call**

在 `process_task()` 中把组合池选择改为：

```rust
let portfolio_pool_outputs = if should_use_profit_optimized_v2(&task.config) {
    select_portfolio_pool_outputs_v2(outputs.clone(), max_portfolio_drawdown_pct, 10, 10, 5)
} else {
    select_portfolio_pool_outputs(outputs.clone(), max_portfolio_drawdown_pct, &task.config.risk_profile)
};
```

确保 `display_outputs` 仍使用严格单策略回撤过滤，避免前端把高回撤组合专用候选误认为单策略合格 Top10。

- [ ] **Step 5: Add artifact summary fields**

在写 summary 的 JSON 中加入：

```rust
"portfolio_pool_candidate_count": portfolio_pool_outputs.len(),
"portfolio_pool_note": "positive-return candidates include qualified, high-return and low-drawdown tiers; final portfolio still enforces hard drawdown limit",
```

- [ ] **Step 6: Verify Task 3**

Run:

```bash
cargo test -p backtest-worker portfolio_pool_keeps_qualified_high_return_and_low_drawdown_tiers_per_symbol -- --nocapture
cargo test -p backtest-worker candidate_outputs_keep_top_five_per_symbol_and_enrich_summary -- --nocapture
```

Expected: PASS.

- [ ] **Step 7: Commit Task 3**

```bash
git add apps/backtest-worker/src/main.rs
git commit -m "feat: 分层保留马丁组合候选池" -m "问题描述：组合优化需要同时使用合格低回撤候选和高收益高回撤候选，现有池可能过早丢弃互补策略。" -m "修复思路：按币种分层保留单策略合格、高收益、低回撤候选，负收益仍剔除，组合结果继续硬控回撤。"
```

---

### Task 4: 完整资金曲线组合优化器 v2

**Files:**
- Modify: `apps/backtest-engine/src/portfolio_search.rs`
- Test: `apps/backtest-engine/src/portfolio_search.rs` inline tests

- [ ] **Step 1: Add failing tests for high/low risk complementary portfolio**

在 `apps/backtest-engine/src/portfolio_search.rs` tests 增加：

```rust
#[test]
fn portfolio_v2_combines_high_return_with_low_drawdown_stabilizer_under_hard_limit() {
    let high = candidate_with_curve(
        "btc-growth",
        "BTCUSDT",
        120.0,
        55.0,
        8.0,
        100.0,
        vec![100.0, 180.0, 125.0, 230.0],
    );
    let low = candidate_with_curve(
        "eth-stable",
        "ETHUSDT",
        18.0,
        6.0,
        2.0,
        100.0,
        vec![100.0, 103.0, 106.0, 118.0],
    );
    let loss = candidate_with_curve(
        "ada-loss",
        "ADAUSDT",
        -5.0,
        3.0,
        1.0,
        100.0,
        vec![100.0, 99.0, 98.0, 95.0],
    );

    let artifact = build_portfolio_top_n_v2(&[high, low, loss], 30.0, 10);
    let first = artifact.top3.first().expect("expected complementary portfolio");

    assert!(first.max_drawdown_pct <= 30.0, "portfolio must obey hard drawdown: {first:?}");
    assert!(first.members.iter().any(|m| m.candidate_id == "btc-growth"));
    assert!(first.members.iter().any(|m| m.candidate_id == "eth-stable"));
    assert!(first.members.iter().all(|m| m.candidate_id != "ada-loss"));
}
```

- [ ] **Step 2: Add failing test for top10 output**

```rust
#[test]
fn portfolio_v2_can_return_top_ten_ranked_portfolios() {
    let mut candidates = Vec::new();
    for index in 0..12 {
        let symbol = if index % 3 == 0 { "BTCUSDT" } else if index % 3 == 1 { "ETHUSDT" } else { "SOLUSDT" };
        candidates.push(candidate_with_curve(
            &format!("c{index}"),
            symbol,
            20.0 + index as f64 * 3.0,
            5.0 + index as f64,
            2.0,
            100.0,
            vec![100.0, 105.0 + index as f64, 110.0 + index as f64],
        ));
    }

    let artifact = build_portfolio_top_n_v2(&candidates, 30.0, 10);
    assert!(artifact.top3.len() >= 3);
    assert!(artifact.all_portfolios.as_ref().map(|items| items.len()).unwrap_or(0) >= 10);
}
```

如果 `PortfolioTop3Artifact` 没有 `all_portfolios`，本任务会新增可选字段并保持 `top3` 兼容。

- [ ] **Step 3: Run tests and confirm failure**

Run: `cargo test -p backtest-engine portfolio_v2 -- --nocapture`

Expected: FAIL because `build_portfolio_top_n_v2` / `all_portfolios` does not exist.

- [ ] **Step 4: Extend artifact struct compatibly**

在 `PortfolioTop3Artifact` 增加字段：

```rust
#[serde(default, skip_serializing_if = "Option::is_none")]
pub all_portfolios: Option<Vec<WeightedPortfolio>>,
```

确保已有构造点都填 `all_portfolios: None` 或由 v2 填 Some。

- [ ] **Step 5: Implement v2 entrypoint**

新增：

```rust
pub fn build_portfolio_top_n_v2(
    candidates: &[EvaluatedCandidate],
    max_drawdown_pct: f64,
    top_n: usize,
) -> PortfolioTop3Artifact {
    let mut artifact = build_portfolio_top3(candidates, max_drawdown_pct);
    let top_n = top_n.max(3).min(10);
    let all = build_ranked_portfolios_v2(candidates, max_drawdown_pct, top_n);
    artifact.top3 = all.iter().take(3).cloned().collect();
    artifact.eligible_candidate_count = candidates.iter().filter(|c| c.return_pct > 0.0).count();
    artifact.all_portfolios = Some(all);
    artifact
}
```

- [ ] **Step 6: Implement ranked portfolio v2 search**

新增私有函数，核心要求：

```rust
fn build_ranked_portfolios_v2(
    candidates: &[EvaluatedCandidate],
    max_drawdown_pct: f64,
    top_n: usize,
) -> Vec<WeightedPortfolio> {
    let eligible: Vec<&EvaluatedCandidate> = candidates
        .iter()
        .filter(|candidate| candidate.return_pct > 0.0 && candidate.planned_margin_quote > 0.0)
        .collect();

    if eligible.len() < 2 {
        return Vec::new();
    }

    let seed_indices = best_indices_by_symbol(&eligible, 8);
    let templates = allocation_templates_v2();
    let mut scored = Vec::<WeightedPortfolio>::new();

    enumerate_combinations_v2(
        &eligible,
        &seed_indices,
        &templates,
        max_drawdown_pct,
        &mut scored,
    );

    scored.sort_by(|a, b| b.score.total_cmp(&a.score));
    dedupe_portfolios_by_member_weight(&mut scored);
    scored.truncate(top_n.max(3).min(10));
    scored
}
```

实现细节必须满足：

- 成员数支持 `2..=8`，但为了性能每个 symbol 只取前 `8` 个 seed。
- 权重模板包含高低风险互补，如 `[0.7,0.3]`, `[0.6,0.4]`, `[0.5,0.3,0.2]`, `[0.4,0.3,0.2,0.1]`, `[0.3,0.25,0.2,0.15,0.1]`。
- 调用现有 `build_weighted_portfolio(..., max_drawdown_pct)` 计算完整曲线和硬回撤。
- score 使用 `annualized / max_drawdown` 为主，叠加低相关/成员数分散奖励，但不得让超过回撤限制的组合进入结果。

- [ ] **Step 7: Add correlation/drawdown overlap penalty helpers**

新增 helpers，输入 `WeightedPortfolio` 或候选成员曲线：

```rust
fn daily_return_correlation_penalty(members: &[(&EvaluatedCandidate, f64)]) -> f64 {
    let correlations = pairwise_curve_correlations(members);
    if correlations.is_empty() {
        return 1.0;
    }
    let avg = correlations.iter().sum::<f64>() / correlations.len() as f64;
    if avg > 0.8 { 0.85 } else if avg > 0.6 { 0.93 } else { 1.0 }
}
```

如果当前 equity point 时间粒度不方便按日聚合，可先使用采样点收益相关性；但测试中必须覆盖高相关惩罚。

- [ ] **Step 8: Verify Task 4**

Run:

```bash
cargo test -p backtest-engine portfolio_v2 -- --nocapture
cargo test -p backtest-engine portfolio_search -- --nocapture
```

Expected: PASS.

- [ ] **Step 9: Commit Task 4**

```bash
git add apps/backtest-engine/src/portfolio_search.rs
git commit -m "feat: 升级马丁组合资金曲线优化器" -m "问题描述：组合器需要利用高收益高回撤与低回撤稳定策略互补，而不是只靠少量固定模板。" -m "修复思路：新增 TopN 组合搜索，按完整资金曲线合成并硬控回撤，加入分散与相关性惩罚。"
```

---

### Task 5: Worker 接入组合 v2 与结果摘要

**Files:**
- Modify: `apps/backtest-worker/src/main.rs`
- Modify: `apps/backtest-engine/src/portfolio_search.rs` only if Task 4 API needs tiny adjustment
- Test: `apps/backtest-worker/src/main.rs` inline tests

- [ ] **Step 1: Add failing test for v2 portfolio artifact fields**

在 `apps/backtest-worker/src/main.rs` tests 增加：

```rust
#[test]
fn extended_universe_summary_reports_portfolio_top10_and_pool_counts() {
    let summary = build_portfolio_summary_for_test(
        42,
        18,
        10,
        Some("positive-return candidates include qualified, high-return and low-drawdown tiers"),
    );

    assert_eq!(summary.get("portfolio_pool_candidate_count").and_then(|v| v.as_u64()), Some(42));
    assert_eq!(summary.get("expanded_universe_symbol_count").and_then(|v| v.as_u64()), Some(18));
    assert_eq!(summary.get("portfolio_top_n").and_then(|v| v.as_u64()), Some(10));
    assert!(summary.get("portfolio_pool_note").and_then(|v| v.as_str()).unwrap().contains("high-return"));
}
```

如果没有 summary helper，新增纯函数：

```rust
fn build_portfolio_summary(
    portfolio_pool_candidate_count: usize,
    expanded_universe_symbol_count: usize,
    portfolio_top_n: usize,
    note: &str,
) -> serde_json::Value
```

- [ ] **Step 2: Run test and confirm failure**

Run: `cargo test -p backtest-worker extended_universe_summary_reports_portfolio_top10_and_pool_counts -- --nocapture`

Expected: FAIL before helper/fields exist.

- [ ] **Step 3: Use portfolio v2 for profit optimized mode**

在 `apps/backtest-worker/src/main.rs` import：

```rust
use backtest_engine::portfolio_search::{build_portfolio_top3, build_portfolio_top_n_v2};
```

在 `process_task()` 中：

```rust
let portfolio_top_n = if should_use_profit_optimized_v2(&task.config) { 10 } else { 3 };
let portfolio_top3 = if should_use_profit_optimized_v2(&task.config) {
    build_portfolio_top_n_v2(&portfolio_candidates, max_portfolio_drawdown_pct, portfolio_top_n)
} else {
    build_portfolio_top3(&portfolio_candidates, max_portfolio_drawdown_pct)
};
```

保持变量名可以仍叫 `portfolio_top3`，但 artifact 内含 `all_portfolios`。

- [ ] **Step 4: Add summary fields in task result**

在 final summary JSON 中合并：

```rust
"expanded_universe_symbol_count": effective_symbols.len(),
"portfolio_top_n": portfolio_top_n,
"portfolio_pool_candidate_count": portfolio_pool_outputs.len(),
"portfolio_pool_note": "positive-return candidates include qualified, high-return and low-drawdown tiers; final portfolio still enforces hard drawdown limit",
```

- [ ] **Step 5: Ensure no single-candidate portfolio**

增加/保留断言逻辑：组合结果成员数 `< 2` 时不进入组合 TopN。若 `build_weighted_portfolio` 已处理，增加 worker 测试：

```rust
#[test]
fn portfolio_artifact_never_reports_single_member_as_combination() {
    let candidates = vec![portfolio_candidate_fixture("btc-only", "BTCUSDT", 50.0, 10.0)];
    let artifact = build_portfolio_top_n_v2(&candidates, 30.0, 10);
    assert!(artifact.top3.is_empty());
}
```

- [ ] **Step 6: Verify Task 5**

Run:

```bash
cargo test -p backtest-worker extended_universe_summary_reports_portfolio_top10_and_pool_counts -- --nocapture
cargo test -p backtest-worker long_short_task_produces_long_and_short_candidates_via_intelligent_search -- --nocapture
```

Expected: PASS.

- [ ] **Step 7: Commit Task 5**

```bash
git add apps/backtest-worker/src/main.rs apps/backtest-engine/src/portfolio_search.rs
git commit -m "feat: 接入马丁组合 Top10 输出" -m "问题描述：扩展深搜需要输出组合 Top10 与候选池诊断，便于比较收益和回撤。" -m "修复思路：worker 在收益优先模式下调用组合 v2，并写入扩展币种、候选池和 TopN 摘要。"
```

---

### Task 6: 前端结果解释与组合 Top10 展示

**Files:**
- Modify: `apps/web/components/backtest/backtest-console.tsx`
- Modify: `apps/web/components/backtest/portfolio-candidate-review.tsx`
- Modify: `apps/web/components/backtest/backtest-wizard.tsx`
- Modify: `apps/web/lib/api-types.ts` or actual API type file found by `rg "PortfolioTop3|MartingalePortfolio" apps/web/lib apps/web/components`
- Test: frontend build

- [ ] **Step 1: Add API type fields**

在现有 `MartingalePortfolioArtifact` 或对应类型中新增可选字段：

```ts
all_portfolios?: MartingaleWeightedPortfolio[];
portfolio_pool_candidate_count?: number;
expanded_universe_symbol_count?: number;
portfolio_top_n?: number;
portfolio_pool_note?: string;
```

如果 artifact summary 字段在 `summary` 内，类型应加到 `MartingaleBacktestCandidateSummary`：

```ts
portfolio_pool_candidate_count?: number;
expanded_universe_symbol_count?: number;
portfolio_top_n?: number;
portfolio_pool_note?: string;
```

- [ ] **Step 2: Add wizard toggle copy**

在 `backtest-wizard.tsx` 中增加扩展深搜说明；如果已有高级参数区，加入 checkbox：

```tsx
<label className="flex items-start gap-2 rounded-lg border border-slate-700 p-3 text-sm text-slate-200">
  <input
    type="checkbox"
    checked={form.extendedUniverse}
    onChange={(event) => setForm((prev) => ({ ...prev, extendedUniverse: event.target.checked }))}
  />
  <span>
    <span className="font-medium">扩展币种深搜</span>
    <span className="block text-slate-400">
      自动使用具备 2023-01-01 起完整 1m 合约数据的主流币种池，耗时更长，但更容易找到高收益/低回撤组合。
    </span>
  </span>
</label>
```

提交 payload 时加入：

```ts
extended_universe: form.extendedUniverse,
search_mode: form.extendedUniverse ? "profit_optimized_v2" : undefined,
```

- [ ] **Step 3: Display single strategy vs portfolio pool distinction**

在 `backtest-console.tsx` 结果区域增加提示卡：

```tsx
{summary.portfolio_pool_note ? (
  <div className="rounded-lg border border-amber-500/30 bg-amber-500/10 p-3 text-sm text-amber-100">
    <div className="font-medium">组合候选池说明</div>
    <div className="mt-1 text-amber-100/80">
      单策略 Top10 只展示自身满足回撤限制的策略；组合候选池还会保留正收益高弹性策略，最终组合仍会硬控最大回撤。
    </div>
  </div>
) : null}
```

- [ ] **Step 4: Render portfolio Top10 if available**

在组合展示组件中：

```tsx
const portfolios = artifact.all_portfolios?.length ? artifact.all_portfolios : artifact.top3;
```

展示标题：

```tsx
<h3>组合 Top{portfolios.length}</h3>
```

每个组合展示：年化、总收益、最大回撤、Calmar、成员数量、单币种最大权重。

- [ ] **Step 5: Show member contribution and reason**

在 `portfolio-candidate-review.tsx` 每个 member 下展示：

```tsx
<div className="text-xs text-slate-400">
  权重 {member.allocation_pct.toFixed(1)}% · {member.symbol} · 杠杆 {member.leverage ?? "-"}x
</div>
```

如后端没有 reason 字段，前端用规则生成：

```ts
function portfolioMemberReason(member: MartingalePortfolioMember): string {
  if ((member.max_drawdown_pct ?? 0) <= 10) return "低回撤稳定器";
  if ((member.annualized_return_pct ?? member.return_pct ?? 0) >= 50) return "高收益弹性成员";
  return "分散组合成员";
}
```

- [ ] **Step 6: Build frontend**

Run: `npm run build --workspace apps/web` or the repo's actual frontend build command. If workspace command differs, use `npm run build` from `apps/web`.

Expected: build exits 0.

- [ ] **Step 7: Commit Task 6**

```bash
git add apps/web/components/backtest/backtest-console.tsx apps/web/components/backtest/portfolio-candidate-review.tsx apps/web/components/backtest/backtest-wizard.tsx apps/web/lib/api-types.ts
git commit -m "feat: 展示马丁扩展深搜组合结果" -m "问题描述：前端需要区分单策略 Top10、组合候选池和组合 Top10，否则用户无法理解高低风险互补组合。" -m "修复思路：增加扩展深搜入口说明、组合候选池提示、组合 Top10 与成员权重/角色展示。"
```

---

### Task 7: 验证回测、部署与结果报告

**Files:**
- No source change expected unless validation exposes bug
- Use: `deploy/docker/docker-compose.yml`
- Use: `.worktrees/full-v1/.env`

- [ ] **Step 1: Run Rust regression tests**

Run:

```bash
cargo test -p backtest-engine --lib -- --nocapture
cargo test -p backtest-worker -- --nocapture
cargo test -p api-server martingale -- --nocapture
cargo fmt --check
```

Expected: all PASS.

- [ ] **Step 2: Build and redeploy changed services**

Run:

```bash
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env -f deploy/docker/docker-compose.yml build backtest-worker api-server web
docker compose --env-file /home/bumblebee/Project/grid_binance/.worktrees/full-v1/.env -f deploy/docker/docker-compose.yml up -d --no-deps --force-recreate backtest-worker api-server web
```

Expected: services rebuild and start successfully. Do not touch unrelated port 3000 service.

- [ ] **Step 3: Create 7-symbol baseline validation task**

Use existing API endpoint/payload pattern from previous validation. Payload requirements:

```json
{
  "strategy_type": "martingale_grid",
  "symbols": ["BTCUSDT", "ETHUSDT", "SOLUSDT", "BNBUSDT", "XRPUSDT", "DOGEUSDT", "ADAUSDT"],
  "config": {
    "mode": "auto_search",
    "market": "futures",
    "direction": "long_short",
    "risk_profile": "aggressive",
    "search_mode": "profit_optimized_v2",
    "per_symbol_top_n": 10,
    "portfolio_top_n": 10,
    "time_range_mode": "auto_since_2023_to_last_month_end"
  }
}
```

Expected: task succeeds; report whether it beats previous benchmark `43.95%` annualized / `29.32%` max drawdown.

- [ ] **Step 4: Create 18-symbol expanded validation task**

Payload requirements:

```json
{
  "strategy_type": "martingale_grid",
  "symbols": [],
  "config": {
    "mode": "auto_search",
    "extended_universe": true,
    "market": "futures",
    "direction": "long_short",
    "risk_profile": "aggressive",
    "search_mode": "profit_optimized_v2",
    "per_symbol_top_n": 10,
    "portfolio_top_n": 10,
    "time_range_mode": "auto_since_2023_to_last_month_end"
  }
}
```

Expected: task uses at least 18 effective symbols, succeeds, outputs portfolio Top10.

- [ ] **Step 5: Extract and save validation metrics**

For each task, extract:

- task_id
- status
- effective symbol count
- candidate count
- portfolio pool count
- Top1/Top3/Top10 annualized return
- Top1/Top3/Top10 max drawdown
- Top1 member symbols and weights
- whether max single symbol allocation <=80%
- whether annualized >=50%
- if not >=50%, best achieved annualized and limiting factor

Save report to:

`docs/superpowers/reports/2026-05-21-martingale-expanded-universe-validation.md`

- [ ] **Step 6: Commit validation report**

```bash
git add docs/superpowers/reports/2026-05-21-martingale-expanded-universe-validation.md
git commit -m "test: 记录马丁扩展币种深搜验证结果" -m "复现路径：运行 7 币种基准和 18 币种扩展池 long_short aggressive 回测，检查组合 Top10 年化收益与最大回撤。" -m "修复思路：用真实任务结果对比上一版基准，记录是否提升收益或降低回撤。"
```

- [ ] **Step 7: Push and final status**

Run:

```bash
git status --short
git push origin main
```

Expected: push succeeds; final `git status --short` is empty.

---

## Final Verification Checklist

- [ ] `cargo test -p backtest-engine --lib -- --nocapture` passes.
- [ ] `cargo test -p backtest-worker -- --nocapture` passes.
- [ ] `cargo test -p api-server martingale -- --nocapture` passes.
- [ ] Frontend build passes.
- [ ] 7-symbol benchmark task succeeds and reports comparison vs `43.95% / 29.32%`.
- [ ] 18-symbol expanded task succeeds and reports portfolio Top10.
- [ ] `long_short` outputs contain both long and short legs.
- [ ] Portfolio results have at least two members.
- [ ] Single-symbol allocation never exceeds `80%`.
- [ ] Portfolio max drawdown never exceeds risk profile hard limit.
- [ ] Negative-return candidates do not enter final portfolio results.
- [ ] Git working tree is clean and pushed.
