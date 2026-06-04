# Martingale Indicator Integration & Walk-Forward Anti-Overfitting Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 在现有搜索框架中引入 ATR 自适应间距、ATR 动态止盈/止损、ADX 趋势过滤入场，并加入 walk-forward 样本外验证，确保回测结果不过拟合。

**Architecture:** 三层改动：(1) 搜索空间扩展 — 在 `StagedMartingaleSearchSpace` 和 `CoarseParameterPoint` 中新增 ATR/ADX 相关维度；(2) 候选生成器改造 — `build_single_direction_candidate` / `strategy_from_leg_params` 支持 ATR spacing/TP 和 ADX entry trigger；(3) Walk-forward 验证 — 在搜索完成 top 候选后，用训练集/测试集拆分验证年化和回撤的一致性。

**Tech Stack:** Rust `backtest-engine`, Rust `backtest-worker`, PostgreSQL, Docker Compose.

---

## Anti-Overfitting Principles (MUST enforce)

1. **指标只能用历史已知的值**：ATR/ADX 在 bar N 的值只能用 bar <= N 的数据计算（当前引擎已满足 — `IndicatorRuntimeContext` 累积到当前 bar）。
2. **搜索维度不能包含未来信息**：ATR period、ADX threshold 等是参数，不是从数据中自适应选择的。每个参数组合在所有 symbol 上使用相同的参数值。
3. **Walk-forward 验证**：对最终选出的 top 组合，必须在样本外数据上验证。如果训练集 Ann/DD 与测试集 Ann/DD 偏差超过阈值（如训练集 Ann/DD > 2x 测试集），该组合应被标记为"过拟合风险"。
4. **参数空间合理范围**：ATR multiplier 范围 [0.5, 3.0]、ADX threshold [15, 40]、ATR period [7, 28] — 不做极端值搜索。
5. **不对特定 symbol 做参数调优**：所有候选使用同一组搜索参数，不在搜索结果上做 symbol 级别的二次调参。

---

## Task 1: 扩展搜索空间 — 新增 ATR/ADX 维度

**Files:**
- Modify: `apps/backtest-engine/src/search.rs:143-164` (StagedMartingaleSearchSpace + CoarseParameterPoint)

**Goal:** 在搜索空间中新增 `spacing_model`、`take_profit_model`、`atr_period`、`atr_multiplier`、`adx_filter_enabled`、`adx_threshold` 维度。

### 1a. 扩展 StagedMartingaleSearchSpace

```rust
// search.rs — add to StagedMartingaleSearchSpace:
pub struct StagedMartingaleSearchSpace {
    // ... existing 7 fields unchanged ...
    pub spacing_model: Vec<SpacingModelChoice>,
    pub take_profit_model: Vec<TakeProfitModelChoice>,
    pub atr_period: Vec<u32>,
    pub atr_spacing_multiplier_bps: Vec<u32>, // stored as bps of atr_multiplier (e.g. 15000 = 1.5x)
    pub atr_tp_multiplier_bps: Vec<u32>,      // stored as bps of atr_multiplier
    pub adx_filter_enabled: Vec<bool>,
    pub adx_threshold_bps: Vec<u32>,          // stored as bps (e.g. 2500 = 25.0)
    pub adx_period: Vec<u32>,
}
```

