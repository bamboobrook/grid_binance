"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import { BacktestCharts } from "@/components/backtest/backtest-charts";
import { BacktestProfessionalPanel } from "@/components/backtest/backtest-professional-panel";
import { BacktestResultTable, type PortfolioMember, type PortfolioTop3Row } from "@/components/backtest/backtest-result-table";
import { BacktestTaskList } from "@/components/backtest/backtest-task-list";
import { BacktestWizard } from "@/components/backtest/backtest-wizard";
import { MartingaleRiskWarning } from "@/components/backtest/martingale-risk-warning";
import { PortfolioCandidateReview, type PortfolioBasketItem } from "@/components/backtest/portfolio-candidate-review";
import { publishPortfolio, requestBacktestApi } from "@/components/backtest/request-client";
import type { MartingaleBacktestCandidateSummary, PortfolioRecalculateResponse } from "@/lib/api-types";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";
import { cn } from "@/lib/utils";

type ConsoleTab = "wizard" | "professional";

type ApiTask = {
  task_id?: string;
  status?: string;
  strategy_type?: string;
  summary?: Record<string, unknown> | null;
  config?: Record<string, unknown> | null;
  updated_at?: string;
  created_at?: string;
};

type ApiCandidate = {
  candidate_id?: string;
  status?: string;
  rank?: number;
  config?: Record<string, unknown> | null;
  summary?: Record<string, unknown> | null;
};

type BacktestTask = {
  id: string;
  name: string;
  status: string;
  rawStatus: string;
  progress: string;
  stage: string;
  updatedAt: string;
  summary: Record<string, unknown> | null;
  config?: Record<string, unknown> | null;
};

type BacktestCandidate = {
  id: string;
  symbol: string;
  market: string;
  direction: string;
  searchMode: string;
  score: string;
  drawdown: string;
  returnPct: string;
  tradeCount: string;
  parameters: string;
  decision: string;
  rank?: number;
  summary: MartingaleBacktestCandidateSummary;
  rawConfig?: Record<string, unknown>;
};

type RefreshTasksOptions = {
  selectLatest?: boolean;
};

const SURFACE_TAGS = [
  "随机搜索",
  "智能搜索",
  "Hedge Mode",
  "逐仓",
  "全仓",
  "Portfolio",
  "生存优先",
] as const;

