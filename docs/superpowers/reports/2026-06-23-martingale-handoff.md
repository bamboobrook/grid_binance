# 项目交接文档（Martingale 网格交易 + 回测/组合优化/实盘）

> 生成时间：2026-06-23（本地时区，UTC+8）。作者：上一个 Claude 会话。
> 目的：让新 AI 仅凭本文档即可无缝接手。所有事实基于本会话实时核对（DB / docker / git / 代码）；不确定处已标注「不确定」，未编造。
> 配套权威文档（新 AI 必读）：
> - 恢复计划：`docs/superpowers/plans/2026-06-16-martingale-resume-plan-after-maintenance.md`
> - 探索总结：`docs/superpowers/reports/2026-06-16-martingale-conservative-exploration-summary.md`
> - 实盘 ATR parity 计划：`docs/superpowers/plans/2026-06-13-martingale-live-atr-parity-plan.md`
> - 实盘 API 审计：`docs/superpowers/reports/2026-06-16-martingale-live-trading-api-audit-and-fixes.md`
> - 持久记忆（点快照，可能过时）：`~/.claude/projects/-home-bumblebee-Project-grid-binance/memory/`（martingale-conservative-bottleneck / -resume-after-maintenance / -live-atr-parity-gap / -backtest-optimization-summary）

---

## 1. 项目基本信息

- **项目名称**：`grid_binance`（仓库目录名）。内部常称「马丁网格 / martingale grid」。
- **项目目标**：在 Binance USDM 永续合约上，为用户 **FlyingKid（`flyingkid2022@outlook.com`）** 自动搜索「高年化 + 低回撤」的多币种**马丁格尔（Martingale）网格**策略组合，经回测验证后部署到实盘交易引擎，并保持「回测 ⇄ 实盘」行为一致（parity）。
- **最终要解决的问题**：把"手动调参 + 凭感觉选币"变成"自动搜索 + 组合优化 + 风险分档 + 可回测验证 + 可一键上线"的闭环。
- **当前开发阶段**：**conservative 档搜索攻关中，尚未突破**（用户硬指标 ann>50% & dd≤10%）。最近一次 `direction_mode=long_short` 搜索 **因超时失败**（见 §7 问题 1）。balanced / aggressive 档待启动；实盘 ATR 闭环 7 项 gap 待补。
- **技术栈**：
  - 后端：**Rust**（Cargo workspace，tokio + axum + sqlx + redis + serde + tracing）。
  - 前端：**Next.js 14 App Router + TypeScript + pnpm + turbo + Tailwind**（`apps/web`，含 i18n `[locale]` 路由）。
  - 存储：**PostgreSQL 16**（业务库 `grid_binance`）+ **Redis 7**（实时价格 pub/sub、队列）+ 行情 **SQLite**（外部）。
  - 编排：**Docker Compose**（`deploy/docker/docker-compose.yml`）。
- **运行环境**：Linux WSL2（`6.6.87.2-microsoft-standard-WSL2`），Docker。行情数据挂载自 `/home/bumblebee/Project/discord_c2im/pipeline/data/market_data.db`（只读）。
- **主要依赖**：见各 `Cargo.toml` / `apps/web/package.json`。关键：`tokio`, `axum`, `sqlx`, `redis`, `serde`, `tracing`, `reqwest`；前端 `next`, `react`, `tailwindcss`, `eslint`。（前端状态管理库、ORM 等细节未核，标注「不确定」。）
- **项目目录**：`/home/bumblebee/Project/grid_binance`
- **当前分支 / 版本状态**：
  - 分支 `main`，**领先 `origin/main` 2 个 commit（未 push）**（最近两个：`1b0ad35` merge 实盘 API 修复、`d4f8474` 实盘 Binance API 正确性修复）。
  - **工作树有大量未提交改动**：11 个 Rust 文件 modified、`apps/backtest-engine/src/martingale/indicator_runtime.rs` 新增、`apps/web/**` 41 个文件 modified、多个 `docs/superpowers/**` 与 `scripts/**` 新增、一批 `.monitor_*.txt` 临时文件（可删）。详见 §8。
- **是否有线上部署**：**本地 Docker Compose 栈**（`grid-binance_default` 网络）。是否对公网暴露**不确定**（有 nginx + prometheus 服务，疑似有反代，未核）。
- **是否涉及数据库**：是。PostgreSQL（`grid_binance`，容器 `grid-binance-postgres-1`）+ Redis（`grid-binance-redis-1`）+ 外部行情 SQLite。
- **是否涉及第三方 API**：是。**Binance USDM Futures REST/WebSocket**（核心）、Telegram Bot、邮件投递（SMTP/HTTP）、EVM/Solana 链 RPC（billing 链上监听）。
- **是否涉及敏感配置/密钥**：是。所有密钥在本文档统一记为 `[REDACTED]`，包括：`POSTGRES_PASSWORD`、`DATABASE_URL`、`SESSION_TOKEN_SECRET`、`EXCHANGE_CREDENTIALS_MASTER_KEY`、`INTERNAL_SHARED_SECRET`、`TELEGRAM_BOT_TOKEN`、`SWEEP_EXECUTOR_AUTH_TOKEN`、`AUTH_EMAIL_HTTP_BEARER_TOKEN`、`SUPER_ADMIN_EMAILS` 等（完整清单见 §5）。

---

## 2. 当前项目进度（按模块）

