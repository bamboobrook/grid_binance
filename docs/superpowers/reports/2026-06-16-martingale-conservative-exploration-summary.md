# Conservative ann>50%&dd≤10% 探索总结（2026-06-13 ~ 2026-06-16）

> 设备维护前总结。用户目标：conservative 年化>50% 且 max_dd≤10%（新窗口 2023-01~2026-05，18 币 extended universe，fee 4.5bps/slippage 2.0bps，1m K线）。

## 一、探索历程

| 任务 | direction | 关键改动 | portfolio_count | 最佳组合 ann/dd | 结论 |
|------|-----------|---------|:---:|---|------|
| seed521 | long | 旧 worker(fixed_percent) | 0 | — | 候选全被 dd 门控排除 |
| seed521-atr | long | ATR parity | 3 | 3-5% / 6-9% | 组合池只剩低 ann |
| seed521-ddfix | long | dd 门控放宽(20→50) | 0 | — | long 高相关，组合 dd 压不到 10% |
| optc | long | 方案C(极小权重+风险平价) | 0 | — | 权重再优也无法克服 long 高相关 |
| **lshort** | **long_short** | **short 对冲** | **1** | **4.52% / 7.66%** | **short 对冲突破 dd(7.66≤10)，但 ann 4.52<50** |

## 二、关键发现

1. **dd 门控 bug**（已修，`backtest-worker/src/main.rs` `scoring_config_from_task`）：搜索阶段 survival_valid 用 dd_limit(=20) 同时设 global+strategy drawdown，排除所有 dd>20% 高 ann 候选。已放宽 `screening_dd_cap=(max_dd*2.5).max(50)`。
2. **long 高相关死结**：long-only 候选（BTC/INJ/ETH）回撤同步，组合 dd 压不到 10%（ddfix/optc portfolio_count=0）。
3. **short 对冲有效**：long_short 让组合 dd 可达 10%（lshort portfolio_count=1, dd 7.66%）——结构性突破。
4. **候选 ann 天花板**：新窗口下高 ann(>50%) 必伴高 dd(>35%)，低 dd(≤10%) 候选 ann<5%。**组合 ann 难破 50%**——这是 conservative 当前真正瓶颈（不是 dd，dd 已被 short 对冲解决）。

## 三、代码改动（已部署 backtest-worker 镜像，未 git commit）

- `apps/backtest-worker/src/main.rs` `scoring_config_from_task`：dd 门控放宽
- `apps/backtest-engine/src/portfolio_search.rs`：极小权重 stabilizer 模板（60%+4%×n）+ `stochastic_allocations` 风险平价（ann√/dd）
- backtest-engine ATR/ADX parity（DeepSeek working tree）+ rss_mb/refinement DB 进度修复

镜像 build 方法：`docker build -f deploy/docker/rust-service.Dockerfile --build-arg APP_NAME=backtest-worker -t grid-binance-backtest-worker:latest .`（compose build 因 env 插值失败）

## 四、当前最佳 conservative

- **lshort portfolio-1**：ann 4.52% / dd 7.66%（short 对冲，dd 达标，ann 不够）—— long_short+方案C 在新窗口的最佳
- 旧 baseline `fk-18-conservative-baseline-from-v5-20260611`：40.69%/9.66%（旧窗口 2025-04，新窗口下候选 ann 更低）

## 五、瓶颈结论

conservative `ann>50% & dd≤10%` 在 2023-2026.5 窗口下：
- **dd 瓶颈已解决**（short 对冲 + 方案C）
- **ann 瓶颈仍在**：候选 ann 天花板低。需策略层面突破（搜索参数放宽提 ann / ADX 强过滤提质量 / 币种时段筛选），见恢复计划。

关联：[[2026-06-16-martingale-resume-plan-after-maintenance]]、[[martingale-conservative-bottleneck]]、[[martingale-live-atr-parity-gap]]。