export function BacktestConsole({ lang, locale }: { lang: UiLanguage; locale: string }) {
  const [activeTab, setActiveTab] = useState<ConsoleTab>("wizard");
  const [tasks, setTasks] = useState<BacktestTask[]>([]);
  const [candidates, setCandidates] = useState<BacktestCandidate[]>([]);
  const [selectedTaskId, setSelectedTaskId] = useState("");
  const [selectedCandidate, setSelectedCandidate] = useState<BacktestCandidate | null>(null);
  const [selectedPortfolio, setSelectedPortfolio] = useState<PortfolioTop3Row | null>(null);
  const [basketItems, setBasketItems] = useState<PortfolioBasketItem[]>([]);
  const [sandboxItems, setSandboxItems] = useState<PortfolioBasketItem[]>([]);
  const [sandboxResult, setSandboxResult] = useState<PortfolioRecalculateResponse | null>(null);
  const [sandboxPending, setSandboxPending] = useState(false);
  const [sandboxFeedback, setSandboxFeedback] = useState("");
  const [sandboxOpen, setSandboxOpen] = useState(false);
  const [publishOpen, setPublishOpen] = useState(false);
  const [feedback, setFeedback] = useState("");
  const [loading, setLoading] = useState(true);
  const activePanelId = activeTab === "wizard" ? "backtest-wizard-panel" : "backtest-professional-panel";
  const activeTabId = activeTab === "wizard" ? "backtest-wizard-tab" : "backtest-professional-tab";

  const refreshTasks = useCallback(async (options: RefreshTasksOptions = {}) => {
    setLoading(true);
    const result = await requestBacktestApi("/api/user/backtest/tasks", { cache: "no-store" });
    setLoading(false);
    if (!result.ok) {
      setFeedback(result.message);
      return;
    }
    const apiTasks = Array.isArray(result.data) ? result.data as ApiTask[] : [];
    const normalized = apiTasks.map((task) => normalizeTask(task, lang));
    setTasks(normalized);
    const firstTaskId = normalized[0]?.id ?? "";
    setSelectedTaskId((current) => {
      if (options.selectLatest) return firstTaskId;
      if (current && normalized.some((task) => task.id === current)) return current;
      return firstTaskId;
    });
    setFeedback("");
  }, [lang]);

  const refreshCandidates = useCallback(async (taskId: string) => {
    if (!taskId) {
      setCandidates([]);
      setSelectedCandidate(null);
      return;
    }
    const result = await requestBacktestApi(`/api/user/backtest/tasks/${taskId}/candidates`, { cache: "no-store" });
    if (!result.ok) {
      setCandidates([]);
      setSelectedCandidate(null);
      setFeedback(result.message);
      return;
    }
    const apiCandidates = Array.isArray(result.data) ? result.data as ApiCandidate[] : [];
    const normalized = apiCandidates.map((candidate) => normalizeCandidate(candidate, lang));
    setCandidates(normalized);
    setSelectedCandidate(normalized[0] ?? null);
    setSelectedPortfolio(null);
  }, [lang]);

  useEffect(() => {
    void refreshTasks();
  }, [refreshTasks]);

  useEffect(() => {
    void refreshCandidates(selectedTaskId);
  }, [refreshCandidates, selectedTaskId]);

  const selectedTask = useMemo(
    () => tasks.find((task) => task.id === selectedTaskId) ?? null,
    [selectedTaskId, tasks],
  );

  useEffect(() => {
    if (!selectedTaskId || isTerminalTaskStatus(selectedTask?.rawStatus)) return;
    const timer = window.setInterval(() => {
      void refreshTasks();
      void refreshCandidates(selectedTaskId);
    }, 5000);
    return () => window.clearInterval(timer);
  }, [refreshCandidates, refreshTasks, selectedTask?.rawStatus, selectedTaskId]);

  const selectedTaskName = useMemo(
    () => selectedTask?.name ?? selectedTaskId,
    [selectedTask, selectedTaskId],
  );

  const selectedSummary = sandboxResult ? portfolioSandboxSummaryForCharts(sandboxResult) : selectedPortfolio ? portfolioSummaryForCharts(selectedPortfolio) : (selectedCandidate?.summary ?? {});
  const selectedDetailTitle = selectedPortfolio
    ? pickText(lang, `组合 #${selectedPortfolio.portfolio_rank || "?"} 图表与明细`, `Portfolio #${selectedPortfolio.portfolio_rank || "?"} charts and details`)
    : selectedCandidate
      ? pickText(lang, `${selectedCandidate.symbol} 候选图表与明细`, `${selectedCandidate.symbol} candidate charts and details`)
      : pickText(lang, "回测图表", "Backtest charts");
  const taskSummary = selectedTask?.summary ?? {};
  const portfolioPoolNote = typeof taskSummary.portfolio_pool_note === "string" ? taskSummary.portfolio_pool_note : "";
  const portfolioTopN = typeof taskSummary.portfolio_top_n === "number" ? taskSummary.portfolio_top_n : 3;
  const expandedUniverseSymbolCount = typeof taskSummary.expanded_universe_symbol_count === "number" ? taskSummary.expanded_universe_symbol_count : null;
  const portfolioPoolCandidateCount = typeof taskSummary.portfolio_pool_candidate_count === "number" ? taskSummary.portfolio_pool_candidate_count : null;

  function candidateToBasketItem(candidate: BacktestCandidate, weightPct: number, index: number): PortfolioBasketItem {
    const recommendedLeverage = candidate.summary.recommended_leverage ?? candidate.summary.max_leverage_used ?? 1;
    return {
      localId: `${candidate.id}-${Date.now()}-${index}`,
      candidateId: candidate.id,
      taskId: selectedTaskId,
      selectedTaskId,
      symbol: candidate.symbol,
      market: candidate.market,
      direction: candidate.direction,
      riskProfile: candidate.summary.risk_profile ?? "balanced",
      parameters: candidate.parameters,
      recommended_weight_pct: weightPct,
      recommended_leverage: recommendedLeverage,
      weightPct: String(weightPct),
      leverage: String(recommendedLeverage),
      enabled: true,
      parameterSnapshot: candidate.rawConfig ?? { description: candidate.parameters },
      metricsSnapshot: { ...candidate.summary },
    };
  }

  function addCandidateToBasket(candidate: BacktestCandidate) {
    const recommendedWeightPct = candidate.summary.recommended_weight_pct ?? (basketItems.length === 0 ? 100 : 0);
    setBasketItems((current) => [...current, candidateToBasketItem(candidate, recommendedWeightPct, current.length)]);
  }

  function addCandidateToSandbox(candidate: BacktestCandidate) {
    if (candidate.summary.publishable === false) {
      setSandboxFeedback(pickText(lang, "该候选超过回撤限制，只能作为诊断查看，不能加入沙盒。", "This candidate exceeds drawdown limits and cannot be added to the sandbox."));
      return;
    }
    setSandboxItems((current) => [...current, candidateToBasketItem(candidate, current.length === 0 ? 100 : 0, current.length)]);
  }

  function editPortfolioSandbox(portfolio: PortfolioTop3Row) {
    const missingMembers: PortfolioMember[] = [];
    const items = portfolio.members.flatMap((member, index) => {
      const candidate = candidates.find((entry) => entry.id === member.candidate_id);
      if (!candidate) {
        missingMembers.push(member);
        return [];
      }
      return [candidateToBasketItem(candidate, member.allocation_pct, index)];
    });
    setSandboxItems(items);
    setSandboxOpen(true);
    setSandboxResult(portfolioToSandboxResult(portfolio));
    setSelectedPortfolio(portfolio);
    setSelectedCandidate(null);
    if (missingMembers.length > 0) {
      setSandboxFeedback(pickText(lang, `组合中有 ${missingMembers.length} 个候选未入库，旧任务无法直接发布；请重新回测生成新组合，或只用已入库候选重新计算后发布。`, `${missingMembers.length} portfolio members were not persisted by this old task; rerun the backtest or recalculate using only persisted candidates before publishing.`));
      setPublishOpen(false);
      return;
    }
    setSandboxFeedback(pickText(lang, "已载入组合沙盒，可继续增删策略并重算。", "Portfolio loaded into sandbox; you can edit and recalculate."));
  }

  async function recalculateSandbox() {
    const enabledItems = sandboxItems.filter((item) => item.enabled);
    const totalWeight = enabledItems.reduce((sum, item) => sum + (Number(item.weightPct) || 0), 0);
    if (Math.abs(totalWeight - 100) > 0.01) {
      setSandboxFeedback(pickText(lang, "启用项权重合计必须为 100%。", "Enabled weights must sum to 100%."));
      return;
    }
    setSandboxPending(true);
    setSandboxFeedback(pickText(lang, "正在重算组合表现…", "Recalculating portfolio performance..."));
    const response = await requestBacktestApi("/api/user/backtest/portfolios/recalculate", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify({
        task_id: selectedTaskId,
        max_drawdown_pct: readNumber(selectedTask?.summary?.max_drawdown_pct) ?? readNumber(readObject(selectedTask?.config?.scoring)?.max_drawdown_pct),
        items: enabledItems.map((item) => ({
          candidate_id: item.candidateId,
          symbol: item.symbol,
          weight_pct: Number(item.weightPct),
          leverage: Number(item.leverage),
          enabled: item.enabled,
        })),
      }),
    });
    setSandboxPending(false);
    if (!response.ok) {
      setSandboxFeedback(response.message);
      return;
    }
    setSandboxResult(response.data as PortfolioRecalculateResponse);
    setSelectedCandidate(null);
    setSelectedPortfolio(null);
    setSandboxFeedback(pickText(lang, "组合已重算，图表已切换到沙盒结果。", "Portfolio recalculated; charts now show the sandbox result."));
  }

  return (
    <div className="space-y-6">
      <MartingaleRiskWarning lang={lang} compact />
      <section className="rounded-2xl border border-border bg-card p-6 shadow-sm">
        <div className="flex flex-col gap-4 lg:flex-row lg:items-end lg:justify-between">
          <div className="space-y-2">
            <p className="text-xs font-semibold uppercase tracking-[0.25em] text-muted-foreground">
              {pickText(lang, "马丁 Portfolio 回测台", "Martingale Portfolio Backtest Desk")}
            </p>
            <h1 className="text-3xl font-semibold tracking-tight">
              {pickText(lang, "两阶段回测控制台", "Two-stage backtest console")}
            </h1>
            <p className="max-w-3xl text-sm text-muted-foreground">
              {pickText(
                lang,
                "先做 K 线海选，再做成交级精测。输出每个币种 Top 10 与组合 Top 3。任务、候选与发布复核均来自后端 API，不展示静态假数据。",
                "Screen with candles first, then refine with trade-level replay. Outputs per-symbol Top 10 and portfolio Top 3. Tasks, candidates, and publish review come from backend APIs, not static mock data.",
              )}
            </p>
          </div>
          <div className="flex flex-wrap gap-2">
            {SURFACE_TAGS.map((tag) => (
              <span
                className="rounded-full border border-border bg-secondary/40 px-3 py-1 text-xs font-medium text-foreground"
                key={tag}
              >
                {tag}
              </span>
            ))}
          </div>
        </div>
      </section>

      {/* Overfitting risk banner */}
      {selectedSummary.overfitting_risk && (
        <div className="rounded-xl border border-amber-500/40 bg-amber-500/5 px-4 py-3 text-sm">
          <p className="font-semibold text-amber-700 dark:text-amber-300">
            {pickText(lang, "过拟合风险", "Overfitting risk")}
          </p>
          <p className="mt-1 text-muted-foreground">
            {pickText(
              lang,
              "该候选在训练段表现优秀，但验证或压力段表现显著下降，存在过拟合风险。",
              "This candidate performed well in training but degraded significantly in validation or stress windows, indicating overfitting risk.",
            )}
          </p>
        </div>
      )}

      {/* Portfolio pool note */}
      {portfolioPoolNote ? (
        <div className="rounded-xl border border-indigo-500/30 bg-indigo-500/5 px-4 py-3 text-sm">
          <p className="font-semibold">
            {pickText(lang, "组合候选池说明", "Portfolio pool note")}
          </p>
          <p className="mt-1 text-muted-foreground">
            {pickText(
              lang,
              "单策略 Top10 只展示自身满足回撤限制的策略；组合候选池还会保留正收益高弹性策略，最终组合仍会硬控最大回撤。",
              "Single-strategy Top10 only shows strategies meeting their own drawdown limits; the portfolio candidate pool also retains positive-return high-growth strategies. Final portfolios still enforce hard drawdown limits.",
            )}
          </p>
          <div className="mt-2 flex flex-wrap gap-x-4 gap-y-1 text-xs text-muted-foreground">
            {expandedUniverseSymbolCount != null && (
              <span>{pickText(lang, `扩展币种：${expandedUniverseSymbolCount}`, `Expanded symbols: ${expandedUniverseSymbolCount}`)}</span>
            )}
            {portfolioPoolCandidateCount != null && (
              <span>{pickText(lang, `候选池：${portfolioPoolCandidateCount}`, `Pool candidates: ${portfolioPoolCandidateCount}`)}</span>
            )}
            <span>{pickText(lang, `组合 Top${portfolioTopN}`, `Portfolio Top${portfolioTopN}`)}</span>
          </div>
        </div>
      ) : null}

      <div className="grid gap-6 xl:grid-cols-[minmax(0,1.1fr)_minmax(360px,0.9fr)]">
        <section className="rounded-2xl border border-border bg-card p-4 shadow-sm">
          <div className="mb-4 flex items-start justify-between gap-3">
            <div>
              <h2 className="text-lg font-semibold">{pickText(lang, "1. 创建回测任务", "1. Create backtest task")}</h2>
              <p className="text-sm text-muted-foreground">{pickText(lang, "向导适合自动搜索，专业模式适合精细控制。", "Wizard is for auto search; professional mode is for detailed control.")}</p>
            </div>
          </div>
          <div
            aria-label={pickText(lang, "回测配置模式", "Backtest configuration mode")}
            className="mb-4 flex flex-wrap gap-2"
            role="tablist"
          >
            <TabButton active={activeTab === "wizard"} controls="backtest-wizard-panel" id="backtest-wizard-tab" label={pickText(lang, "Wizard 模式", "Wizard mode")} onClick={() => setActiveTab("wizard")} />
            <TabButton active={activeTab === "professional"} controls="backtest-professional-panel" id="backtest-professional-tab" label={pickText(lang, "Professional Console", "Professional console")} onClick={() => setActiveTab("professional")} />
          </div>
          <div aria-labelledby={activeTabId} id={activePanelId} role="tabpanel">
            {activeTab === "wizard" ? (
              <BacktestWizard lang={lang} onTaskCreated={() => refreshTasks({ selectLatest: true })} />
            ) : (
              <BacktestProfessionalPanel lang={lang} onTaskCreated={() => refreshTasks({ selectLatest: true })} />
            )}
          </div>
        </section>

        <section className="rounded-2xl border border-border bg-card p-4 shadow-sm">
          <div className="mb-4">
            <h2 className="text-lg font-semibold">{pickText(lang, "2. 任务列表与进度", "2. Tasks and progress")}</h2>
            <p className="text-sm text-muted-foreground">{pickText(lang, "选择任务后，下方会全宽展示候选、组合、图表与交易明细。", "Select a task; candidates, portfolios, charts, and trade previews appear full-width below.")}</p>
          </div>
          <BacktestTaskList
            lang={lang}
            loading={loading}
            onRefresh={refreshTasks}
            onSelectTask={setSelectedTaskId}
            selectedTaskId={selectedTaskId}
            tasks={tasks}
          />
        </section>
      </div>

      <section className="rounded-2xl border border-border bg-card p-4 shadow-sm">
        <h2 className="mb-3 text-lg font-semibold">{pickText(lang, "3. 候选与自动组合结果", "3. Candidates and auto portfolios")}</h2>
        <BacktestResultTable
          candidates={candidates}
          lang={lang}
          onAddToBasket={(candidate) => {
            setSelectedCandidate(candidate);
            addCandidateToBasket(candidate);
            setPublishOpen(true);
            setFeedback(pickText(lang, `已加入组合篮子：${candidate.symbol}`, `Added to portfolio basket: ${candidate.symbol}`));
          }}
          selectedId={selectedCandidate?.id ?? ""}
          selectedTaskStatus={selectedTask?.rawStatus ?? ""}
          taskName={selectedTaskName}
          portfolioTop3={portfolioTop3FromTask(selectedTask)}
          portfolioTopN={portfolioTopN}
          selectedPortfolioId={selectedPortfolio?.portfolio_id ?? ""}
          onSelect={(candidate) => {
            const full = candidates.find((c) => c.id === candidate.id) ?? null;
            setSelectedCandidate(full);
            setSelectedPortfolio(null);
          }}
          onSelectPortfolio={(portfolio) => {
            setSelectedPortfolio(portfolio);
            setSelectedCandidate(null);
            setSandboxResult(null);
          }}
          onEditPortfolio={(portfolio) => editPortfolioSandbox(portfolio)}
        />
      </section>

      <section className="rounded-2xl border border-border bg-card p-4 shadow-sm">
        <h2 className="mb-3 text-lg font-semibold">{pickText(lang, "4. 图表与交易明细", "4. Charts and trade details")}</h2>
        <p className="mb-4 text-sm text-muted-foreground">{selectedDetailTitle}</p>
        <BacktestCharts summary={selectedSummary} />
      </section>

      {hasSegmentData(selectedSummary) && (
        <section className="rounded-2xl border border-border bg-card p-4 shadow-sm">
          <h2 className="mb-3 text-lg font-semibold">{pickText(lang, "5. 分段表现对比", "5. Segment performance")}</h2>
          <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
            <SegmentBlock label={pickText(lang, "训练", "Train")} value={selectedSummary.train_return_pct} />
            <SegmentBlock label={pickText(lang, "验证", "Validate")} value={selectedSummary.validate_return_pct} />
            <SegmentBlock label={pickText(lang, "测试", "Test")} value={selectedSummary.test_return_pct} />
            <SegmentBlock label={pickText(lang, "压力", "Stress")} value={selectedSummary.stress_return_pct} />
          </div>
        </section>
      )}

      <section className="rounded-2xl border border-border bg-card p-4 shadow-sm">
        <button className="flex w-full items-center justify-between gap-3 text-left" onClick={() => setSandboxOpen((open) => !open)} type="button">
          <span>
            <span className="block text-lg font-semibold">{pickText(lang, "6. 组合沙盒", "6. Portfolio sandbox")}</span>
            <span className="block text-sm text-muted-foreground">{pickText(lang, "从自动组合点编辑，或把候选加入沙盒，调整权重/杠杆后重算。", "Edit an auto portfolio or add candidates, then adjust weights/leverage and recalculate.")}</span>
          </span>
          <span className="rounded-full border border-border px-3 py-1 text-xs">{sandboxOpen ? pickText(lang, "收起", "Collapse") : pickText(lang, "展开", "Expand")}</span>
        </button>
        {sandboxOpen ? (
          <div className="mt-4 space-y-3">
            {selectedCandidate ? (
              <div className="flex flex-wrap gap-2">
                <button className="rounded-full bg-secondary px-3 py-1 text-xs font-medium" onClick={() => addCandidateToSandbox(selectedCandidate)} type="button">
                  {pickText(lang, `加入沙盒：${selectedCandidate.symbol}`, `Add to sandbox: ${selectedCandidate.symbol}`)}
                </button>
              </div>
            ) : null}
            {sandboxItems.length > 0 ? (
              <div className="space-y-2">
                {sandboxItems.map((item) => (
                  <div className="grid gap-2 rounded-lg border border-border p-3 text-xs md:grid-cols-[minmax(0,1fr)_120px_120px_90px]" key={item.localId}>
                    <div>
                      <p className="font-medium">{item.symbol}</p>
                      <p className="break-all text-muted-foreground">{item.direction} · {item.leverage}x · {item.candidateId}</p>
                    </div>
                    <label className="space-y-1"><span className="text-[10px] uppercase text-muted-foreground">权重%</span><input className="w-full rounded border border-border bg-background px-2 py-1" value={item.weightPct} onChange={(event) => setSandboxItems((current) => current.map((row) => row.localId === item.localId ? { ...row, weightPct: event.currentTarget.value } : row))} /></label>
                    <label className="space-y-1"><span className="text-[10px] uppercase text-muted-foreground">杠杆</span><input className="w-full rounded border border-border bg-background px-2 py-1" value={item.leverage} onChange={(event) => setSandboxItems((current) => current.map((row) => row.localId === item.localId ? { ...row, leverage: event.currentTarget.value } : row))} /></label>
                    <button className="rounded border border-border px-2 py-1" onClick={() => setSandboxItems((current) => current.filter((row) => row.localId !== item.localId))} type="button">{pickText(lang, "移除", "Remove")}</button>
                  </div>
                ))}
                <div className="flex flex-wrap items-center gap-3">
                  <button className="rounded-full bg-primary px-4 py-2 text-sm font-medium text-primary-foreground disabled:opacity-60" disabled={sandboxPending} onClick={() => void recalculateSandbox()} type="button">
                    {sandboxPending ? pickText(lang, "重算中…", "Recalculating...") : pickText(lang, "重新计算组合表现", "Recalculate portfolio")}
                  </button>
                  <button className="rounded-full border-2 border-amber-500 bg-amber-500/10 px-5 py-2 text-sm font-semibold text-amber-700 shadow-sm transition-colors hover:bg-amber-500/20 dark:text-amber-200" onClick={() => { setBasketItems(sandboxItems); setPublishOpen(true); }} type="button">
                    {pickText(lang, "用作发布篮子", "Use as publish basket")}
                  </button>
                  {sandboxResult ? <span className="text-sm text-muted-foreground">{pickText(lang, `年化 ${sandboxResult.annualized_return_pct?.toFixed(2) ?? "—"}% · 回撤 ${sandboxResult.max_drawdown_pct.toFixed(2)}%`, `Annualized ${sandboxResult.annualized_return_pct?.toFixed(2) ?? "—"}% · DD ${sandboxResult.max_drawdown_pct.toFixed(2)}%`)}</span> : null}
                </div>
              </div>
            ) : <p className="text-sm text-muted-foreground">{pickText(lang, "暂无沙盒成员。", "No sandbox members yet.")}</p>}
            <p className="text-sm text-muted-foreground" aria-live="polite">{sandboxFeedback}</p>
          </div>
        ) : null}
      </section>

      <section className="rounded-2xl border border-border bg-card p-4 shadow-sm">
        <button className="flex w-full items-center justify-between gap-3 text-left" onClick={() => setPublishOpen((open) => !open)} type="button">
          <span>
            <span className="block text-lg font-semibold">{pickText(lang, "7. 发布复核", "7. Publish review")}</span>
            <span className="block text-sm text-muted-foreground">{pickText(lang, "发布前确认权重、杠杆、风险摘要和实盘参数映射。", "Confirm weights, leverage, risk summary, and live parameter mapping before publishing.")}</span>
          </span>
          <span className="rounded-full border border-border px-3 py-1 text-xs">{publishOpen ? pickText(lang, "收起", "Collapse") : pickText(lang, "展开", "Expand")}</span>
        </button>
        {publishOpen ? (
          <div className="mt-4">
            <PortfolioCandidateReview
              basketItems={basketItems}
              candidate={selectedCandidate}
              lang={lang}
              locale={locale}
              onPublish={publishPortfolio}
              onRemove={(localId) => setBasketItems((current) => current.filter((item) => item.localId !== localId))}
              onUpdate={(localId, patch) => setBasketItems((current) => current.map((item) => (
                item.localId === localId ? { ...item, ...patch } : item
              )))}
            />
          </div>
        ) : null}
      </section>

      <p aria-live="polite" className="text-sm text-muted-foreground">
        {feedback}
      </p>
    </div>
  );
}