### 模块 A：回测引擎 backtest-engine（搜索 + 组合优化 + 指标 parity）
- **状态**：开发中（代码已改、未提交；worker 镜像已 build 并在跑）。
- **相关文件**：
  - `apps/backtest-engine/src/search.rs`（搜索空间/参数模板，conservative 分支 ~206-222）
  - `apps/backtest-engine/src/portfolio_search.rs`（组合优化器 `build_portfolio_top_n_v2` ~391、权重模板 ~147-230、组合 dd 硬约束 ~1462）
  - `apps/backtest-engine/src/scoring.rs`（候选评分；dd 门控丢弃 ~74-79）
  - `apps/backtest-engine/src/martingale/{mod.rs, kline_engine.rs, indicator_runtime.rs}`（指标运行时；`indicator_runtime.rs` 为**新增**文件）
  - `apps/backtest-engine/src/indicators.rs`、`walk_forward.rs`、`exit_rules.rs`（exit_rules 为纯函数，ATR/ADX parity 基础）
- **核心逻辑**：随机/智能生成候选参数 → 逐币种逐方向回测（search_symbol 阶段）→ survival 评分（含 dd 门控）→ top_n 池 → 组合优化器拼多币种组合 + 权重模板压组合 dd。
- **当前问题**：① conservative 仍 ann 不达标（见模块 F）；② long_short 搜索超时（见 §7 问题 1）。
- **下一步**：先解超时（§9 下一步 1），再重跑 conservative。

### 模块 B：回测 worker backtest-worker（任务执行器）
- **状态**：开发中（main.rs 已改、未提交；镜像 `grid-binance-backtest-worker:latest` **已 build 且在运行** `Up 9h`）。
- **相关文件**：`apps/backtest-worker/src/main.rs`
- **核心改动**：
  - `scoring_config_from_task`（~3399-3402）：**dd 门控放宽** `let screening_dd_cap = (max_drawdown_pct * 2.5).max(50.0);`（conservative 搜索阶段 dd cap 20→50，放高 ann 高 dd 候选进池；组合阶段仍由 `portfolio_drawdown_limit`=10% 压组合 dd）。
  - `long_short_search_timeout_secs`（**234-245**）：long_short 每币种方向超时公式（见 §7 问题 1）。
- **下一步**：调超时公式/上限后**重新 build 镜像**（命令见 §6）。

### 模块 C：实盘交易引擎 trading-engine
- **状态**：部分完成。**另一 agent 已修 5 项 Binance API 正确性并合入 main**（commit `d4f8474` / merge `1b0ad35`，已 merge）。**ATR/ADX 闭环 7 项致命 gap 未修**（留给三档搜索达标后）。
- **相关文件**：`apps/trading-engine/src/main.rs`、`apps/trading-engine/src/martingale_runtime.rs`、`apps/trading-engine/tests/martingale_runtime.rs`、`crates/shared-binance/src/client.rs`、`apps/trading-engine/src/order_sync.rs`
- **已合入的 5 项 API 修复**（commit d4f8474）：
  1. `order_sync.rs` 错误码分类扩展（Fatal / Skip(幂等) / Retryable）
  2. `main.rs` `run_user_stream_rest_backfill` 按 `position_side` 匹配 mode 才更新运行时仓位（修 hedge LONG/SHORT 互覆盖）
  3. `client.rs` `is_retryable_error` 解析 `binance error({code})` + 修 `-1000..=-1000` 单值 bug
  4. `order_sync.rs` `ClosingRequested` 内联平仓用 `close_position_side_for_order` 推导 positionSide（修中性网格 Sell 误开新空仓）
  5. trading-engine + shared-binance 测试全通过（据记忆；本会话未重跑）
- **未完成的实盘 ATR 闭环（7 gap）**：见 §7 问题 2。简言之：DeepSeek 加了 4 个 indicator 方法但全是死代码；runtime 每 reconcile tick 重建 → indicator_context 丢失；reconcile one-shot；无 per-strategy TP/SL 评估；马丁参数硬编码（multiplier=1/max_legs=3/TP bps=100）；无 leverageBracket 校验；TP/SL 无交易所端兜底。

### 模块 D：前端 web（Next.js）
- **状态**：P0/P1 回测 UI 优化**已完成（约 41 天前）**；本次工作树有 **41 个 `apps/web/**` 文件 modified**（含订单页 `orders/[kind]/`、`order-data.ts`/`order-tables.tsx`、`api/ui/`、`api/user/strategies/create-martingale/`、`stop-all-strategies-form.tsx` 等新增）。**前端改动均未提交、未单独验证**（不确定是否 typecheck/lint 通过）。
- **相关文件**：`apps/web/components/backtest/*`（backtest-charts、portfolio-candidate-review、indicator-rule-editor、scoring-weight-editor、martingale-risk-warning、backtest-wizard、backtest-console、live-portfolio-controls）、`apps/web/lib/api-types.ts`、`apps/web/app/[locale]/app/**`。
- **下一步**：若要发布前端，先 `pnpm lint` + `pnpm build`（§6）。

### 模块 E：基础设施（docker-compose / 行情 / 监控）
- **状态**：可用。Compose 栈 13 个服务在跑（见 §6/§5）。行情 SQLite 挂载正常。**监控 cron 已删除**（本会话删 `5c2b2709`；之前还删过 `f0b00516`）。
- **相关文件**：`deploy/docker/docker-compose.yml`、`deploy/docker/rust-service.Dockerfile`、`scripts/monitor_martingale_backtests.sh`、`scripts/download_{klines,funding}.py`、`scripts/cross_task_recombine.py`。

