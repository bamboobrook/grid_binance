# Full-v1 Review Regression Fix Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** 收尾修复当前仍未完成的 3 个回归: `SpotSellOnly` 普通上侧网格顺序错误、普通上侧网格在归一化后丢失锚点首档、`referencePriceMode=market` 时生命周期按钮被误锁死；并完成最终回归验收。

**Architecture:** 已完成的修复任务已从本文件移除，这个版本只保留“还需要继续执行”的内容。剩余问题都集中在策略保存/归一化/UI 提交流程这一条链路上，所以优先在 `strategy_service.rs` 和 `strategy-workspace-form.tsx` 收口，随后跑一次覆盖 trading-engine、api-server、web 的最终回归，确认前面已经修好的项没有回退。

**Tech Stack:** Rust / Axum / shared-db / Next.js App Router / TypeScript / Cargo / Node test runner / Playwright（可选）

---

## 执行约束

- 本计划只针对 `.worktrees/full-v1`，不要在仓库根目录 `grid_binance` 上直接改代码。
- 本文件只保留剩余待执行项；已完成任务不要再重复实现。
- 目标是修回现有契约，不是扩功能。不要新增 endpoint，不要顺手重构 shared crate。
- 所有改动都要先补或扩现有测试，再动实现；至少补 Rust 侧行为测试和 Web 的 source-level contract test。

## 本轮代码评审必须关闭的 3 条问题

- `[P1] Recompute derived reference price after level normalization`：`apps/api-server/src/services/strategy_service.rs:1488` 附近的 `build_strategy()` 会在 `normalize_strategy_levels()` 前用原始 levels 推导 `reference_price`，普通单边网格遇到交易所 tick size 四舍五入后会留下过期锚点价，导致 `initial_entry_side()` 跳过首档锚点单。只允许重算“请求未显式传入 `reference_price`”的推导值。
- `[P2] Stop blocking lifecycle buttons on market reference preview`：`apps/web/components/strategies/strategy-workspace-form.tsx:913` 附近把除 delete 外的所有 intent 都绑定到 `marketReferenceSubmitBlocked`，导致行情预览 pending/失败时无法 pause/stop/resume/preflight 运行中策略。
- `[P2] Include SpotSellOnly in ascending level validation`：`apps/api-server/src/services/strategy_service.rs:1467` 附近的 `expected_level_direction()` 没把 `SpotSellOnly` 纳入升序，普通现货卖出上侧网格会被当成降序校验/归一化。

### Task 1: 修复普通上侧网格顺序、归一化后的锚点价，以及市价模式下被误锁死的生命周期按钮

**Files:**
- Modify: `apps/api-server/src/services/strategy_service.rs`
- Modify: `apps/web/components/strategies/strategy-workspace-form.tsx`
- Test: `apps/api-server/tests/strategy_flow.rs`
- Test: `apps/api-server/src/services/strategy_service.rs`
- Test: `tests/verification/strategy_surface_contract.test.mjs` 或 `tests/verification/web_app_shell.test.mjs`

- [x] **Step 1: 先修 `SpotSellOnly` 的执行顺序，让普通上侧网格保持升序**

`expected_level_direction()` 现在仍然只把 `FuturesShort` 放在 `Ascending`，`SpotSellOnly` 还被落到 `Descending`。这里要改成:

- `ClassicBilateralGrid` 继续走 `Ascending`
- `SpotClassic` / `FuturesNeutral` 继续走 `Ascending`
- `SpotSellOnly` / `FuturesShort` 都走 `Ascending`
- `SpotBuyOnly` / `FuturesLong` 保持 `Descending`

修完后，`validate_strategy_request()` 和 `normalize_strategy_levels()` 的顺序校验会自动跟着生效，现货上侧 ordinary grid 不会再被保存成 `[high -> low]`。

建议替换为这种显式 match，避免以后再漏模式:

```rust
fn expected_level_direction(strategy_type: StrategyType, mode: StrategyMode) -> LevelDirection {
    if matches!(strategy_type, StrategyType::ClassicBilateralGrid) {
        return LevelDirection::Ascending;
    }

    match mode {
        StrategyMode::SpotClassic
        | StrategyMode::SpotSellOnly
        | StrategyMode::FuturesNeutral
        | StrategyMode::FuturesShort => LevelDirection::Ascending,
        StrategyMode::SpotBuyOnly | StrategyMode::FuturesLong => LevelDirection::Descending,
    }
}
```

- [x] **Step 2: 把”由 levels 推导出来的 `reference_price`”改成基于归一化后 levels 重算**

当前 `build_strategy()`、`update_strategy()`、`build_template()` 都在 `normalize_strategy_levels()` 前先算了 `reference_price`，导致像 `100.009 -> 100` 这种 tick-size 归一化后，锚点价还停在旧值，首档会被 `initial_entry_side()` 跳过。

修法要求:

- 如果请求显式传了 `reference_price`，保留显式值，不要改写
- 如果 `reference_price` 是从 levels 推导出来的，必须在 `normalize_strategy_levels()` 之后用归一化后的 `draft_revision.levels` 或 `template.levels` 重算并写回 revision
- create / update / template 三条持久化链都要一起修，不要只修单一路径；`create_strategy_from_template()` 如仍绕过归一化，也要确认不会保存未归一化模板派生策略

推荐做法是抽一个只处理“推导型 reference price”的小 helper，在归一化后统一回写，避免复制三份逻辑。

可执行实现锚点:

- 在 `reference_price_for_request()` 附近新增 helper，例如 `fn derived_reference_price_from_grid_levels(levels: &[GridLevel]) -> Decimal`，逻辑与当前 `reference_price_for_request()` 的 fallback 一致，取归一化后首个 `entry_price`。
- 在 `StrategyService::create_strategy()` 中，`self.normalize_strategy_levels(&mut strategy)?;` 之后调用 helper：只有 `request.reference_price.is_none()` 时，写回 `strategy.draft_revision.reference_price`。
- 在 `StrategyService::update_strategy()` 中同样在 `self.normalize_strategy_levels(&mut strategy)?;` 之后回写推导型 `strategy.draft_revision.reference_price`。
- `build_template()` 目前不是 `StrategyService` 方法，不能直接调用 `normalize_strategy_levels(&mut Strategy)`。实现时二选一：
  - 把“按 symbol/market 归一化 levels 并重算 budget/spacing/reference”的逻辑拆成可复用内部函数，让策略与模板共用；或
  - 在创建模板的 service 方法里，先构造临时 `Strategy` 复用 `normalize_strategy_levels()`，再把归一化后的 revision/levels/budget/spacing/reference 写回模板。
- 不要在 `build_revision()` 内无条件重算 `reference_price`，否则会覆盖显式 `reference_price`。
- 不要只比较 `reference_price_source`，显式与推导的判断以 `request.reference_price.is_some()` 为准；`reference_price_source="manual"` 也可能没传具体 `reference_price`。

- [x] **Step 3: 扩已有 Rust 测试，把剩余两个策略保存回归锁死**

优先扩现有测试，不要平地起新测试文件。至少覆盖:

- `apps/api-server/tests/strategy_flow.rs` 的 `ordinary_grid_normalization_keeps_anchor_first_after_exchange_rounding`
- `apps/api-server/src/services/strategy_service.rs` 的 `create_strategy_quantizes_levels_to_exchange_filters_when_metadata_exists`

新增断言目标:

- `SpotSellOnly` 创建后 `levels` 仍然是从低到高
- `SpotBuyOnly` / `SpotSellOnly` / `FuturesShort` 在价格归一化后，未显式传入的 `draft_revision.reference_price` 与归一化后的首格一致，例如 `100.009` 被 tick size 归一化成 `100.00` 后 reference 也必须是 `100.00`
- 显式传入 `reference_price` 时，归一化后不能被 helper 覆盖
- 如果实现上顺手，直接断言 `rebuild_runtime()` 或启动/恢复后产出的 working order 没丢锚点首档会更稳

