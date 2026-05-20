# 马丁回测 CPU 并行深搜性能优化 Spec

日期：2026-05-20
状态：待确认

## 背景

当前马丁回测已经具备 long/short/long_short 参数搜索、单币种 Top10、组合 Top3、资金曲线、回撤曲线与交易明细能力。但在 7 币种深搜验证中，`backtest-worker` 容器 CPU 仅约 109%，实际接近单核运行；`BACKTEST_WORKER_MAX_THREADS=2` 仅作为环境配置存在，候选筛选与二阶段精筛仍按顺序执行。

用户机器具备高性能 CPU（9950X）和 5090 GPU。现阶段马丁回测是逐 K 线状态机，包含加仓、止盈、止损、手续费、强平、双向腿状态等大量分支。直接 GPU 化需要重写为批量向量化 kernel，成本高、风险大、容易造成回测语义偏差。因此本轮先做 CPU 并行，把 9950X 多核心用于候选筛选/精筛；GPU 暂不进入本轮实现。

## 目标

1. 充分利用 CPU：让候选粗筛、二阶段精筛可以按 `BACKTEST_WORKER_MAX_THREADS` 并行执行。
2. 保持回测准确性：并行只改变执行顺序，不改变单个候选回测语义、手续费、杠杆本金、止损、止盈、资金曲线计算。
3. 保持结果稳定：同一请求、同一随机种子、同一行情数据，应得到确定性的候选排序和组合结果。
4. 保持可控资源：默认仍保守，生产可通过环境变量提高并发；避免把数据库、内存、CPU 打满导致前端/API 不可用。
5. 缩短 7 币种深搜验证耗时：同等参数预算下，搜索耗时应明显低于当前顺序执行。

## 非目标

1. 本轮不重写 GPU kernel，不引入 CUDA/OpenCL 依赖。
2. 本轮不改变收益评分公式和组合算法的目标函数，除非发现并行化必须修复的确定性问题。
3. 本轮不扩大搜索空间本身；搜索更深应通过用户配置 `random_candidates/intelligent_rounds/search_space` 控制。
4. 本轮不改前端交互，除非需要展示并行状态或错误提示。

## 现状问题

- `deploy/docker/docker-compose.yml` 默认 `BACKTEST_WORKER_MAX_THREADS=2`，不适合高性能本机深搜。
- `WorkerConfig.max_threads` 当前只读取和打印，没有实际驱动候选并发。
- `run_long_short_staged_search()` 中候选粗筛循环是顺序执行。
- 新增的 long_short 二阶段精筛对 survivor 逐个展开、逐个筛选，进一步放大顺序瓶颈。
- 单个 7 币种任务在 ADA 阶段长时间停留，CPU 约 109%，说明不是行情 IO 瓶颈，而是单候选 CPU 顺序计算瓶颈。

## 设计方案

### 1. Worker 并发配置

- 保留 `BACKTEST_WORKER_MAX_THREADS` 作为全局搜索并发上限。
- 本机部署默认建议从 `2` 提高到 `24`，给 9950X 留出系统/API/数据库余量。
- 代码层对非法值做保护：最小 `1`，建议最大不超过可用并行度；若环境值过大，应 clamp 到合理上限或记录警告。

### 2. 候选批量并行筛选

新增一个内部工具函数，例如：

- 输入：候选列表、线程上限、每个候选的筛选闭包。
- 输出：按原候选顺序归并后的 `Vec<EvaluatedCandidate>` 和 `Vec<CandidateRejectionSample>`。
- 行为：
  - 候选可以并行执行 `run_candidate_kline_screening()`。
  - 每个候选独立读取已加载的 `MarketDataContext`。
  - 不在并行闭包内写数据库、写 artifact 或更新任务状态。
  - 完成后统一排序/截断，确保结果确定。

### 3. long_short 二阶段精筛并行化

- 第一层：coarse candidates 并行筛选。
- 第二层：survivor 生成 fine candidates 后，fine candidates 也并行筛选。
- 为避免任务爆炸：
  - survivor 数量仍限制为 `min(per_symbol_top_n.max(10), 24)`。
  - 每个 survivor 的 fine budget 仍为 `max(task.random_candidates, 12)`，`intelligent_rounds=1`。
  - 总并发由同一个 `max_threads` 控制，不允许 survivor × candidate 无限并发。