### 1b. 新增枚举类型

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SpacingModelChoice {
    FixedPercent,
    Atr,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TakeProfitModelChoice {
    Percent,
    Atr,
}
```

### 1c. 扩展 CoarseParameterPoint

```rust
pub struct CoarseParameterPoint {
    // ... existing 8 fields unchanged ...
    pub spacing_model: SpacingModelChoice,
    pub take_profit_model: TakeProfitModelChoice,
    pub atr_period: u32,
    pub atr_spacing_multiplier_bps: u32,
    pub atr_tp_multiplier_bps: u32,
    pub adx_filter_enabled: bool,
    pub adx_threshold_bps: u32,
    pub adx_period: u32,
}
```

### 1d. 更新 for_profile 和 profit_optimized_v2

为三种 risk profile 设置合理的 ATR/ADX 默认搜索范围：

**conservative:**
- `spacing_model: [FixedPercent, Atr]`
- `take_profit_model: [Percent, Atr]`
- `atr_period: [7, 14, 21]`
- `atr_spacing_multiplier_bps: [10000, 15000, 20000]` (1.0x, 1.5x, 2.0x ATR)
- `atr_tp_multiplier_bps: [10000, 15000, 20000]`
- `adx_filter_enabled: [true, false]` (conservative 偏好 ADX 过滤)
- `adx_threshold_bps: [2000, 2500, 3000]` (20, 25, 30)
- `adx_period: [14, 21]`

**balanced:**
- `spacing_model: [FixedPercent, Atr]`
- `take_profit_model: [Percent, Atr]`
- `atr_period: [7, 14, 21, 28]`
- `atr_spacing_multiplier_bps: [8000, 12000, 16000, 20000]`
- `atr_tp_multiplier_bps: [8000, 12000, 16000, 20000]`
- `adx_filter_enabled: [true, false]`
- `adx_threshold_bps: [1500, 2000, 2500, 3000]`
- `adx_period: [14, 21]`

**aggressive:**
- `spacing_model: [FixedPercent, Atr]`
- `take_profit_model: [Percent, Atr]`
- `atr_period: [7, 14, 21, 28]`
- `atr_spacing_multiplier_bps: [5000, 8000, 12000, 16000, 20000, 30000]`
- `atr_tp_multiplier_bps: [5000, 8000, 12000, 16000, 20000, 30000]`
- `adx_filter_enabled: [false]` (aggressive 不太需要 ADX 过滤，但保留选项)
- `adx_threshold_bps: [1500, 2000]`
- `adx_period: [14]`

- [ ] **Step 1:** 在 `search.rs` 中定义 `SpacingModelChoice` 和 `TakeProfitModelChoice` 枚举
- [ ] **Step 2:** 扩展 `StagedMartingaleSearchSpace` 和 `CoarseParameterPoint` 结构体
- [ ] **Step 3:** 更新 `for_profile` 三种 risk profile 的默认值
- [ ] **Step 4:** 更新 `profit_optimized_v2` 扩展搜索范围
- [ ] **Step 5:** 更新 `fine_space_around` 对新维度的邻域生成
- [ ] **Step 6:** 编译验证 `cargo check -p backtest-engine`
- [ ] **Step 7:** 运行现有搜索测试确保不 break `cargo test -p backtest-engine --lib`

---

## Task 2: 改造候选生成器 — 支持 ATR/ADX 参数

**Files:**
- Modify: `apps/backtest-engine/src/search.rs:287-629` (generate_staged_candidates_for_symbol, build_single_direction_candidate, strategy_from_leg_params)

**Goal:** 候选生成器根据搜索空间的 `spacing_model`、`take_profit_model`、`adx_filter_enabled` 等参数，生成使用 ATR spacing/TP 和 ADX entry trigger 的 `MartingaleStrategyConfig`。

### 2a. 更新 generate_staged_candidates_for_symbol

在 long_short 和 single-direction 两个分支中，随机采样新增的维度：
- `smi`: spacing_model index
- `tpmi`: take_profit_model index
- `api`: atr_period index
- `asmi`: atr_spacing_multiplier_bps index
- `atpmi`: atr_tp_multiplier_bps index
- `afei`: adx_filter_enabled index
- `athi`: adx_threshold_bps index
- `adpi`: adx_period index

注意：当 `spacing_model == FixedPercent` 时，ATR 相关参数仍需记录到候选中（但不影响 spacing 计算，ATR period 仍然用于 indicators 列表以便引擎计算 ATR 供其他用途）。当 `adx_filter_enabled == false` 时，不添加 ADX entry trigger。

### 2b. 更新 build_single_direction_candidate

函数签名新增参数：
```rust
fn build_single_direction_candidate(
    // ... existing params ...
    spacing_model: SpacingModelChoice,
    take_profit_model: TakeProfitModelChoice,
    atr_period: u32,
    atr_spacing_multiplier_bps: u32,
    atr_tp_multiplier_bps: u32,
    adx_filter_enabled: bool,
    adx_threshold_bps: u32,
    adx_period: u32,
) -> Result<SearchCandidate, String>
```

构建 `MartingaleStrategyConfig` 时：
- `spacing`: 根据 `spacing_model` 选择 `FixedPercent { step_bps }` 或 `Atr { multiplier, min_step_bps, max_step_bps }`
  - ATR spacing: `multiplier = atr_spacing_multiplier_bps / 10000.0`, `min_step_bps = spacing_bps / 2`, `max_step_bps = spacing_bps * 3`
- `take_profit`: 根据 `take_profit_model` 选择 `Percent { bps }` 或 `Atr { multiplier }`
  - ATR TP: `multiplier = atr_tp_multiplier_bps / 10000.0`
- `indicators`: 添加 `Atr { period: atr_period }`。如果 `adx_filter_enabled`，也添加 `Adx { period: adx_period }`
- `entry_triggers`: 保留 `Cooldown { seconds: 21600 }`。如果 `adx_filter_enabled`，添加 `IndicatorExpression { expression: format!("adx({adx_period}) > {adx_threshold}", adx_threshold = adx_threshold_bps as f64 / 100.0) }`

### 2c. 更新 strategy_from_leg_params (long_short path)

同样的逻辑，但 long 和 short 各自独立选择 spacing_model 和 take_profit_model。
注意：为了减少搜索空间爆炸，long_short 模式下两个方向共享相同的 ATR/ADX 参数（period、multiplier、threshold），但可以有不同的 spacing_bps 和 take_profit_bps。

### 2d. 验证约束

当 `spacing_model == Atr` 时，必须确保 `atr_period >= 2`（ATR 最少需要 2 根 K 线）。如果 `atr_spacing_multiplier_bps == 0`，降级为 FixedPercent。

- [ ] **Step 1:** 更新 `LegParameters` 新增 ATR/ADX 字段
- [ ] **Step 2:** 更新 `generate_staged_candidates_for_symbol` long_short 分支
- [ ] **Step 3:** 更新 `generate_staged_candidates_for_symbol` single-direction 分支
- [ ] **Step 4:** 更新 `build_single_direction_candidate` 函数
- [ ] **Step 5:** 更新 `strategy_from_leg_params` 函数
- [ ] **Step 6:** 更新 `fine_space_around` 对新维度的处理
- [ ] **Step 7:** 编译验证 `cargo check -p backtest-engine`
- [ ] **Step 8:** 运行所有测试 `cargo test -p backtest-engine --lib`

---

## Task 3: ATR Spacing 约束验证

**Files:**
- Modify: `apps/backtest-engine/src/search.rs:456-466` (is_valid_fixed_percent_spacing)

**Goal:** 新增 `is_valid_atr_spacing` 验证函数，确保 ATR spacing 参数合理。

```rust
fn is_valid_atr_spacing(
    direction: MartingaleDirection,
    min_step_bps: u32,
    max_step_bps: u32,
    max_legs: u32,
) -> bool {
    if min_step_bps > max_step_bps {
        return false;
    }
    let max_distance_bps = max_step_bps.saturating_mul(max_legs);
    match direction {
        MartingaleDirection::Long => max_distance_bps < 9_500,
        MartingaleDirection::Short => max_distance_bps <= 30_000,
    }
}
```

- [ ] **Step 1:** 实现 `is_valid_atr_spacing`
- [ ] **Step 2:** 在候选生成器中调用此验证
- [ ] **Step 3:** 编写测试 `atr_spacing_validation_rejects_invalid_bounds`
- [ ] **Step 4:** 运行测试

---

## Task 4: 集成测试 — ATR/ADX 候选端到端验证

**Files:**
- Add tests to: `apps/backtest-engine/src/search.rs` (staged_tests mod)

**Goal:** 验证新生成的 ATR/ADX 候选可以被引擎正确执行。

### 4a. 测试用例

1. `atr_spacing_candidate_generates_valid_config` — 验证 ATR spacing 候选的 `MartingaleStrategyConfig.spacing` 是 `Atr { .. }` variant
2. `atr_take_profit_candidate_generates_valid_config` — 验证 ATR TP 候选的 `take_profit` 是 `Atr { .. }` variant
3. `adx_filter_candidate_has_indicator_expression_trigger` — 验证 ADX 过滤候选的 `entry_triggers` 包含 `IndicatorExpression`
4. `mixed_atr_percent_candidates_both_appear` — 验证当搜索空间包含 FixedPercent 和 Atr 时，两种候选都会生成

- [ ] **Step 1:** 编写 4 个测试
- [ ] **Step 2:** 运行测试 `cargo test -p backtest-engine --lib -- staged_tests`
- [ ] **Step 3:** 修复任何失败

---

## Task 5: Walk-Forward 验证框架集成

**Files:**
- Modify: `apps/backtest-engine/src/portfolio_search.rs` (portfolio search scoring)
- Modify: `apps/backtest-worker/src/main.rs` (worker — add walk-forward step)

**Goal:** 在 portfolio search 完成后，对 top 候选自动执行 walk-forward 验证，在结果中输出 WFE (Walk-Forward Efficiency) 指标。

### 5a. Walk-Forward 配置

对于我们的数据范围 (2023-01-01 到 2026-04-30)：
- **Train**: 12 months
- **Validate**: 3 months  
- **Test**: 3 months
- **Step**: 6 months

生成约 4-5 个窗口，每个窗口覆盖 18 个月。

### 5b. WFE 计算

```
WFE = (Test Ann/DD) / (Train Ann/DD)
```

- WFE >= 0.5: 良好 — 测试集保持至少一半的风险调整收益
- WFE >= 0.3: 可接受 — 有一定衰减但策略仍然有效
- WFE < 0.3: 过拟合风险 — 训练集表现无法在测试集复现

### 5c. Worker 集成点

在 worker 的 `search_symbol` 阶段完成后，新增 `walk_forward_validation` 阶段：
1. 取 top 3 候选
2. 对每个候选，生成 walk-forward 窗口
3. 在每个窗口的 train + test 区间分别执行回测
4. 计算 WFE
5. 将 WFE 写入 summary JSON

### 5d. 在 portfolio scoring 中使用 WFE

在组合评分时，对 WFE < 0.3 的候选施加惩罚（降低 score），确保过拟合的候选不会进入最终组合。

- [ ] **Step 1:** 在 `portfolio_search.rs` 中新增 `WalkForwardValidation` 结构体和 `compute_wfe` 函数
- [ ] **Step 2:** 在 `time_splits.rs` 中新增 `default_backtest_walk_forward_config` 便捷函数
- [ ] **Step 3:** 在 `backtest-worker/main.rs` 中新增 `walk_forward_validation` 阶段
- [ ] **Step 4:** 在组合评分中集成 WFE 惩罚
- [ ] **Step 5:** 编写 WFE 计算的单元测试
- [ ] **Step 6:** 运行所有测试

---

## Task 6: 搜索空间大小验证 — 确保可执行

**Goal:** 验证新增维度后搜索空间不会爆炸。

### 计算

现有搜索空间（balanced, long_short, profit_optimized_v2）：
- leverage: 10 × spacing: 12 × multiplier: 10 × max_legs: 8 × tp: 10 × tail: 10 × weight: 9 = 8,640,000
- 实际采样限制为 ~500 候选/symbol，所以不穷举

新增维度（保守估计，balanced）：
- spacing_model: 2 × tp_model: 2 × atr_period: 4 × atr_spacing_mult: 4 × atr_tp_mult: 4 × adx_enabled: 2 × adx_threshold: 4 × adx_period: 2 = 4,096
- 新增因子: 4,096x
- 采样候选数不变（~500/symbol），只是参数空间更大，随机采样覆盖更稀疏

**结论：** 由于使用随机采样而非穷举，搜索空间增大不影响执行时间，只影响覆盖率。保持每 symbol 500 候选不变。

- [ ] **Step 1:** 计算 conservative/balanced/aggressive 各自的理论搜索空间大小
- [ ] **Step 2:** 确认 worker 配置（候选数限制）不变
- [ ] **Step 3:** 预估 ATR/ADX 候选的回测执行时间是否增加（ATR 需要指标计算，但当前引擎已经逐 bar 累积，只是多用一个指标函数）

---

## Task 7: 重新构建 Docker 镜像并部署

**Files:**
- Modify: Docker image (cargo build)

- [ ] **Step 1:** `cargo build --release -p backtest-worker`
- [ ] **Step 2:** 重新构建 Docker 镜像
- [ ] **Step 3:** 重启 workers
- [ ] **Step 4:** 提交一个测试任务验证 ATR/ADX 候选能正常跑通

---

## Task 8: 搜索执行 — 三档重跑

**Goal:** 用新的搜索空间重新跑三档搜索。

### 8a. Conservative (目标: DD<=10%, Ann>50%)

- 使用新的 ATR spacing + ATR TP + ADX 过滤
- ATR 自适应间距期望在低波动期减少间距（更频繁止盈），高波动期拉宽间距（避免被止损）
- ADX 趋势过滤期望过滤掉震荡期入场（减少无效开仓）
- 组合搜索 6 个 seed，每个 500 候选/symbol × 18 symbol

### 8b. Balanced (目标: DD<=20%, Ann 尽量高)

- 在现有结果基础上（seed53 Ann=65.52%）尝试超越
- ATR TP 可能提供更好的止盈位置
- 6 个 seed

### 8c. Aggressive (目标: DD<=30%, Ann 尽量高)

- 在现有结果基础上（seed173 Ann=77.00%）尝试超越
- 6 个 seed

- [ ] **Step 1:** 创建搜索任务（18 个任务 = 3 risk × 6 seed）
- [ ] **Step 2:** 提交到 worker 队列
- [ ] **Step 3:** 等待完成并收集结果
- [ ] **Step 4:** 对 top 组合做 walk-forward 验证
- [ ] **Step 5:** 输出最终结果表（包含 WFE 指标）

---

## Key Invariants

1. **Fee/slip 双向扣除** — 不变
2. **加权平均成本** — 不变  
3. **组合 DD 从组合曲线** — 不变
4. **ATR/ADX 只用历史数据** — 引擎已满足（push_bar 累积到当前 bar）
5. **参数搜索用随机采样** — 不穷举，不过拟合参数空间
6. **Walk-forward WFE 作为过拟合检测** — 新增
7. **所有搜索参数对所有 symbol 一致** — 不做 per-symbol 调参

---

## Risk Assessment

| 风险 | 概率 | 影响 | 缓解 |
|------|------|------|------|
| ATR 候选回测时间大幅增加 | 低 | 中 | ATR 计算复杂度 O(N*P)，P<=28，与现有指标同数量级 |
| 搜索空间过大导致覆盖率不足 | 中 | 中 | 保持 500 候选/symbol，必要时增加到 1000 |
| Conservative 仍无法达到 DD<=10% Ann>50% | 高 | 低 | 这是结构性限制，ATR/ADX 可能缓解但不一定突破 |
| ADX 过滤导致交易次数过少 | 中 | 中 | ADX threshold 搜索范围 [15, 30]，保守值不会过度过滤 |
| Walk-forward 验证发现所有 top 组合都过拟合 | 低 | 高 | 说明需要更稳健的策略框架，而非参数调优 |