建议在 `apps/api-server/tests/strategy_flow.rs` 的 `ordinary_grid_normalization_keeps_anchor_first_after_exchange_rounding` 中补这些断言:

```rust
assert_eq!(
    spot_body["draft_revision"]["reference_price"],
    spot_body["draft_revision"]["levels"][0]["entry_price"]
);

let spot_sell_created = create_strategy(
    &app,
    &user_token,
    json!({
        "name": "Spot Sell Ordinary Normalized",
        "symbol": "BTCUSDT",
        "market": "Spot",
        "mode": "SpotSellOnly",
        "generation": "Custom",
        "strategy_type": "ordinary_grid",
        "reference_price_source": "manual",
        "levels": [
            grid_level("100.009", "0.0104", 120, None),
            grid_level("104.004", "0.0104", 120, None),
            grid_level("108.001", "0.0104", 120, None)
        ],
        "overall_take_profit_bps": 500,
        "overall_stop_loss_bps": 800,
        "post_trigger_action": "Stop"
    }),
)
.await;
assert_eq!(spot_sell_created.status(), StatusCode::CREATED);
let spot_sell_body = response_json(spot_sell_created).await;
assert_eq!(spot_sell_body["draft_revision"]["levels"][0]["entry_price"], "100.00");
assert_eq!(spot_sell_body["draft_revision"]["levels"][1]["entry_price"], "104.00");
assert_eq!(spot_sell_body["draft_revision"]["levels"][2]["entry_price"], "108.00");
assert_eq!(
    spot_sell_body["draft_revision"]["reference_price"],
    spot_sell_body["draft_revision"]["levels"][0]["entry_price"]
);
```

建议在 `apps/api-server/src/services/strategy_service.rs` 的 `create_strategy_quantizes_levels_to_exchange_filters_when_metadata_exists` 中补一个显式 reference 保留用例，避免 helper 误伤:

```rust
let explicit_reference = strategy_with_explicit_reference.draft_revision.reference_price;
assert_eq!(explicit_reference, dec!(101.23));
```

实际变量名按测试现有结构调整；重点是先构造一个带 `reference_price: Some(dec!(101.23))` 的 request，再断言归一化后 reference 没被首档价格覆盖。

- [x] **Step 4: 缩小 `marketReferenceSubmitBlocked` 的作用范围，只锁真正依赖当前表单行情值的提交**

`apps/web/components/strategies/strategy-workspace-form.tsx` 现在仍然是“除了 delete 以外全禁用”。要改成按 intent 区分:

- 继续受限: `save`、无 intent 的新建/创建提交、其他确实依赖当前表单 `referencePrice` 的保存类动作
- 必须解锁: `pause`、`stop`、`start`、`resume`、`preflight`、`delete`

原因: 这些动作在 `apps/web/app/api/user/strategies/[id]/route.ts` 里根本不读取表单 `referencePrice`，所以 `/api/market/preview` pending/失败时，用户仍然必须能操作运行中策略。

建议新增小 helper，并在 wizard 与非 wizard 两处按钮共用，禁止继续写 `button.value === "delete" ? false : marketReferenceSubmitBlocked`:

```tsx
  const marketReferenceBlockedIntents = new Set<string | undefined>([undefined, "save", "create", "draft"]);
  const isSubmitBlockedByMarketReference = (intent?: string) => (
    marketReferenceSubmitBlocked && marketReferenceBlockedIntents.has(intent)
  );
```

然后两处按钮统一改成:

```tsx
disabled={isSubmitBlockedByMarketReference(button.value)}
```