### 4. 单向搜索路径兼容

- 单向 `intelligent_search()` 当前有自己的流程，本轮不强行重构全部算法。
- 但若发现单向路径也明显顺序瓶颈，可以只把 refinement survivor 的 fine search 部分纳入同一批量并行工具。
- 优先保证 long_short，因为用户当前核心诉求是双向马丁深搜。

### 5. 取消/暂停/超时

- 并行筛选期间仍需要周期性检查 cancel/pause。
- 本轮最小实现：每批候选开始前检查 cancel；批量完成后再次检查。
- 如果已有 AtomicBool 取消机制可复用，则并行闭包中也读取取消标记。
- 超时逻辑从“每 5 个候选检查一次 elapsed”调整为“批量调度前/批量归并后检查 elapsed”，避免并行时误判。

### 6. 数据安全与确定性

- `MarketDataContext` 必须只读共享；若类型不满足 `Sync`，需要改为 `Arc<MarketDataContext>` 或预提取只读数据切片。
- 并行结果必须带原始 index，归并时按 index 排序后再进入评分排序。
- 候选 ID 仍由生成阶段确定，精筛 ID 加 survivor 前缀，避免不同 survivor 产生重复 ID。
- 不允许在并行闭包里调用会产生非确定随机数的逻辑；候选生成仍在并行前完成。

### 7. 资源参数建议

本机推荐配置：

```env
BACKTEST_WORKER_MAX_THREADS=24
```

可选压力测试配置：

```env
BACKTEST_WORKER_MAX_THREADS=28
```

不建议一开始设满 32 线程，因为还需要保留资源给 Postgres、Redis、API、前端和系统。

## 验收标准

### 功能验收

1. long_short 候选仍必须包含 long 和 short 两条腿。
2. 候选结果仍必须包含年化收益、最大回撤、杠杆、planned_margin、资金曲线、回撤曲线、交易预览。
3. 组合 Top3 仍必须是真组合，不是单候选；单币种总权重不得超过 80%。
4. 负收益候选不得进入最终可选结果。
5. 最大回撤限制仍按风险档位执行：保守 20%，均衡 25%，激进 30%，必要时只按既有受控 fallback 放宽。

### 性能验收

1. 部署后 `docker stats grid-binance-backtest-worker-1` 在深搜期间 CPU 应明显高于单核，目标至少 `800%+`，理想 `1600%+`，具体取决于线程数和行情数据大小。
2. 7 币种任务不应长时间卡在单币种且 CPU 仅约 `100%`。
3. 同预算 7 币种验证任务整体耗时应明显低于顺序版。

### 回归验收

必须通过：

```bash
cargo test -p backtest-worker long_short_candidate_selection_prioritizes_profit_potential_within_budget -- --nocapture
cargo test -p backtest-worker long_short_smoke_payload_expands_to_diverse_dual_leg_candidates -- --nocapture
cargo test -p backtest-worker explicit_long_short_search_budget_is_respected_for_wide_multisymbol_runs -- --nocapture
node tests/verification/backtest_worker_contract.test.mjs
```

建议补充测试：

- 并行筛选输出数量与顺序确定性测试。
- `max_threads=1` 与 `max_threads>1` 在固定候选输入下产生相同 TopN 的测试。
- long_short 二阶段精筛在并行后仍会保留高杠杆/高止盈潜力候选的测试。

## 风险与缓解

1. 并行读取数据结构不满足线程安全：先编译验证；必要时用 `Arc` 包装只读上下文。
2. 过高并发导致内存/缓存压力：通过 `BACKTEST_WORKER_MAX_THREADS` 限制并发，默认 24 而不是 32。
3. 结果排序非确定：候选带 index 归并，排序只使用 score + candidate_id 作为稳定 tie-break。
4. 任务取消响应变慢：批量大小不应无限，必要时按 chunk 调度。
5. 深搜收益仍不达 50% 年化：性能优化只提升搜索吞吐，不保证市场本身存在满足 20~25% 回撤约束且 50% 年化的真实策略。

## 待用户确认

确认后进入计划阶段，实施内容包括：

1. 增加并行筛选工具。
2. 接入 long_short 粗筛与精筛。
3. 调整 Docker 默认线程数到本机推荐值。
4. 增加确定性/并行回归测试。
5. 部署并重跑 7 币种验证任务。
