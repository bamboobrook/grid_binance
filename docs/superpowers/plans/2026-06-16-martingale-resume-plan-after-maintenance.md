# 设备维护后恢复执行计划（2026-06-16）

> 设备停电维护。重启后按此计划继续。目标：完成 conservative/balanced/aggressive 三档搜索 + 实盘 ATR 闭环补全。
> 前置总结：`docs/superpowers/reports/2026-06-16-martingale-conservative-exploration-summary.md`

## 〇、设备重启后恢复步骤

1. **启动 backtest-worker**（镜像 `grid-binance-backtest-worker:latest` 已含方案C+dd门控放宽+ATR/ADX parity，未删）：
```bash
docker run -d --name grid-binance-backtest-worker-1 --network grid-binance_default \
  -e DATABASE_URL="postgres://postgres:postgres@postgres:5432/grid_binance" \
  -e REDIS_URL="redis://redis:6379/0" \
  -e BACKTEST_ARTIFACT_ROOT="/var/lib/grid-binance/backtest-artifacts" \
  -e BACKTEST_MARKET_DATA_DB_PATH="/market-data/market_data.db" \
  -e BACKTEST_WORKER_MAX_THREADS=24 -e BACKTEST_WORKER_POLL_MS=5000 \
  -v grid-binance_backtest-artifacts:/var/lib/grid-binance/backtest-artifacts \
  -v /home/bumblebee/Project/discord_c2im/pipeline/data:/market-data:ro \
  -e APP_NAME=backtest-worker grid-binance-backtest-worker:latest
```
   若镜像被清：`docker build -f deploy/docker/rust-service.Dockerfile --build-arg APP_NAME=backtest-worker -t grid-binance-backtest-worker:latest .`
2. **确认 worker 启动**：`docker logs grid-binance-backtest-worker-1`（max_threads=24）
3. **新建 cron 监控 1h**（CronCreate `13 * * * *`，prompt 参考 memory martingale-conservative-bottleneck，决策树：succeeded→查 portfolio ann/dd；达标→下一档；未达标→下一 seed/调参）
4. **确认无 active 任务**（lshort 已归档，worker 已停）

## 一、conservative 突破 ann>50%（当前 dd 已达标 7.66%，ann 4.52% 不够）

lshort 证明 short 对冲解决 dd，瓶颈在 ann。提 ann 方向（按 ROI）：
- **方案D（首选）搜索参数放宽**（`apps/backtest-engine/src/search.rs:206-222` conservative 分支）：leverage 加 7,8；take_profit_bps 加 160,200；max_legs 加 7；spacing_bps 加 80。提 ann 也提 dd，配合 dd 门控放宽 + short 对冲压组合 dd。需重建镜像。
- **方案E ADX 强过滤**：adx_threshold_bps 加 1800（当前 2000/2500/3000 偏高过滤中等趋势），提高入场质量。
- 重搜 conservative long_short（per_sym=10, port_n=10, 方案C, dd门控放宽, seed 887/1597）。**long_short 必须保留**（short 对冲是 dd 突破关键）。
- 目标：组合 ann>50% & dd≤10%。
- 若仍不够：考虑放宽组合 dd 到 12-15%（与用户确认）或接受 ann~30-40% 最佳 + 标记瓶颈转 balanced。

## 二、balanced（long_short 必用，方向修正）

- direction_mode=**long_short**，seeds 67,173,307,521
- 目标超 `fk-18-bal-v2-seed53-20260601` (65.52%/19.32%)，dd≤20%
- 配置同 conservative（方案C, dd门控放宽, per_sym=10, port_n=10）

## 三、aggressive（long_short）

- direction_mode=**long_short**，seeds 67,211,307,521
- 目标超 `fk-18-agg-v2-seed173-20260601` (77%/28.03%)，dd≤30%，挑战 ~100.5%

## 四、实盘 ATR 闭环补全（`docs/superpowers/plans/2026-06-13-martingale-live-atr-parity-plan.md`）

另一个 agent 已修 5 项 Binance API 正确性（commit d4f8474/merge 1b0ad35，已合入 main）。我补：
1. **马丁参数硬编码**（`martingale_runtime_config_from_strategy` multiplier=1/max_legs=3/TP bps=100）→ 从策略 config 反序列化真实值
2. **leverageBracket 名义价值上限校验**
3. **TP/SL 无交易所端兜底**（进程崩溃无保护）→ 每腿成交后挂 reduceOnly + closePosition=true 条件单
4. **TP/SL 优化（用户特别要求）**：根据策略最终结果的 TP/SL 模型（ATR/Percent/Trailing/Mixed）+ 搜索参数，在实盘正确实现（evaluate_strategy_exit + 进程级 INDICATOR_FEEDS 持久化 + 持续评估路径 + parity 测试）

## 五、最终报告

三档结果 + 实盘 parity 验证 → `docs/superpowers/reports/2026-06-11-martingale-three-risk-search-report.md`（plan Task6 表格）。

## 统一配置（所有搜索）

```
random_candidates=64, per_symbol_top_n=10, portfolio_top_n=10,
direction_mode=long_short, search_space_mode=risk_profile_auto,
fee_bps=4.5, slippage_bps=2.0, start_ms=1672531200000, end_ms=1780271999999,
time_range_mode=auto_since_2023_to_last_month_end, extended_universe=true,
search_mode=profit_optimized_v2, intelligent_rounds=5
```
提交模板：`INSERT backtest_tasks(task_id='fk-18-{risk}-seed{N}-{date}', owner='flyingkid2022@outlook.com', status='queued', strategy_type='martingale_auto_search', config='{...}'::jsonb, summary='{}'::jsonb)`

## 关键约束

- owner=flyingkid2022@outlook.com；FlyingKid 可见每档只保留一个最佳，其余归档 archive+flyingkid2022@outlook.com
- 回测+实盘 parity（指标语义一致）
- worker 镜像 = 方案C + dd门控放宽 + ATR/ADX parity（已 build，未 commit；恢复时若镜像在则直接用，否则重建）
- dd 门控放宽 + 方案C + long_short 已验证（lshort portfolio_count=1, dd 7.66%）