function SegmentBlock({ label, value }: { label: string; value: number | undefined }) {
  if (value == null) return null;
  const pct = value.toFixed(2);
  return (
    <div className="rounded-lg border border-border bg-background p-3 text-center">
      <p className="text-xs text-muted-foreground">{label}</p>
      <p className={`mt-1 text-sm font-semibold ${value >= 0 ? "text-emerald-600" : "text-red-600"}`}>
        {pct}%
      </p>
    </div>
  );
}

function hasSegmentData(s: MartingaleBacktestCandidateSummary): boolean {
  return s.train_return_pct != null || s.validate_return_pct != null || s.test_return_pct != null || s.stress_return_pct != null;
}

function normalizeTask(task: ApiTask, lang: UiLanguage): BacktestTask {
  const taskId = task.task_id ?? "";
  const config = task.config ?? {};
  const summary = task.summary ?? {};
  const taskSymbols = readTaskSymbols(config);
  const symbols = taskSymbols.join("/") || pickText(lang, "未选择 symbol", "No symbols");
  const stage = readString(summary.stage_label) || readString(summary.stage) || readString(summary.current_stage) || statusStage(task.status, lang);
  const progress = readProgress(summary, taskSymbols.length, lang);
  return {
    id: taskId,
    name: `${symbols} · ${task.strategy_type ?? "martingale_grid"}`,
    status: humanizeStatus(task.status, lang),
    rawStatus: task.status ?? "",
    progress,
    stage,
    updatedAt: formatDate(task.updated_at || task.created_at),
    summary: isRecord(task.summary) ? task.summary : null,
  };
}