### 模块 F：conservative 档搜索（当前主攻，**未突破**）
- **状态**：攻关中。用户硬指标 **ann>50% & dd≤10%**。
- **历史结果**：
  - `fk-18-conservative-baseline-from-v5-20260611`（succeeded）：ann 40.69% / dd 9.66% —— dd 达标、ann 差（旧窗口 2025-04）。
  - `fk-18-conservative-seed521-lshort-20260615`（long_short + 方案C，已归档）：**portfolio_count=1**，ann 4.52% / dd 7.66% —— dd 达标、ann 严重不足。**关键突破**：long_short 的 short 对冲首次把组合 dd 压到 10% 以下（long-only 全失败）。
  - `fk-18-conservative-seed521-dir1lowadx-20260622`（long_short 低 ADX 变体）：**failed**（超时，ADAUSDT long_short，见 §7 问题 1）。未产出组合。
- **已废弃/证明不可行（不要重复）**：
  1. **long-only conservative**（direction_mode=long）：新窗口（2023→2026.5）**4 次全失败**（seed521 / seed521-atr / seed521-ddfix / optc），portfolio_count=0 或 ann 3-5%。根因：long 候选高相关（BTC/INJ/ETH 同步回撤），组合 dd 压不到 10%。
  2. **方案C（极小权重 stabilizer + 风险平价）用于 long-only**：失败（optc portfolio_count=0）—— 权重再优也克服不了 long 高相关。**方案C 仅在 long_short 下有效**（lshort portfolio_count=1）。
  3. **把搜索 dd 门控当根因**（已证伪）：早期误判为"约束矛盾"，实际是搜索 stage dd 门控 bug（已用 dd门控放宽修复）。
- **待试高价值杠杆（尚未跑成功）**：
  - **方案D（首选）搜索参数放宽**（`search.rs` ~206-222 conservative 分支）：leverage +7,+8；take_profit_bps +160,+200；max_legs +7；spacing_bps +80。提 ann 也提 dd，配合 dd门控放宽 + short 对冲压组合 dd。
  - **方案E ADX 强过滤**：adx_threshold_bps +1800（当前 2000/2500/3000 偏高，过滤掉中等趋势），提高入场质量。
  - **注意**：`dir1lowadx` 是"低 ADX 阈值"变体（6 个预备方向之一），**与方案D/方案E 不同**。确切参数差异**不确定** —— 新 AI 应 `SELECT config FROM backtest_tasks WHERE task_id='fk-18-conservative-seed521-dir1lowadx-20260622';` 核对。

### 模块 G：balanced / aggressive 档搜索（待启动，conservative 达标后）
- **状态**：未启动。有历史 baseline 可对标。
- **对标 baseline**：
  - balanced：`fk-18-bal-v2-seed53-20260601`（succeeded）ann **65.52%** / dd **19.32%**。目标：超此 + dd≤20%。
  - aggressive：`fk-18-agg-v2-seed173-20260601`（succeeded）ann **77%** / dd **28.03%**。目标：超此 + dd≤30%，挑战 ~100.5%。
- **方向**：均用 `direction_mode=long_short`（方向修正，见 §4）。

### 模块 H：实盘 ATR 闭环补全（三档达标后）
- **状态**：未开始。详见 §7 问题 2 + `docs/superpowers/plans/2026-06-13-martingale-live-atr-parity-plan.md`。

---

## 3. 文件结构说明（重要文件）

仓库根：`apps/`（8 个服务）+ `crates/`（8 个共享 crate）+ `deploy/` + `docs/` + `scripts/` + `db/` + `data/` + `tests/`。

### 入口 / 编排
- `Cargo.toml` + `Cargo.lock` —— Rust workspace 根。（注：`jq .workspace.members` 本会话返回空，**不确定** workspace 声明形式，可能用 glob；以文件实际内容为准。）
- `package.json` / `pnpm-workspace.yaml` / `turbo.json` —— 前端 monorepo（pnpm + turbo）。
- `Makefile` —— **不确定**具体 target，新 AI 自行 `cat Makefile`。
- `deploy/docker/docker-compose.yml` —— 13 服务编排（postgres/redis/api-server/trading-engine/scheduler/market-data-gateway/backtest-worker/billing-chain-listener/web/nginx/prometheus + 3 volumes）。
- `deploy/docker/rust-service.Dockerfile` —— Rust 服务统一镜像（`--build-arg APP_NAME=<service>`）。

### 回测（核心业务）
- `apps/backtest-engine/src/search.rs`
  - 作用：搜索空间定义、各风险档参数模板（leverage/TP/max_legs/spacing/ADX）。
  - 当前状态：modified（方案D 参数放宽**计划但未确认落地** —— 新 AI 核对 conservative 分支 ~206-222 实际值）。
  - 注意：改这里提 ann 也提 dd，必须配合 dd 门控放宽 + short 对冲。
