# Claude 完成度复核补充说明（2026-04-22）

> 复核目标：检查“Claude 已按 `docs/superpowers/plans/2026-04-21-full-audit-and-fix-plan.md` 全部完成”这一说法是否成立，并给出继续执行清单。  
> 复核人：Codex  
> 复核范围：`/home/bumblebee/Project/grid_binance/.worktrees/full-v1`

---

## 1. 先说结论

**不能认定为“已经全部完成”。**

原因不是主观判断，而是当前仓库里有直接证据表明：

1. 用户提到的 `2026-04-21-full-audit-and-fix-plan.md` **不在本项目仓库内**。  
   本仓库 `docs/superpowers/plans/` 下不存在该文件。
2. 当前项目里与之最接近、且实际存在的最新计划文件是：
   - `docs/superpowers/plans/2026-04-12-multi-agent-review-gap-closure-plan.md`
   - `docs/superpowers/plans/2026-04-13-strategy-engine-rewrite-plan.md`
3. 以这两份真实存在的计划为准继续复核后，发现：
   - Rust 工作区当前**无法通过 Cargo 解析**
   - 多个前端合约测试**直接失败**
   - 若干计划中要求的新文件/拆分结构**没有按计划落地**
   - 策略详情页、总览页仍存在明显占位/假数据实现

因此，Claude 现在最多只能算“做了一部分”，**不能算按计划全部完成**。

---

## 2. 文件定位结论

### 2.1 用户指定计划文件不存在于当前项目

未找到：

- `docs/superpowers/plans/2026-04-21-full-audit-and-fix-plan.md`

实际在别的项目里找到同名文件：

- `/home/bumblebee/Project/optimize/docs/superpowers/plans/2026-04-21-full-audit-and-fix-plan.md`

该文件内容是 **Bitfinex Lending Optimize**，并非当前 Binance Grid SaaS 项目，**不能拿来当本项目的完成依据**。

### 2.2 当前项目应以这两份计划为准

- [2026-04-12-multi-agent-review-gap-closure-plan.md](/home/bumblebee/Project/grid_binance/.worktrees/full-v1/docs/superpowers/plans/2026-04-12-multi-agent-review-gap-closure-plan.md)
- [2026-04-13-strategy-engine-rewrite-plan.md](/home/bumblebee/Project/grid_binance/.worktrees/full-v1/docs/superpowers/plans/2026-04-13-strategy-engine-rewrite-plan.md)

---

## 3. 已验证的未完成证据

### 3.1 Rust 工作区当前就是坏的，无法完成计划里的 Cargo 验证

执行：

```bash
source "$HOME/.cargo/env" && cargo test -p api-server --test strategy_flow create_strategy_returns_explicit_strategy_type_and_runtime_phase -- --nocapture
```

结果：

```text
error: failed to load manifest for workspace member `/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/backtest-engine`
...
error inheriting `anyhow` from workspace root manifest's `workspace.dependencies.anyhow`
...
`dependency.anyhow` was not found in `workspace.dependencies`
```

同样地，执行：

```bash
source "$HOME/.cargo/env" && cargo test -p trading-engine --test grid_runtime -- --nocapture
```

也被同一个错误阻断。

对应文件：

- [Cargo.toml](/home/bumblebee/Project/grid_binance/.worktrees/full-v1/Cargo.toml)
- [apps/backtest-engine/Cargo.toml](/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/backtest-engine/Cargo.toml)

结论：

- 只要这个问题没修，`2026-04-12` 与 `2026-04-13` 计划中所有 Rust 验证都不可能算完成。

### 3.2 04-13 计划中点名要求的文件并未按计划落地

缺失文件：

- `db/migrations/0008_strategy_engine_rewrite.sql`
- `apps/web/components/strategies/strategy-definition-sections.tsx`
- `apps/web/components/strategies/strategy-runtime-controls.tsx`

虽然部分逻辑可能被塞进了别的文件里，但**计划本身要求的结构拆分并没有完成**。

### 3.3 策略详情页仍是占位页，不符合策略工作台/运行态计划

执行：

```bash
node --test tests/verification/strategy_surface_contract.test.mjs
```

结果：**4 个测试里 2 个失败**

失败点包括：

1. `new strategy workspace exposes real symbol selection and strategy-type-aware controls`
2. `strategy inventory exposes batch lifecycle actions and row-level actions through real forms`

直接证据：

- [tests/verification/strategy_surface_contract.test.mjs](/home/bumblebee/Project/grid_binance/.worktrees/full-v1/tests/verification/strategy_surface_contract.test.mjs)
- [apps/web/app/[locale]/app/strategies/[id]/page.tsx](/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/app/[locale]/app/strategies/[id]/page.tsx)

当前详情页里只有：

- 当前价格
- 策略状态占位 `—`
- 累计收益占位 `—`
- 策略 ID

并没有按计划接入：

- `StrategyWorkspaceForm`
- 后端 `strategy_type` 到表单模型的映射
- 运行中可见的暂停/停止动作
- 策略运行态详情工作台

### 3.4 总览页仍然使用假数据，不符合 04-12 计划里的“真实表层数据”

执行：