function normalizeCandidate(candidate: ApiCandidate, lang: UiLanguage): BacktestCandidate {
  const summary = candidate.summary ?? {};
  const config = candidate.config ?? {};
  const portfolio = readObject(config.portfolio_config) ?? config;
  const strategies = readArray(readObject(portfolio)?.strategies);
  const firstStrategy = readObject(strategies[0]) ?? {};
  const symbols = readString(summary.symbol)
    || uniqueStrings(strategies.map((strategy) => readObject(strategy)?.symbol)).join("/")
    || readString(firstStrategy.symbol)
    || "—";
  const markets = uniqueStrings(strategies.map((strategy) => readObject(strategy)?.market)).join("/")
    || readString(firstStrategy.market)
    || "—";
  const directions = uniqueStrings(strategies.map((strategy) => readObject(strategy)?.direction)).join("+")
    || readString(firstStrategy.direction)
    || "—";
  const score = readNumber(summary.score);
  const drawdown = readNumber(summary.max_drawdown_pct) ?? readNumber(summary.drawdown_pct);
  const returnPct = readNumber(summary.total_return_pct);
  const tradeCount = readNumber(summary.trade_count);
  const spacing = readObject(firstStrategy.spacing);
  const sizing = readObject(firstStrategy.sizing);
  const takeProfit = readObject(firstStrategy.take_profit);
  return {
    id: candidate.candidate_id ?? "",
    symbol: symbols,
    market: markets,
    direction: directions,
    searchMode: readString(summary.result_mode) || readString(summary.search_mode) || readString(summary.source) || pickText(lang, "Worker 候选", "Worker candidate"),
    score: formatScore(score),
    drawdown: drawdown == null ? "—" : `${drawdown.toFixed(2)}%`,
    returnPct: returnPct == null ? "—" : `${returnPct.toFixed(2)}%`,
    tradeCount: tradeCount == null ? "—" : String(Math.round(tradeCount)),
    parameters: describeCandidateParameters(spacing, sizing, takeProfit, lang),
    decision: humanizeCandidateDecision(candidate.status, summary, lang),
    rank: candidate.rank,
    summary: {
      score: readNumber(summary.score) ?? undefined,
      total_return_pct: readNumber(summary.total_return_pct) ?? undefined,
      max_drawdown: readNumber(summary.max_drawdown_pct) != null ? (readNumber(summary.max_drawdown_pct)! / 100) : undefined,
      trade_count: readNumber(summary.trade_count) ?? undefined,
      stop_count: readNumber(summary.stop_count) ?? undefined,
      max_capital_used_quote: readNumber(summary.max_capital_used_quote) ?? undefined,
      survival_passed: typeof summary.survival_passed === "boolean" ? summary.survival_passed : undefined,
      rejection_reasons: Array.isArray(summary.rejection_reasons) ? summary.rejection_reasons as string[] : undefined,
      stress_window_scores: typeof summary.stress_window_scores === "object" && summary.stress_window_scores != null ? summary.stress_window_scores as Record<string, number> : undefined,
      equity_curve: Array.isArray(summary.equity_curve) ? summary.equity_curve as { ts: number; equity: number; drawdown?: number }[] : undefined,
      drawdown_curve: Array.isArray(summary.drawdown_curve) ? summary.drawdown_curve as { ts: number; equity: number; drawdown?: number }[] : undefined,
      artifact_path: readString(summary.artifact_path) || undefined,
      stop_loss_events: Array.isArray(summary.stop_loss_events) ? summary.stop_loss_events as { ts: number; symbol: string; reason: string; loss_pct: number }[] : undefined,
      train_return_pct: readNumber(summary.train_return_pct) ?? undefined,
      validate_return_pct: readNumber(summary.validate_return_pct) ?? undefined,
      test_return_pct: readNumber(summary.test_return_pct) ?? undefined,
      stress_return_pct: readNumber(summary.stress_return_pct) ?? undefined,
      overfitting_risk: typeof summary.overfitting_risk === "boolean" ? summary.overfitting_risk : undefined,
      data_quality_score: readNumber(summary.data_quality_score) ?? undefined,
      recommended_weight_pct: readNumber(summary.recommended_weight_pct) ?? undefined,
      recommended_leverage: readNumber(summary.recommended_leverage) ?? undefined,
      parameter_rank_for_symbol: readNumber(summary.parameter_rank_for_symbol) ?? undefined,
      risk_profile: readString(summary.risk_profile) || undefined,
      risk_summary_human: readString(summary.risk_summary_human) || undefined,
      portfolio_group_key: readString(summary.portfolio_group_key) || undefined,
      market: readString(summary.market) || undefined,
      publishable: typeof summary.publishable === "boolean" ? summary.publishable : undefined,
      candidate_warning: readString(summary.candidate_warning) || undefined,
      max_leverage_used: readNumber(summary.max_leverage_used) ?? undefined,
    },
    rawConfig: config,
  };
}