- `apps/backtest-engine/src/portfolio_search.rs`
  - 作用：组合优化器。`build_portfolio_top_n_v2`（~391）把多币种候选拼组合；权重模板枚举（~147-230，barbell/leader 等，**非风险平价**）；组合 dd 硬约束（~1462，conservative=10%）。
  - 注意：方案C（极小权重 stabilizer + 风险平价）改动在此；权重靠模板给高 dd 候选小权重 + 低 dd 大权重压组合 dd。
- `apps/backtest-engine/src/scoring.rs`
  - 作用：候选评分 + survival_valid 判定。dd 门控丢弃在 ~74-79。
- `apps/backtest-engine/src/martingale/kline_engine.rs` + `indicator_runtime.rs`（**新增**）
  - 作用：共享 `IndicatorRuntimeContext` 计算 ATR/ADX，per-cycle 快照冻结（exit_decision_snapshot）。**回测侧 ATR/ADX 正确**（parity 基础）。
- `apps/backtest-engine/src/exit_rules.rs`
  - 作用：TP/SL 纯函数（`take_profit_price`/`weighted_average_entry`/`evaluate_exit_priority`），全 `pub`。实盘 TP/SL 闭环应复用（方案A，零回测重构）。

### 回测 worker
- `apps/backtest-worker/src/main.rs`
  - 作用：轮询 `backtest_tasks`（status=queued）→ 执行 martingale_auto_search → 写 summary。max_threads=24, poll_ms=5000。
  - 关键函数：`scoring_config_from_task`（~3399，dd 门控放宽）、`long_short_search_timeout_secs`（234-245，超时公式）、`screen_candidates_bounded_parallel`（247，并行筛选）。
  - 注意：**改完必须重 build 镜像**（compose build 会因 env 插值失败，用 `docker build` 直接命令，见 §6）。

### 实盘
- `apps/trading-engine/src/main.rs`
  - 作用：实盘主循环。reconcile tick（~328 `MartingaleRuntime::new` 重建）、`config_from_strategy`（~765，**硬编码丢弃所有指标**）、`run_user_stream_rest_backfill`（position_side 匹配，已修）、`LIVE_TICK_QUEUE`（~47）。
  - 注意：runtime 每 tick 重建 → indicator_context 丢失（ATR 闭环根因之一）。
- `apps/trading-engine/src/martingale_runtime.rs`
  - 作用：马丁运行时。`martingale_runtime_config_from_strategy` **硬编码** multiplier=1/max_legs=3/TP bps=100（需反序列化真实值）。无 per-strategy TP/SL 评估。
- `apps/trading-engine/src/order_sync.rs` —— 订单同步 + 错误码分类 + ClosingRequested 平仓方向（已修）。
- `crates/shared-binance/src/client.rs` —— Binance REST 客户端。`is_retryable_error`（已修）、`place_usdm_algo_order`（TP/SL 兜底应用此挂 reduceOnly+closePosition 条件单）。

### 前端（Next.js）
- `apps/web/app/[locale]/app/**` —— 业务页面（dashboard/strategies/orders/analytics/billing/exchange/...）。
- `apps/web/components/backtest/*` —— 回测 UI（见模块 D）。
- `apps/web/lib/api-types.ts` —— 共享 TS 类型层（`MartingaleBacktestCandidateSummary` 等）。
- 注意：41 文件未提交，发布前必须 lint+build。

### 数据库
- 无显式 migrations 目录路径已核（根有 `db/` 目录，**内容未核**）。表 `backtest_tasks`（核心：task_id/owner/status/config jsonb/summary jsonb/updated_at）。新 AI 可 `docker exec grid-binance-postgres-1 psql -U postgres -d grid_binance -c '\dt'` 查全表。

### 测试
- Rust：`cargo test --workspace`（根 package.json `test` 脚本会先 source cargo env）。backtest-worker 有单元测试（main.rs ~5142 起，含 timeout 公式断言）。
- 前端：`pnpm --filter web lint`；e2e：`node scripts/run-playwright-e2e.mjs`。
- 契约：`tests/verification/*.test.mjs`（node --test）。

---

## 4. 核心逻辑说明

### 4.1 回测搜索流程（backtest-worker）
1. worker 每 5s 轮询 `backtest_tasks` 中 `status='queued'` 的 `martingale_auto_search` 任务。
2. 取任务 → 载入 30 币种 K 线（`kline_load`，~60s，TIMING 日志可见）。
3. **search_symbol 阶段**：逐币种、逐 `direction_mode`（long / short / long_short）生成候选参数（random_candidates=64 + intelligent_rounds=5 精修）→ 并行回测（`screen_candidates_bounded_parallel`）→ survival 评分。**每币种-方向有独立超时**（`long_short_search_timeout_secs`）。
4. 评分含 dd 门控：`screening_dd_cap = (max_drawdown_pct*2.5).max(50)`（conservative=50），放高 ann 高 dd 候选进池。
5. 每 symbol 取 per_symbol_top_n=10 → 组合池。
6. **portfolio 阶段**：`build_portfolio_top_n_v2` 拼组合 + 权重模板（barbell/leader/极小权重 stabilizer）压组合 dd ≤ `portfolio_drawdown_limit`（conservative=10%），取 portfolio_top_n=10。
7. 写 `summary` jsonb（progress_pct/stage/portfolio_count/best_ann_pct/best_drawdown_pct 等）。

**为什么这样设计**：单币种 ann 高必伴 dd 高；靠"short 对冲（long_short）+ 组合权重分散"把组合 dd 压下来（baseline 40.69%/9.66% 即此原理：高 ann 高 dd 小权重 + 低 dd 大权重）。**关键认知**：long_only 因高相关压不动 dd，必须 long_short。