如果当前 `intentButtons` 的创建/保存 intent 名称不是 `create` / `draft`，以实际 `intentRow` 传入值为准补进集合；生命周期 intent 一律不能进入集合。

- [x] **Step 5: 补 Web source-level contract test，锁住按钮禁用边界**

在 `tests/verification/strategy_surface_contract.test.mjs` 或 `tests/verification/web_app_shell.test.mjs` 至少补 3 类断言:

- 市价模式下仍保留 `Pause Strategy` / `Stop Strategy` / `Resume` 等生命周期入口
- 生命周期按钮不再统一复用 `marketReferenceSubmitBlocked`，源码中不能再出现 `button.value === "delete" ? false : marketReferenceSubmitBlocked`
- 保存类动作仍会受 `marketReferenceSubmitBlocked` 保护

建议用源码级断言，避免引入浏览器依赖。示例可加到 `tests/verification/strategy_surface_contract.test.mjs`:

```js
test("market reference preview only blocks save-like form submits", () => {
  const source = readFileSync("apps/web/components/strategies/strategy-workspace-form.tsx", "utf8");
  assert.match(source, /isSubmitBlockedByMarketReference/);
  assert.doesNotMatch(source, /button\.value === "delete" \? false : marketReferenceSubmitBlocked/);
  assert.match(source, /marketReferenceBlockedIntents/);
  assert.match(source, /"save"/);
  assert.doesNotMatch(source, /marketReferenceBlockedIntents[^;]+"pause"/s);
  assert.doesNotMatch(source, /marketReferenceBlockedIntents[^;]+"stop"/s);
  assert.doesNotMatch(source, /marketReferenceBlockedIntents[^;]+"resume"/s);
  assert.doesNotMatch(source, /marketReferenceBlockedIntents[^;]+"preflight"/s);
});
```

如果测试文件已经有类似 read helper，复用现有 helper，不要重复造轮子。

- [x] **Step 6: 先跑这组剩余问题对应的定向验证**

Run: `source "$HOME/.cargo/env" && cargo test -p api-server --test strategy_flow ordinary_grid_normalization_keeps_anchor_first_after_exchange_rounding -- --nocapture`

Expected: PASS。

Run: `source "$HOME/.cargo/env" && cargo test -p api-server create_strategy_quantizes_levels_to_exchange_filters_when_metadata_exists -- --nocapture`

Expected: PASS。

Run: `source "$HOME/.cargo/env" && cargo test -p api-server --test strategy_flow spot_sell -- --nocapture`

Expected: 如果没有独立 `spot_sell` 测试名，可以跳过；若新增了独立测试则必须 PASS。

Run: `node --test tests/verification/strategy_surface_contract.test.mjs tests/verification/web_app_shell.test.mjs`

Expected: PASS。

### Task 2: 最终回归验收

**Files:**
- Modify: `docs/superpowers/plans/2026-04-23-full-v1-review-regression-fix-plan.md`（仅在执行时回填勾选状态）

- [x] **Step 1: 跑核心 Rust 回归，确认已修项目没有回退**

Run: `source "$HOME/.cargo/env" && cargo test -p trading-engine -- --nocapture && cargo test -p api-server --test strategy_flow --test notification_flow -- --nocapture`

Expected: 全绿；至少不能再出现当前剩余 3 个问题，也不能让前面已修好的运行态恢复、Telegram、通知偏好、列表兼容、tags/notes、backtest 代理回退。

- [x] **Step 2: 跑 Web 静态回归**

Run: `pnpm --filter web exec tsc --noEmit && node --test tests/verification/web_app_shell.test.mjs tests/verification/strategy_surface_contract.test.mjs tests/verification/strategy_runtime_recovery_contract.test.mjs`

Expected: PASS。

- [ ] **Step 3: 如环境已启动，做最小人工烟测**

至少确认 7 条路径:

- `/en/app/backtest` 提交表单后不会立即报 500
- `SpotSellOnly` 普通上侧网格保存后仍保持从低到高，启动/预检不是只剩一档卖单
- `FuturesShort` 或 `SpotSellOnly` 在价格归一化后，首档锚点单不会被 `reference_price` 漂移吃掉
- 详情页使用 `referencePriceMode=market` 时，即使 `/api/market/preview` pending/失败，仍能执行暂停、停止、恢复与预检
- 策略数 >20 时，详情页保存/启动/暂停第 21 条策略仍可命中
- 绑定 Telegram 且配置 bot token 时，runtime TP/SL/error 会留下 telegram log
- 新建策略与编辑策略都能保存 `tags` / `notes`

- [ ] **Step 4: 提交前写清楚 commit log**

如果要提交 Git，commit message 或补充说明里至少写出以下三类信息中的一类，最好三类都带上:

- 问题描述
- 复现路径
- 修复思路

不要只写空泛的 `fix bug`。

## 额外提醒

- 当前剩余问题都在同一条“ordinary grid 保存 -> 归一化 -> reference price -> runtime entry side -> UI 提交控制”链路上，修的时候要一起看，不要只打一行补丁。
- `Task 2` 不是走形式。即便前面的功能已经写完，没跑最终回归之前，这份计划仍然不能算完成。

## 2026-04-24 复核新增问题：全量 API 回归仍未绿

当前 3 条评审问题的定向验证已通过，但我在完整复核时跑 `source "$HOME/.cargo/env" && cargo test -p api-server --tests` 仍发现 `exchange_flow` 失败，所以不要宣称整个程序已完成。

已确认通过:

- `source "$HOME/.cargo/env" && cargo test -p api-server --test strategy_flow ordinary_grid_normalization_keeps_anchor_first_after_exchange_rounding -- --nocapture`：PASS，覆盖归一化后派生 `reference_price`、`SpotSellOnly` 上侧顺序、首档锚点不漂移。
- `node --test tests/verification/strategy_surface_contract.test.mjs tests/verification/web_app_shell.test.mjs`：PASS，覆盖 market preview 只阻塞保存类提交，不再误锁生命周期按钮。

仍需修复:

- [x] **Step A: 修复 `exchange_flow` 里缺省 `strategy_type` 与 `SpotClassic` 的兼容回归**

复现命令:

```bash
source "$HOME/.cargo/env" && cargo test -p api-server --test exchange_flow credential_updates_require_running_strategies_to_be_paused_first -- --nocapture
```

当前结果: `apps/api-server/tests/exchange_flow.rs:297` 创建策略期望 `201`，实际 `400`。该用例 payload 是 `market=Spot` + `mode=SpotClassic` + 未显式传 `strategy_type`。现在 `SaveStrategyRequest.strategy_type` 默认是 `OrdinaryGrid`，而 `strategy_type_matches_mode()` 禁止 `OrdinaryGrid + SpotClassic`，导致旧用例/旧客户端被拒。

修复要求:

- 不要放宽 `strategy_type_matches_mode()` 到允许错误组合；应在请求规范化阶段兼容缺省策略类型。
- 当请求未显式提交 `strategy_type` 且 `mode` 是 `SpotClassic` 或 `FuturesNeutral` 时，将 `strategy_type` 规范化为 `ClassicBilateralGrid`。
- 当请求显式提交了 `strategy_type` 时，继续按现有规则校验；例如显式 `ordinary_grid + SpotClassic` 仍应报错。
- 推荐在 `normalize_strategy_request()` 或其前置 hint 逻辑里处理，避免 create/update/template 路径分叉。

- [x] **Step B: 补一条 API 回归测试锁住兼容行为**

可在 `apps/api-server/tests/strategy_flow.rs` 或 `apps/api-server/src/services/strategy_service.rs` 单元测试中补:

- 旧 payload 不带 `strategy_type`、`mode=SpotClassic` 时可以创建，并最终保存为 `classic_bilateral_grid`。
- 显式 `strategy_type=ordinary_grid`、`mode=SpotClassic` 时仍返回 `400`，防止误放宽。

如果使用 `strategy_flow`，优先覆盖 HTTP 行为；如果用 service 单测，重点断言 `normalize_strategy_request()` 后的策略类型。

- [x] **Step C: 重新跑完整 API 回归并单独确认失败链路消失**

Run:

```bash
source "$HOME/.cargo/env" && cargo test -p api-server --test exchange_flow credential_updates_require_running_strategies_to_be_paused_first -- --nocapture
source "$HOME/.cargo/env" && cargo test -p api-server --test exchange_flow -- --nocapture
source "$HOME/.cargo/env" && cargo test -p api-server --tests
```

Expected: 全部 PASS。注意首次失败会 poison `exchange_flow` 的 env lock，导致后续 16 个测试级联失败；所以必须先单测首个失败，再跑整个 `exchange_flow`。

## 2026-04-24 再复核结论：workspace 回归已通过

Claude 对上一轮 API 兼容问题与后续 `trading-engine` execution effects 问题的修复均已通过复核：原始 3 条评审问题的定向验证 PASS，`execution_effects` 整组 PASS，`cargo test --workspace --tests` 与前端契约验证也已 PASS。

已确认通过:

- `source "$HOME/.cargo/env" && cargo test -p api-server --test strategy_flow ordinary_grid_normalization_keeps_anchor_first_after_exchange_rounding -- --nocapture`：PASS。
- `source "$HOME/.cargo/env" && cargo test -p api-server --test strategy_flow classic_mode_without_explicit_strategy_type_infers_bilateral -- --nocapture`：PASS。
- `source "$HOME/.cargo/env" && cargo test -p api-server --test strategy_flow strategy_creation_rejects_strategy_type_and_mode_mismatch -- --nocapture`：PASS。
- `source "$HOME/.cargo/env" && cargo test -p api-server --test exchange_flow credential_updates_require_running_strategies_to_be_paused_first -- --nocapture`：PASS。
- `source "$HOME/.cargo/env" && cargo test -p api-server --test exchange_flow -- --nocapture`：PASS，17 passed。
- `source "$HOME/.cargo/env" && cargo test -p api-server --tests`：PASS，需要允许本地端口/临时容器。
- `node --test tests/verification/strategy_surface_contract.test.mjs tests/verification/web_app_shell.test.mjs`：PASS。

复核结论:

- [x] **Step D: 修复 `trading-engine` 的 execution effects 通知回归/并发挂起**

2026-04-24 复核结果：Claude 已修复该问题。普通沙箱下该测试会因为本地端口绑定权限报 `Operation not permitted`，这不是业务失败；在允许本地端口/临时资源的环境中重跑后，`execution_effects` 整组稳定通过。

已验证语义:

- 配置 bot token 且 mock Telegram 成功时，会落 `channel == "telegram"`、`template_key == "GridFillExecuted"`、`status == "delivered"` 记录。
- Telegram 绑定存在但 bot token 缺失时，会保留 failed 通知记录。
- 整组 `execution_effects` 不再出现并发污染导致的断言失败或挂起。

- [x] **Step E: 重跑完整 workspace 回归**

2026-04-24 fresh verification:

```bash
source "$HOME/.cargo/env" && timeout 120s cargo test -p trading-engine --test execution_effects -- --nocapture
# PASS: 7 passed; 0 failed

source "$HOME/.cargo/env" && cargo test --workspace --tests
# PASS: workspace tests completed with exit 0

node --test tests/verification/strategy_surface_contract.test.mjs tests/verification/web_app_shell.test.mjs
# PASS: 2 passed; 0 failed
```

当前无新增待修复项。后续若在普通沙箱复现 `bind test server: Operation not permitted`，应切换到允许本地端口绑定的测试环境，不要把权限失败误判为业务回归。