function formatScore(value: number | null) {
  if (value == null) return "—";
  if (Math.abs(value) >= 1_000_000) return value > 0 ? "生存通过" : "未通过";
  return value.toFixed(2);
}

function describeCandidateParameters(
  spacing: Record<string, unknown> | null,
  sizing: Record<string, unknown> | null,
  takeProfit: Record<string, unknown> | null,
  lang: UiLanguage,
) {
  const fixed = readObject(spacing?.fixed_percent);
  const multiplier = readObject(sizing?.multiplier);
  const percent = readObject(takeProfit?.percent);
  const spacingPct = readNumber(fixed?.step_bps);
  const firstOrder = readNumber(multiplier?.first_order_quote);
  const orderMultiplier = readNumber(multiplier?.multiplier);
  const maxLegs = readNumber(multiplier?.max_legs);
  const tp = readNumber(percent?.bps);
  const parts = [
    spacingPct == null ? null : pickText(lang, `间隔 ${(spacingPct / 100).toFixed(2)}%`, `Step ${(spacingPct / 100).toFixed(2)}%`),
    firstOrder == null ? null : pickText(lang, `首单 ${firstOrder}U`, `Base ${firstOrder}U`),
    orderMultiplier == null ? null : pickText(lang, `倍投 ${orderMultiplier}x`, `Scale ${orderMultiplier}x`),
    maxLegs == null ? null : pickText(lang, `${maxLegs} 层`, `${maxLegs} legs`),
    tp == null ? null : pickText(lang, `止盈 ${(tp / 100).toFixed(2)}%`, `TP ${(tp / 100).toFixed(2)}%`),
  ].filter(Boolean);
  return parts.join(" · ") || "—";
}