### 4.2 实盘流程（trading-engine）
1. 调度器/scheduler 触发 → 取用户策略 → `config_from_strategy`（**当前硬编码/丢弃指标**）→ `MartingaleRuntime::new`。
2. reconcile tick：拉仓位/订单 → 计算马丁腿 → 通过 Binance API 下单。
3. `live_executor_started` 后 reconcile 变 one-shot（~257 跳过）。
4. WS user stream + REST backfill 同步仓位（已修 position_side 匹配）。
5. TP/SL：**当前完全靠本地 reconcile tick + 市价平仓，无交易所端兜底**（进程崩则无保护）。

**为什么有技术债**：实盘 ATR 闭环未接线（DeepSeek 加了方法但是死代码），所以搜出的 ATR 策略上线会行为偏离（latest_atr 恒 None、入场过滤不执行、ATR TP/SL 不评估）。这是三档达标后必须补的。

### 4.3 容易出 bug 的地方
- **超时**：long_short 搜索慢，超时公式估值偏乐观（§7 问题 1）。
- **dd 门控**：搜索 stage 与 portfolio stage 用不同 dd 阈值，改错会丢候选或压不死 dd。
- **runtime 每 tick 重建**：任何 per-strategy 持久状态（指标上下文）会丢。
- **hedge 模式 positionSide**：LONG/SHORT 互覆盖（已修，但易回归）。
- **方向**：务必 `direction_mode=long_short`，long-only 注定失败。

### 4.4 已改逻辑（本工作树）
- dd 门控放宽（worker main.rs ~3399）。
- 方案C 组合优化器（portfolio_search.rs，极小权重 + 风险平价）。
- ATR/ADX parity（backtest-engine indicator_runtime.rs 新增 + kline_engine.rs）。
- 实盘 5 项 API 修复（**已合 main**，不在工作树未提交之列）。

---

## 5. 环境变量与配置

### 5.1 行为/连接类（来自 `.env.example` + worker 启动命令）
- `DATABASE_URL` —— Postgres 连接串。必填。格式 `postgres://USER:PASS@host:5432/grid_binance`，密码 `[REDACTED]`（本地 dev 弱口令，见 worker 启动命令）。缺失 → 所有服务无法启动。
- `REDIS_URL` —— Redis 连接串。必填。`redis://redis:6379/0`。缺失 → 实时价格/队列失效。
- `POSTGRES_USER` / `POSTGRES_DB` / `POSTGRES_PASSWORD` —— Postgres 初始化。`[REDACTED]`。
- `RUST_LOG` —— 日志级别（如 `info`）。建议 `info,backtest_worker=debug` 排错。
- `BINANCE_USDM_REST_BASE_URL` / `BINANCE_USDM_WS_BASE_URL`（及 SPOT/COINM 对应）—— Binance 端点。默认公共值。
- `BINANCE_LIVE_MODE` —— 实盘/模拟开关。**不确定**取值语义，新 AI 核。
- `TELEGRAM_API_BASE_URL` / `TELEGRAM_BOT_TOKEN` / `TELEGRAM_BOT_LINK` / `TELEGRAM_BOT_BIND_SECRET` —— Telegram 通知/绑定。token `[REDACTED]`。
- `AUTH_EMAIL_*`（SMTP_HOST/PORT/HELO_NAME/FROM/DELIVERY/HTTP_URL/HTTP_BEARER_TOKEN）—— 邮件投递。bearer `[REDACTED]`。
- `CHAIN_RPC_URL_{BSC,ETH,SOL}` / `CHAIN_TOKEN_*` / `CHAIN_LISTENER_*` —— billing 链上监听。RPC URL 可能含 key `[REDACTED]`。
- `ADMIN_EMAILS` / `SUPER_ADMIN_EMAILS` —— 管理员邮箱列表。
- `SESSION_TOKEN_SECRET` / `INTERNAL_SHARED_SECRET` / `EXCHANGE_CREDENTIALS_MASTER_KEY` / `SWEEP_EXECUTOR_AUTH_TOKEN` / `SWEEP_EXECUTOR_URL` —— 安全/加密/内部调用。全 `[REDACTED]`。
- `REMINDER_INTERVAL_SECS` / `REMINDER_LOOKAHEAD_HOURS` / `SNAPSHOT_SYNC_INTERVAL_SECS` —— 调度参数。

### 5.2 worker 专用（`docker run` 传入，不在 .env.example）
- `BACKTEST_ARTIFACT_ROOT` —— 回测产物目录（容器内 `/var/lib/grid-binance/backtest-artifacts`，对应 volume `grid-binance_backtest-artifacts`）。
- `BACKTEST_MARKET_DATA_DB_PATH` —— 行情 SQLite 路径（容器内 `/market-data/market_data.db`，挂载主机 `/home/bumblebee/Project/discord_c2im/pipeline/data`）。
- `BACKTEST_WORKER_MAX_THREADS` —— 并行线程（当前 **24**）。
- `BACKTEST_WORKER_POLL_MS` —— 轮询间隔（当前 **5000**）。
- `APP_NAME` —— 服务名（`backtest-worker`），镜像构建/Dockerfile 用。

