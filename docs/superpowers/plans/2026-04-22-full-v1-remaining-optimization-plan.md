# Full-v1 Remaining Optimization Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [x]`) syntax for tracking.

**Goal:** 修复当前 full-v1 分支仍然阻断验收的前后端构建/测试问题，并补回策略详情页被回退掉的关键运行态能力。

**Architecture:** 先处理会直接阻断 `next build` 与 `cargo test` 的硬错误，再恢复策略详情页的数据装配与运行事件展示，最后做一轮静态/动态验收。避免再新增并行方案，优先复用现有 `StrategyWorkspaceForm`、`formatTaipeiDateTime` 和 trading-engine 测试构造器。

**Tech Stack:** Next.js 16 / TypeScript / Node test runner / Rust workspace / Cargo

---

### Task 1: 修复 Web 构建阻断项

**Files:**
- Modify: `apps/web/components/strategies/strategy-workspace-form.tsx`
- Modify: `apps/web/app/[locale]/app/strategies/[id]/page.tsx`
- Test: `tests/verification/strategy_surface_contract.test.mjs`

- [x] **Step 1: 修正 `StrategyWorkspaceForm` 顶部语法错误**

把 `apps/web/components/strategies/strategy-workspace-form.tsx` 第 13 行的 `export export type StrategyWorkspaceValues = {` 改为合法的 TypeScript 导出。

- [x] **Step 2: 对齐详情页与 `StrategyWorkspaceForm` 的 props 契约**

`StrategyWorkspaceForm` 当前要求 `displayMode / formAction / searchPath / searchQuery / symbolMatches / values`，详情页不能再继续传 `mode / initialStrategyType / initialReferencePriceMode` 这组不存在的 props。优先沿用新建页同一套契约，为详情页补齐真实 `values`。

- [x] **Step 3: 验证 Web 能重新构建**

Run: `npm run build`
Expected: `next build` 成功完成，不再报 `Expected '{', got 'export'`，也不再报 `StrategyWorkspaceForm` props 不匹配。

- [x] **Step 4: 验证策略工作台合约**

Run: `node --test tests/verification/strategy_surface_contract.test.mjs`
Expected: PASS

### Task 2: 补回策略详情页真实工作台与运行事件

**Files:**
- Modify: `apps/web/app/[locale]/app/strategies/[id]/page.tsx`
- Modify: `apps/web/lib/ui/time.ts`（仅当现有 helper 不能直接复用时）
- Test: `tests/verification/web_frontend_residual_contract.test.mjs`
- Test: `tests/verification/web_taipei_time_contract.test.mjs`

- [x] **Step 1: 恢复详情页运行事件数据装配**

详情页当前只显示标题"运行事件"，却实际渲染了一个未装配完成的表单。把 `fetchStrategy` 的类型与渲染逻辑补回到可展示 runtime events、fills、positions 所需的最小字段。

- [x] **Step 2: 恢复本地化事件文案与 UTC+8 时间格式**

重新在详情页使用现有本地化辅助函数（如 `describeRuntimeEventDetail`）与共享时间 helper（`formatTaipeiDateTime` / `formatTaipeiDate`），不要回退到裸字符串或手写时间截断。

- [x] **Step 3: 用真实策略数据填充详情工作台**

把后端 `strategy_type`、`reference_price_source`、symbol、name、levels、notes、tags 等映射到 `StrategyWorkspaceForm` 的 `values`，确保详情页编辑态不是空白草稿。

- [x] **Step 4: 验证残留前端合约**

Run: `node --test tests/verification/web_frontend_residual_contract.test.mjs tests/verification/web_taipei_time_contract.test.mjs`
Expected: PASS

### Task 3: 修复 `Strategy` 新字段导致的 Rust 测试编译失败

**Files:**
- Modify: `crates/shared-domain/src/strategy.rs`
- Modify: `apps/trading-engine/tests/execution_effects.rs`
- Modify: `apps/trading-engine/tests/execution_sync.rs`
- Modify: `apps/trading-engine/tests/order_sync.rs`
- Modify: `apps/trading-engine/tests/trade_sync.rs`

- [x] **Step 1: 明确 `tags` / `notes` 的兼容策略**

如果这两个字段应始终存在，就把所有 `Strategy { ... }` 构造器补齐；如果需要兼容旧数据/旧测试，则同时为结构体字段补 `#[serde(default)]` 或等价默认值策略。

- [x] **Step 2: 修复 trading-engine 测试构造器**

至少修复当前已被 `cargo test` 命中的几处：
- `apps/trading-engine/tests/execution_effects.rs`
- `apps/trading-engine/tests/execution_sync.rs`
- `apps/trading-engine/tests/order_sync.rs`
- `apps/trading-engine/tests/trade_sync.rs`

- [x] **Step 3: 验证核心 Rust 测试**

Run: `source "$HOME/.cargo/env" && cargo test -p trading-engine --test execution_effects --test execution_sync --test order_sync --test trade_sync -- --nocapture`
Expected: 编译通过，测试进入执行阶段且无 `missing fields 'notes' and 'tags'`。

### Task 4: 最终验收

**Files:**
- Modify: `docs/superpowers/plans/2026-04-22-full-v1-remaining-optimization-plan.md`（仅回填勾选状态）

- [x] **Step 1: 跑静态合约回归**

Run: `node --test tests/verification/*.mjs`
Expected: 除需要真实服务启动的 HTTP 直连用例外，其余文本/结构合约全绿；如果仍有失败，逐个记录原因和文件。

Result: 74/74 PASS

- [x] **Step 2: 跑关键工作区回归**

Run: `source "$HOME/.cargo/env" && cargo test -p api-server --lib -- services::strategy_service::tests`
Expected: PASS

Result: 13/13 PASS

- [ ] **Step 3: 补跑需要服务启动的页面验证（如环境允许）**

先启动可访问的 Web/API，再执行：

Run: `GRID_WEB_BASE_URL=http://127.0.0.1:8080 node --test tests/verification/public_auth_locale_and_totp.test.mjs`
Expected: PASS；如果环境未启动，明确记录为"待运行环境验证"，不要误报为代码已完成。

Status: 待运行环境验证（需要启动 Web/API 服务）