function TabButton({
  active,
  controls,
  id,
  label,
  onClick,
}: {
  active: boolean;
  controls: string;
  id: string;
  label: string;
  onClick: () => void;
}) {
  return (
    <button
      aria-controls={controls}
      aria-selected={active}
      className={cn(
        "rounded-full border px-4 py-2 text-sm font-medium transition-colors",
        active
          ? "border-primary bg-primary text-primary-foreground"
          : "border-border bg-background text-foreground hover:bg-secondary/50",
      )}
      id={id}
      onClick={onClick}
      role="tab"
      tabIndex={active ? 0 : -1}
      type="button"
    >
      {label}
    </button>
  );
}

function readObject(value: unknown): Record<string, unknown> | null {
  return value && typeof value === "object" && !Array.isArray(value) ? value as Record<string, unknown> : null;
}

function readArray(value: unknown): unknown[] {
  return Array.isArray(value) ? value : [];
}

function readString(value: unknown): string {
  return typeof value === "string" ? value : "";
}

function readStringArray(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((entry): entry is string => typeof entry === "string") : [];
}

function readNumber(value: unknown): number | null {
  if (typeof value === "number" && Number.isFinite(value)) {
    return value;
  }
  if (typeof value === "string" && value.trim()) {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : null;
  }
  return null;
}