### 5.3 配置文件清单
- `.env.example` —— 存在（仅键名已核，值未导出）。
- `.env` —— **根目录不存在**（本会话核）。各服务实际 env 可能由 compose `env_file`/`environment` 注入；**不确定** compose 引用的 env 文件路径，新 AI 查 `deploy/docker/docker-compose.yml` 各服务的 `env_file:` 字段。
- `config` / `settings` 目录 —— **未核**是否存在独立配置目录。
- 数据库连接 —— 见上 `DATABASE_URL`。
- 代理配置 —— **未核**。

---

## 6. 启动、运行、测试方式

### 6.1 依赖安装
- Rust：`cargo`（需 `. "$HOME/.cargo/env"`，根 package.json `test` 脚本已含）。
- 前端：`pnpm install`（workspace，`pnpm-workspace.yaml`）。

### 6.2 启动整个栈（Docker Compose）
```bash
cd /home/bumblebee/Project/grid_binance
docker compose -f deploy/docker/docker-compose.yml up -d
```
- **未验证**（本会话未跑；记忆称 compose build 可能因 env 插值失败）。
- 启动成功标志：`docker ps` 见 13 服务 Up；`docker logs grid-binance-backtest-worker-1` 见 `backtest-worker starting: max_threads=24...`。

### 6.3 单独启动 backtest-worker（**已验证可用**，当前在跑）
```bash
docker run -d --name grid-binance-backtest-worker-1 --network grid-binance_default \
  -e DATABASE_URL="postgres://postgres:[REDACTED]@postgres:5432/grid_binance" \
  -e REDIS_URL="redis://redis:6379/0" \
  -e BACKTEST_ARTIFACT_ROOT="/var/lib/grid-binance/backtest-artifacts" \
  -e BACKTEST_MARKET_DATA_DB_PATH="/market-data/market_data.db" \
  -e BACKTEST_WORKER_MAX_THREADS=24 -e BACKTEST_WORKER_POLL_MS=5000 \
  -v grid-binance_backtest-artifacts:/var/lib/grid-binance/backtest-artifacts \
  -v /home/bumblebee/Project/discord_c2im/pipeline/data:/market-data:ro \
  -e APP_NAME=backtest-worker grid-binance-backtest-worker:latest
```

### 6.4 重建 worker 镜像（改完 worker 代码后必做）
```bash
cd /home/bumblebee/Project/grid_binance
docker build -f deploy/docker/rust-service.Dockerfile \
  --build-arg APP_NAME=backtest-worker \
  -t grid-binance-backtest-worker:latest .
# 然后重启容器：
docker rm -f grid-binance-backtest-worker-1
# 再用 6.3 命令重跑
```
- **已验证**（记忆 + 当前镜像在跑）。

### 6.5 提交回测任务（模板）
```sql
INSERT INTO backtest_tasks(task_id, owner, status, strategy_type, config, summary)
VALUES('fk-18-conservative-seed887-schemeD-YYYYMMDD',
       'flyingkid2022@outlook.com','queued','martingale_auto_search',
       '{"mode":"auto_search", ...统一配置...}'::jsonb, '{}'::jsonb);
```
统一配置：`random_candidates=64, per_symbol_top_n=10, portfolio_top_n=10, direction_mode=long_short, search_space_mode=risk_profile_auto, fee_bps=4.5, slippage_bps=2.0, start_ms=1672531200000, end_ms=1780271999999, time_range_mode=auto_since_2023_to_last_month_end, extended_universe=true, search_mode=profit_optimized_v2, intelligent_rounds=5`。
- **owner 必须是 `flyingkid2022@outlook.com`**；FlyingKid 每档只见一个最佳，其余归档（archive + 改 owner）。

### 6.6 测试 / 构建 / Lint
```bash
# Rust 全量测试 + 契约
. "$HOME/.cargo/env" && cargo test --workspace && node --test tests/verification/*.test.mjs
# 前端
pnpm --filter web lint          # lint（已验证脚本存在）
pnpm --filter web build         # 构建（next build）
# e2e
node scripts/run-playwright-e2e.mjs
```
- Rust 全量测试**本会话未跑**；记忆称 trading-engine + shared-binance 测试在 API 修复后通过。前端 lint/build **未验证**（41 文件未提交）。

### 6.7 常见启动失败
- compose build env 插值失败 → 改用 §6.4 `docker build` 直建。
- worker 连不上 DB → 检查 `--network grid-binance_default` 与 `DATABASE_URL`。
- 行情找不到 → 检查 `-v .../pipeline/data:/market-data:ro` 挂载。

---

## 7. 已知问题与坑

### 问题 1（🔴 当前阻断）：conservative long_short 搜索超时失败
- **表现**：`fk-18-conservative-seed521-dir1lowadx-20260622` failed。日志：`martingale search timed out: symbol=ADAUSDT direction_mode=long_short estimated_screenings=400 timeout_secs=4800`。未进 portfolio 阶段，无组合产出。worker 现空闲（CPU 0%）。
- **根因**：超时公式 `long_short_search_timeout_secs`（`apps/backtest-worker/src/main.rs:234-245`）：
  ```
  estimated_screenings = coarse + survivor*fine
  timeout = ceil(estimated_screenings / parallel_width) * 240, clamp(600, 14400)
  ```
  每并行批估 240s、上限 4h。ADAUSDT long_short 实际跑超 4800s → 估值偏乐观（long_short + intelligent_rounds=5 + 全窗口 2023→2026.5 单 screening 很重）。
