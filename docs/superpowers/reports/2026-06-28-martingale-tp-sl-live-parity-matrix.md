# 马丁 TP/SL 实盘一致性支持矩阵（P0.3）

> 日期：2026-06-28
> 目的：明确回测引擎支持的 TP/SL 模型在 trading-engine 实盘的可复现性，
> 划定本轮小资金搜索允许的模型子集，避免搜索出「回测幻觉」候选。
> 审计方法：直接读 backtest-engine 与 trading-engine 源码，逐模型比对。

## 结论（先看这里）

本轮小资金搜索**只允许**以下两个模型组合进入最终候选：

- **Take Profit**：`Percent { bps }`
- **Stop Loss**：`StrategyDrawdownPct { pct_bps }`

其余 TP/SL 模型要么在实盘无实现，要么实盘回退到粗略近似（不是真值），
不允许进入最终搜索空间。Trailing/Mixed TP 标记为「待验」，留待第二轮
补齐实盘 callback 重挂逻辑后再开放。

## Take Profit 模型矩阵

| 模型 | 回测支持 | trading-engine 实盘 | 实盘下单方式 | 最终搜索允许 |
|---|:---:|:---:|---|:---:|
| `Percent { bps }` | ✅ | ✅ | `martingale_percent_take_profit_price` 算精确价 + 市价平仓（`main.rs:1933-1947`） | ✅ |
| `Trailing { activation_bps, callback_bps }` | ✅ | ⚠️ 部分 | 实盘只用 `activation_bps` 当固定 bps（`main.rs:988`），**未实现 callback 重挂**，非真 trailing | ❌ 本轮禁用 |
| `Mixed { phases }` | ✅ | ⚠️ 部分 | 只取第一个 `Percent` phase 的 bps（`main.rs:989-995`），其余 phase 丢失 | ❌ 本轮禁用 |
| `Amount { quote }` | ✅ | ❌ | `take_profit_bps_for_revision` 回退到固定 100bps（`main.rs:996`），不是按金额算的精确价 | ❌ 本轮禁用 |
| `Atr { multiplier }` | ✅ | ❌ | 同上回退到 100bps（`main.rs:996`），不是 ATR 倍数算的精确价 | ❌ 本轮禁用 |

**关键代码事实**：`martingale_percent_take_profit_price`（`main.rs:1937-1939`）
对任何非 `Percent` 模型直接返回 `None`，即实盘订单提交路径**只认 Percent**。
`take_profit_bps_for_revision`（`main.rs:985-998`）虽然对其他模型给了「回退 bps」，
但那只是监控/通知用的近似，不是真实 TP 触发价。

## Stop Loss 模型矩阵

| 模型 | 回测支持 | trading-engine 实盘 | 实盘触发方式 | 最终搜索允许 |
|---|:---:|:---:|---|:---:|
| `StrategyDrawdownPct { pct_bps }` | ✅ | ✅ | `martingale_strategy_drawdown_pct` 算策略级回撤 + 市价平仓（`main.rs:1917`、`martingale_exit.rs:26`） | ✅ |
| `PriceRange { lower, upper }` | ✅ | ❌ | 实盘无触发路径 | ❌ 本轮禁用 |
| `Atr { multiplier }` | ✅ | ❌ | 实盘无 ATR 止损路径 | ❌ 本轮禁用 |
| `Indicator { expression }` | ✅ | ❌ | 实盘无指标止损路径（注意：`extract_symbol_dependencies` 会扫这个表达式的跨币种依赖，但实盘不触发止损） | ❌ 本轮禁用 |
| `SymbolDrawdownAmount { quote }` | ✅ | ❌ | 实盘无路径 | ❌ 本轮禁用 |
| `GlobalDrawdownAmount { quote }` | ✅ | ❌ | 实盘无路径 | ❌ 本轮禁用 |

**关键代码事实**：`martingale_exit.rs:26` 只 match `StrategyDrawdownPct`，
其余 5 种 stop 模型在实盘 exit 路径里**完全不被处理**。

## 对搜索空间的约束

`live_parity_check(config)`（见 `search_small_capital_martingale.rs` 与
`scripts/validate_martingale_portfolio_robustness.py`）必须拒绝任何使用
「本轮禁用」TP/SL 模型的候选进入最终结果。允许的模型组合只有：

```text
take_profit = Percent { bps }
stop_loss   = StrategyDrawdownPct { pct_bps }   # 或 None
```

任何带 Trailing/Mixed/Amount/Atr TP，或 PriceRange/Atr/Indicator/
SymbolDrawdown/GlobalDrawdown SL 的候选，即使在回测里达标，也不能作为
最终候选提交，因为它在实盘无法复现。

## 后续开放计划（不在本轮）

- **Trailing TP 实盘**：需要实现 callback 价随价格创新高重挂的内部状态机，
  对应 Binance `TRAILING_STOP_MARKET`（需 `callbackRate`、`activatePrice`）。
  补齐后单测 + 实盘 dry-run 验证再开放。
- **ATR TP / ATR SL 实盘**：需要每根 bar 重算 ATR 触发价并内部重挂。
- **Indicator SL 实盘**：需要把 `Indicator{expression}` 在每根 bar 求值，
  触发后内部 MARKET reduceOnly。
- 这些都属于「改变交易语义最多」的改动，按 ChatGPT 计划第 7 节优先级
  放到最后，且必须在结构化配置 + 实盘 runtime 双路径都有实现和测试后
  才允许进入搜索空间。