```bash
node --test tests/verification/web_app_shell.test.mjs tests/verification/web_shell_surface_contract.test.mjs tests/verification/web_user_pages_i18n.test.mjs
```

结果：**17 个测试里 4 个失败**

关键失败点：

1. `user shell and dashboard avoid fabricated placeholder identity and expose real operating stats`
2. `user app routes do not rely on fabricated product state for critical truth`
3. `strategy workspace exposes batch actions, templates, and multi-level payload plumbing`
4. `shared shell visual system follows a professional trading-console contract`

直接证据：

- [apps/web/app/[locale]/app/dashboard/page.tsx](/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/app/[locale]/app/dashboard/page.tsx)

当前问题很明确：

- 使用 `generateMockPnlData()`
- 健康卡片使用硬编码 `health={{ running: 3, paused: 1, errorPaused: 0, stopped: 2, draft: 1 }}`
- 没有按测试要求接入 `fetchAnalytics()` / `fetchStrategies()`
- 没有输出手续费、资金费、会员状态、近期账户活动、资产分布图这些真实数据位

### 3.5 有些表面问题修过，但不能据此认定“全部完成”

执行：

```bash
node --test tests/verification/admin_access_theme_layout_contract.test.mjs tests/verification/telegram_rebind_visibility_contract.test.mjs
```

结果：**4 个测试全部通过**

这说明：

- 管理员登录上下文
- 浅色/深色主题关键入口
- 顶部状态卡一排显示
- Telegram 重新绑定可见性

这些点至少有部分修复。

但这只能证明“有一部分做了”，**不能覆盖前面那些仍然失败的关键项**。

---

## 4. 当前最需要 Claude 继续做的内容

请 Claude 继续执行，优先级按下面顺序：

### P0：先把 Rust 工作区修到可验证

1. 修复 [Cargo.toml](/home/bumblebee/Project/grid_binance/.worktrees/full-v1/Cargo.toml) 的 `workspace.dependencies`
2. 让 [apps/backtest-engine/Cargo.toml](/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/backtest-engine/Cargo.toml) 不再因为 `anyhow` 缺失导致整个工作区瘫痪
3. 重新跑：

```bash
source "$HOME/.cargo/env" && cargo test -p api-server --test strategy_flow -- --nocapture
source "$HOME/.cargo/env" && cargo test -p trading-engine --test grid_runtime --test execution_effects -- --nocapture
```

### P1：把策略详情页补成真正可用的工作台

目标文件：

- [apps/web/app/[locale]/app/strategies/[id]/page.tsx](/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/app/[locale]/app/strategies/[id]/page.tsx)

要求：

1. 不要再是占位页
2. 接入 `StrategyWorkspaceForm`
3. 正确映射后端 `strategy_type` / `reference_price_source`
4. 显示真实运行状态
5. 显示合法动作按钮，运行中至少要有暂停/停止
6. 重新跑：

```bash
node --test tests/verification/strategy_surface_contract.test.mjs
```

### P2：把总览页从假数据改成真实接口

目标文件：

- [apps/web/app/[locale]/app/dashboard/page.tsx](/home/bumblebee/Project/grid_binance/.worktrees/full-v1/apps/web/app/[locale]/app/dashboard/page.tsx)

要求：

1. 去掉 `generateMockPnlData()`
2. 去掉硬编码 health 数字
3. 改为接真实的 `fetchAnalytics()` / `fetchStrategies()` 或等价真实接口
4. 补上：
   - 手续费
   - 资金费
   - 会员状态
   - 最近账户活动
   - 资产分布图
5. 重新跑：

```bash
node --test tests/verification/web_app_shell.test.mjs tests/verification/web_shell_surface_contract.test.mjs tests/verification/web_user_pages_i18n.test.mjs
```

### P3：补齐 04-13 计划承诺的结构拆分，或者明确修订计划

目前计划里写了要拆成独立文件，但仓库里没落地。

Claude 需要二选一：

1. 真把这些文件补出来：
   - `db/migrations/0008_strategy_engine_rewrite.sql`
   - `apps/web/components/strategies/strategy-definition-sections.tsx`
   - `apps/web/components/strategies/strategy-runtime-controls.tsx`
2. 或者新增一份修订说明，明确“哪些内容改为等价落在别的文件中”，并更新计划，不要继续让计划与仓库结构不一致。

---

## 5. 这轮复核建议的验收口径

只有当下面四件事全部满足，才可以再说“已完成”：

1. Rust 工作区能正常通过 Cargo 解析与至少一轮关键测试
2. `tests/verification/strategy_surface_contract.test.mjs` 全绿
3. `tests/verification/web_app_shell.test.mjs tests/verification/web_shell_surface_contract.test.mjs tests/verification/web_user_pages_i18n.test.mjs` 全绿
4. 当前计划文档与实际仓库结构一致，不再出现“计划写了、仓库没有”的情况

---

## 6. 本次复核的最简短结论

**Claude 还没有全部做完。**

最核心的缺口有 4 个：

1. Rust 工作区坏了，Cargo 测不了
2. 策略详情页还是占位页
3. 总览页还是假数据
4. 04-13 计划承诺的拆分文件没有完整落地

请 Claude 先按本文件第 4 节继续执行，再回来交付验收证据。