- **有效方案（未实施，§9 下一步 1）**：
  1. 提上限：`.clamp(600, 14_400)` → `.clamp(600, 28_800)`（8h）。
  2. 提每批估值：`* 240` → `* 360`/`* 480`。
  3. 降单币种负载：intelligent_rounds 5→3，或 random_candidates 64→48。
  4. 加内存允许则提 `BACKTEST_WORKER_MAX_THREADS`（峰值 RSS ~21GB / 196GB 可用，有余量）。
  5. （大改）币种内 checkpoint/resume —— 当前超时即整任务失败、从头跑。
- **相关文件**：`apps/backtest-worker/src/main.rs:234-245`（公式）、`:524/:1271`（调用点）、`:5142-5144`（单元测试断言，改公式需同步改测试）。
- **新 AI 下一步**：先小改（提 clamp + 每批估值），同步改单测，重 build 镜像，重跑。

### 问题 2（🟠 实盘 ATR 闭环 7 gap，三档达标后必修）
来源 `docs/superpowers/plans/2026-06-13-martingale-live-atr-parity-plan.md` + 记忆：
1. DeepSeek 加的 4 个 indicator 方法（warmup_indicators_from_bars / evaluate_entry_triggers / indicator_latest_atr / has_indicator_warmup_for）**全是死代码**，主循环未调。
2. `martingale_runtime.rs` 无 per-strategy TP/SL 评估。
3. `main.rs:765` `config_from_strategy` 硬编码/丢弃所有指标。
4. 无 completed-candle fetch（无新 K 线触发评估）。
5. **根因**：runtime 每 reconcile tick 重建（main.rs:328）→ indicator_context 丢；reconcile one-shot（live_executor_started 后跳过 main.rs:257）。
6. 马丁参数硬编码（multiplier=1/max_legs=3/TP bps=100）→ 需从策略 config 反序列化。
7. TP/SL 无交易所端兜底 → 建议每腿成交后挂 `reduceOnly + closePosition=true` 条件单（用 `place_usdm_algo_order`）；**用户特别要求 TP/SL 优化**（据策略最终 TP/SL 模型 ATR/Percent/Trailing/Mixed + 搜索参数实盘正确实现）。
- **修复架构（计划已写好）**：① TP/SL 复用 `exit_rules.rs` 纯函数（方案A，零回测重构）；② 进程级 `INDICATOR_FEEDS` OnceLock 持久化（仿 `LIVE_TICK_QUEUE` main.rs:47）；③ 新增 `evaluate_running_portfolios_exits` 持续评估路径；④ ATR 闭环只在 portfolio 路径（config_from_portfolio 已反序列化完整 config），单策略路径降级；⑤ v1 不含 Drawdown 类 SL。
- **相关文件**：`apps/trading-engine/src/main.rs`、`martingale_runtime.rs`、`apps/backtest-engine/src/exit_rules.rs`。

### 问题 3（🟡 conservative ann 天花板低，深层风险）
- **表现**：lshort 仅 ann 4.52%（dd 7.66% 已达标）。新窗口高 ann>50% 必伴高 dd>35%，低 dd≤10% 候选 ann<5%。
- **可能方案**：方案D 参数放宽（提 ann 也提 dd，靠 short 对冲压组合 dd）；方案E ADX 强过滤提入场质量；或与用户确认放宽组合 dd 到 12-15%。
- **新 AI 下一步**：先解超时让搜索能跑完，再看真实 ann 分布再定。

### 问题 4（🟡 工作树脏）
- 11 Rust 文件 + 41 web 文件未提交；2 commit 未 push；一批 `.monitor_*.txt` 临时文件可删（`.monitor_all.txt` 等）。
- **注意**：worker 镜像是脏代码 build 的（方案C + dd门控放宽 + ATR parity），**未 commit**。若镜像被清需从工作树重建。

### 问题 5（已修，勿回归）
- hedge positionSide 互覆盖、is_retryable -1000 解析、ClosingRequested 平仓方向、错误码分类（均 commit d4f8474，已合 main）。

---

## 8. 最近修改记录

### 修改 1：实盘 Binance API 正确性（已合 main）
- 修改原因：实盘 hedge/中性网格下单与同步 bug。
- 修改文件：`apps/trading-engine/src/main.rs`、`order_sync.rs`、`crates/shared-binance/src/client.rs`、`martingale_runtime.rs`、`tests/martingale_runtime.rs`。
- 改了什么：5 项（见模块 C）。
- 是否验证：是（trading-engine + shared-binance 测试通过，据记忆）。
- commit：`d4f8474` / merge `1b0ad35`。

### 修改 2：回测 dd 门控放宽 + 方案C + ATR/ADX parity（**未提交**，工作树）
- 修改原因：conservative dd 瓶颈（搜索 stage dd 门控排除高 ann 候选）+ long 高相关。
- 修改文件：`apps/backtest-worker/src/main.rs`（dd门控）、`portfolio_search.rs`（方案C）、`apps/backtest-engine/src/{indicators.rs,martingale/{kline_engine.rs,mod.rs,indicator_runtime.rs(新增)},walk_forward.rs}`（ATR parity）、`search.rs`。
- 是否验证：dd门控放宽 + 方案C + long_short 已验证（lshort portfolio_count=1, dd 7.66%）；worker 镜像在跑。