function readProgress(summary: Record<string, unknown>, fallbackTotalSymbols: number, lang: UiLanguage) {
  const progress = readNumber(summary.progress_pct) ?? readNumber(summary.progress);
  const evaluatedCandidates = readNumber(summary.evaluated_candidates) ?? readNumber(summary.candidates_evaluated);
  const completedSymbolsValue = summary.completed_symbols;
  const completedSymbols = Array.isArray(completedSymbolsValue)
    ? completedSymbolsValue.length
    : readNumber(completedSymbolsValue) ?? readNumber(summary.completed_symbol_count);
  const totalSymbols = readNumber(summary.total_symbols) ?? readNumber(summary.symbol_count) ?? fallbackTotalSymbols;
  const parts = [
    progress == null ? null : `${Math.round(progress)}%`,
    evaluatedCandidates == null ? null : pickText(lang, `已评估候选 ${evaluatedCandidates}`, `${evaluatedCandidates} evaluated candidates`),
    completedSymbols == null ? null : pickText(lang, `已完成币种 ${completedSymbols}/${totalSymbols || "?"}`, `${completedSymbols}/${totalSymbols || "?"} completed symbols`),
  ].filter(Boolean);
  return parts.join(" · ") || "—";
}

function readTaskSymbols(config: Record<string, unknown>) {
  const directSymbols = readStringArray(config.symbols);
  if (directSymbols.length > 0) return directSymbols;
  const symbolPool = readObject(config.symbol_pool);
  return readStringArray(symbolPool?.whitelist);
}

function isTerminalTaskStatus(status: string | undefined) {
  return ["succeeded", "completed", "failed", "cancelled", "canceled"].includes(status ?? "");
}

function uniqueStrings(values: unknown[]) {
  return Array.from(new Set(values.map(readString).filter(Boolean)));
}

function statusStage(status: string | undefined, lang: UiLanguage) {
  switch (status) {
    case "queued":
      return pickText(lang, "等待 Worker", "Waiting for worker");
    case "running":
      return pickText(lang, "Worker 运行中", "Worker running");
    case "paused":
      return pickText(lang, "已暂停", "Paused");
    case "succeeded":
      return pickText(lang, "候选已生成", "Candidates generated");
    case "failed":
      return pickText(lang, "任务失败", "Task failed");
    case "cancelled":
      return pickText(lang, "已取消", "Cancelled");
    default:
      return pickText(lang, "等待状态更新", "Waiting for status");
  }
}

function humanizeStatus(status: string | undefined, lang: UiLanguage) {
  const labels: Record<string, [string, string]> = {
    queued: ["排队中", "Queued"],
    running: ["运行中", "Running"],
    paused: ["已暂停", "Paused"],
    succeeded: ["已完成", "Completed"],
    failed: ["失败", "Failed"],
    cancelled: ["已取消", "Cancelled"],
  };
  const label = status ? labels[status] : undefined;
  return label ? pickText(lang, label[0], label[1]) : status || "—";
}

function humanizeCandidateDecision(
  status: string | undefined,
  summary: Record<string, unknown>,
  lang: UiLanguage,
) {
  if (readString(summary.rejection_reason)) {
    return pickText(lang, `淘汰：${readString(summary.rejection_reason)}`, `Rejected: ${readString(summary.rejection_reason)}`);
  }
  if (status === "ready") {
    return pickText(lang, "可生成发布风险摘要", "Ready for publish intent");
  }
  return status || pickText(lang, "等待精测", "Waiting for refinement");
}

function formatDate(value: unknown) {
  if (typeof value !== "string" || !value) {
    return "—";
  }
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }
  return date.toLocaleString();
}

function isRecord(value: unknown): value is Record<string, unknown> {
  return typeof value === "object" && value !== null && !Array.isArray(value);
}


function portfolioSandboxSummaryForCharts(result: PortfolioRecalculateResponse): MartingaleBacktestCandidateSummary {
  return {
    annualized_return_pct: result.annualized_return_pct ?? null,
    drawdown_curve: result.drawdown_curve as MartingaleBacktestCandidateSummary["drawdown_curve"],
    equity_curve: result.equity_curve as MartingaleBacktestCandidateSummary["equity_curve"],
    max_drawdown_pct: result.max_drawdown_pct,
    member_count: result.member_count,
    members: result.members as MartingaleBacktestCandidateSummary["members"],
    portfolio_id: result.portfolio_id,
    risk_summary_human: `沙盒组合包含 ${result.member_count} 个策略，${result.satisfies_drawdown_limit ? "满足" : "超过"}回撤限制`,
    total_return_pct: result.total_return_pct,
    trade_count: result.trade_count,
    trades_preview: result.trades_preview as MartingaleBacktestCandidateSummary["trades_preview"],
    return_drawdown_ratio: result.return_drawdown_ratio ?? null,
  };
}

function portfolioToSandboxResult(portfolio: PortfolioTop3Row): PortfolioRecalculateResponse {
  return {
    portfolio_id: portfolio.portfolio_id,
    member_count: portfolio.member_count,
    total_return_pct: portfolio.total_return_pct || portfolio.return_pct,
    annualized_return_pct: portfolio.annualized_return_pct ?? null,
    max_drawdown_pct: portfolio.max_drawdown_pct,
    return_drawdown_ratio: null,
    trade_count: portfolio.trade_count,
    satisfies_drawdown_limit: true,
    concentration_warnings: [],
    members: portfolio.members,
    equity_curve: portfolio.equity_curve ?? [],
    drawdown_curve: portfolio.drawdown_curve ?? [],
    trades_preview: portfolio.trades_preview ?? [],
  };
}

function portfolioSummaryForCharts(portfolio: PortfolioTop3Row): MartingaleBacktestCandidateSummary {
  return {
    annualized_return_pct: portfolio.annualized_return_pct ?? null,
    drawdown_curve: portfolio.drawdown_curve as MartingaleBacktestCandidateSummary["drawdown_curve"],
    equity_curve: portfolio.equity_curve as MartingaleBacktestCandidateSummary["equity_curve"],
    max_drawdown_pct: portfolio.max_drawdown_pct,
    member_count: portfolio.member_count,
    members: portfolio.members,
    portfolio_id: portfolio.portfolio_id,
    portfolio_rank: portfolio.portfolio_rank,
    risk_summary_human: `组合包含 ${portfolio.member_count} 个策略，最大单币权重 ${Math.max(...portfolio.members.map((member) => member.allocation_pct), 0).toFixed(1)}%`,
    score: portfolio.score,
    total_return_pct: portfolio.return_pct || portfolio.total_return_pct,
    trade_count: portfolio.trade_count,
    trades_preview: portfolio.trades_preview as MartingaleBacktestCandidateSummary["trades_preview"],
  };
}

function portfolioTop3FromTask(task: BacktestTask | null): PortfolioTop3Row[] {
  // Show only the curated Top3 portfolios by default; Top10 remains a backend artifact for compatibility.
  const rows = task?.summary?.portfolio_top3 ?? task?.summary?.portfolio_top10;
  if (!Array.isArray(rows)) return [];
  return rows.map((row: unknown) => {
    const record = isRecord(row) ? row : {};
    const rawMembers = Array.isArray(record.members) ? record.members : [];
    const members: PortfolioMember[] = rawMembers.map((m: unknown) => {
      const mr = isRecord(m) ? m : {};
      return {
        candidate_id: readString(mr.candidate_id) || "",
        symbol: readString(mr.symbol) || "",
        direction: readString(mr.direction) || "",
        allocation_pct: readNumber(mr.allocation_pct) ?? 0,
        return_pct: readNumber(mr.return_pct) ?? 0,
        max_drawdown_pct: readNumber(mr.max_drawdown_pct) ?? 0,
        annualized_return_pct: readNumber(mr.annualized_return_pct) ?? null,
        score: readNumber(mr.score) ?? 0,
        trade_count: readNumber(mr.trade_count) ?? 0,
        leverage: readNumber(mr.leverage) ?? null,
      };
    });
    return {
      portfolio_id: readString(record.portfolio_id) || "",
      portfolio_rank: readNumber(record.portfolio_rank) ?? 0,
      member_count: readNumber(record.member_count) ?? members.length,
      members,
      total_return_pct: readNumber(record.total_return_pct) ?? readNumber(record.return_pct) ?? 0,
      return_pct: readNumber(record.return_pct) ?? 0,
      max_drawdown_pct: readNumber(record.max_drawdown_pct) ?? 0,
      annualized_return_pct: readNumber(record.annualized_return_pct) ?? null,
      score: readNumber(record.score) ?? 0,
      trade_count: readNumber(record.trade_count) ?? 0,
      equity_curve: Array.isArray(record.equity_curve) ? record.equity_curve : undefined,
      drawdown_curve: Array.isArray(record.drawdown_curve) ? record.drawdown_curve : undefined,
      trades_preview: Array.isArray(record.trades_preview) ? record.trades_preview : undefined,
      eligible_candidate_count: readNumber(record.eligible_candidate_count) ?? null,
    };
  });
}