### 修改 3：conservative 搜索超时公式（**未提交**，工作树）
- 修改文件：`apps/backtest-worker/src/main.rs:234-245`。
- 状态：当前估值偏乐观导致 dir1lowadx 超时失败，需调（§7 问题 1）。

### 修改 4：前端回测 UI P0/P1 + 订单/策略页（**未提交**，41 文件）
- 修改原因：回测 UI 硬编码占位、风险展示不可读；新增订单页/策略创建 API。
- 是否验证：P0/P1 约 41 天前 `tsc --noEmit` 通过（据记忆）；本次 41 文件未单独 lint/build。

### 修改 5：用户确认过的需求（重要约束）
- **conservative 必须先突破 ann>50%&dd≤10% 才能继续 balanced/aggressive**（硬顺序）。
- 方向必须 `long_short`（用户原话"long/short/long+short 三种方向结合"）。
- owner=`flyingkid2022@outlook.com`；每档只见一个最佳，其余归档。
- 回测 ⇄ 实盘 parity。
- **TP/SL 优化是用户特别要求**（三档达标后，据策略结果做）。

### 最近废弃的方案
- long-only conservative（4 次失败）、方案C-for-long-only（失败）、dd门控当根因（证伪）。

---

## 9. 下一步开发计划

### 🔴 下一步最优先：修 conservative 搜索超时 + 重跑
- **目标**：让 long_short 搜索能跑完 → 得到真实 ann 分布 → 判断 conservative 能否突破。
- **原因**：当前唯一在跑的 conservative 任务已超时失败，worker 空闲。不修超时，任何重跑都会再次失败。
- **涉及文件**：`apps/backtest-worker/src/main.rs:234-245`（公式）+ `:5142-5144`（同步改单测）。
- **具体步骤**：
  1. 改 `long_short_search_timeout_secs`：`.clamp(600, 14_400)`→`28_800`，`* 240`→`* 360`（或更保守）。
  2. 同步改 `:5142-5144` 单测断言。
  3. `docker build` 重 build 镜像（§6.4）→ 重启 worker。
  4. 同时核对 `dir1lowadx` 完整 config（`SELECT config FROM backtest_tasks WHERE task_id='...'`），决定重跑用 dir1lowadx 参数还是换方案D/E。
  5. 提交新任务（§6.5，owner=flyingkid2022，long_short，seed 887/1597）。
- **验收标准**：任务跑完（status=succeeded），产出 portfolio_count≥1 且 best_ann/best_dd 可读。

### 🟠 第二优先：conservative ann 突破（搜索跑完后）
- **目标**：ann>50% & dd≤10%。
- **原因**：用户硬指标，不突破不进 balanced。
- **具体步骤**：
  - 若重跑 ann 仍<50%：上方案D（search.rs conservative 分支参数放宽）+ 方案E（ADX +1800），重 build，换 seed 重搜。
  - 若仍不行：与用户确认放宽组合 dd 到 12-15%，或标记瓶颈转 balanced。
  - 达标：归档旧 baseline（改 owner archive），保留一个最佳给 FlyingKid。

### 🟡 第三优先：balanced（conservative 达标后）
- **目标**：超 `fk-18-bal-v2-seed53-20260601`（65.52%/19.32%），dd≤20%。
- **具体步骤**：long_short，seeds 67/173/307/521，统一配置（§6.5）。

### 🟡 第四优先：aggressive
- **目标**：超 `fk-18-agg-v2-seed173-20260601`（77%/28.03%），dd≤30%。
- **具体步骤**：long_short，seeds 67/211/307/521。

### 🟣 第五优先：实盘 ATR 闭环补全（三档达标后）
- 7 gap（§7 问题 2）：马丁参数反序列化 + leverageBracket + TP/SL 交易所端兜底（`place_usdm_algo_order` 挂 reduceOnly+closePosition）+ **TP/SL 优化（用户特别要求）**。详 `docs/superpowers/plans/2026-06-13-martingale-live-atr-parity-plan.md`。

### 🟢 第六优先：最终报告 + 清理
- 写 `docs/superpowers/reports/2026-06-11-martingale-three-risk-search-report.md`（三档结果 + parity 验证）。
- 清理 `.monitor_*.txt` 临时文件；决定是否 commit 工作树/push 2 commit。

### ⚠️ 不要现在做 / 容易过度开发
- **不要现在做实盘 ATR 闭环**（用户明确顺序：三档搜索达标后才做）。
- **不要用 long-only**（已证不可行）。
- **不要在没核对 dir1lowadx config 前盲目重跑同参数**。
- **不要过度调超时公式**（先小改验证，避免一次放开到无界导致单任务跑几天）。
- **先验证再写**：改超时公式后先跑一个币种少的测试任务确认不超时，再上 30 币种全量。
- **TP/SL 优化**等策略结果出来再做，不要预写。

### 给新 AI 的环境快照（接手即用）
- worker：`grid-binance-backtest-worker-1` Up 9h，**空闲**（无 queued/running 任务）。
- DB：`docker exec grid-binance-postgres-1 psql -U postgres -d grid_binance`。
- 监控 cron：**已全删**（如需重建，CronCreate 每小时，决策树参考记忆 martingale-conservative-bottleneck）。
- 当前无 active 任务 → 接手第一步可直接改超时公式 + 重 build + 提交新任务。
