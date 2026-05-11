"use client";

import { useCallback, useEffect, useMemo, useState } from "react";
import { BacktestCharts } from "@/components/backtest/backtest-charts";
import { BacktestProfessionalPanel } from "@/components/backtest/backtest-professional-panel";
import { BacktestResultTable } from "@/components/backtest/backtest-result-table";
import { BacktestTaskList } from "@/components/backtest/backtest-task-list";
import { BacktestWizard } from "@/components/backtest/backtest-wizard";
import { PortfolioCandidateReview } from "@/components/backtest/portfolio-candidate-review";
import { requestBacktestApi } from "@/components/backtest/request-client";
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
  progress: string;
  stage: string;
  updatedAt: string;
};

type BacktestCandidate = {
  id: string;
  symbol: string;
  market: string;
  direction: string;
  searchMode: string;
  score: string;
  drawdown: string;
  decision: string;
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
  const [feedback, setFeedback] = useState("");
  const [loading, setLoading] = useState(true);
  const activePanelId = activeTab === "wizard" ? "backtest-wizard-panel" : "backtest-professional-panel";
  const activeTabId = activeTab === "wizard" ? "backtest-wizard-tab" : "backtest-professional-tab";

  const refreshTasks = useCallback(async () => {
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
    setSelectedTaskId((current) => current || firstTaskId);
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
  }, [lang]);

  useEffect(() => {
    void refreshTasks();
  }, [refreshTasks]);

  useEffect(() => {
    void refreshCandidates(selectedTaskId);
  }, [refreshCandidates, selectedTaskId]);

  const selectedTaskName = useMemo(
    () => tasks.find((task) => task.id === selectedTaskId)?.name ?? selectedTaskId,
    [selectedTaskId, tasks],
  );

  return (
    <div className="space-y-6">
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
                "先做 K 线海选，再做成交级精测。任务、候选与发布复核均来自后端 API，不展示静态假数据。",
                "Screen with candles first, then refine with trade-level replay. Tasks, candidates, and publish review come from backend APIs, not static mock data.",
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

      <div className="grid gap-6 xl:grid-cols-[minmax(0,1.6fr)_minmax(320px,0.9fr)]">
        <div className="space-y-6">
          <section className="rounded-2xl border border-border bg-card p-4 shadow-sm">
            <div
              aria-label={pickText(lang, "回测配置模式", "Backtest configuration mode")}
              className="mb-4 flex flex-wrap gap-2"
              role="tablist"
            >
              <TabButton
                active={activeTab === "wizard"}
                controls="backtest-wizard-panel"
                id="backtest-wizard-tab"
                label={pickText(lang, "Wizard 模式", "Wizard mode")}
                onClick={() => setActiveTab("wizard")}
              />
              <TabButton
                active={activeTab === "professional"}
                controls="backtest-professional-panel"
                id="backtest-professional-tab"
                label={pickText(lang, "Professional Console", "Professional console")}
                onClick={() => setActiveTab("professional")}
              />
            </div>

            <div aria-labelledby={activeTabId} id={activePanelId} role="tabpanel">
              {activeTab === "wizard" ? (
                <BacktestWizard lang={lang} onTaskCreated={refreshTasks} />
              ) : (
                <BacktestProfessionalPanel lang={lang} onTaskCreated={refreshTasks} />
              )}
            </div>
          </section>

          <BacktestCharts lang={lang} />
          <BacktestResultTable
            candidates={candidates}
            lang={lang}
            selectedId={selectedCandidate?.id ?? ""}
            taskName={selectedTaskName}
            onSelect={(candidate) => setSelectedCandidate(candidate)}
          />
        </div>

        <div className="space-y-6">
          <BacktestTaskList
            lang={lang}
            loading={loading}
            onRefresh={refreshTasks}
            onSelectTask={setSelectedTaskId}
            selectedTaskId={selectedTaskId}
            tasks={tasks}
          />
          <PortfolioCandidateReview
            candidate={selectedCandidate}
            lang={lang}
            locale={locale}
          />
          <p aria-live="polite" className="text-sm text-muted-foreground">
            {feedback}
          </p>
        </div>
      </div>
    </div>
  );
}

function normalizeTask(task: ApiTask, lang: UiLanguage): BacktestTask {
  const taskId = task.task_id ?? "";
  const config = task.config ?? {};
  const summary = task.summary ?? {};
  const symbols = readStringArray(config.symbols).join("/") || pickText(lang, "未选择 symbol", "No symbols");
  const stage = readString(summary.stage) || readString(summary.current_stage) || statusStage(task.status, lang);
  const progress = readProgress(summary);
  return {
    id: taskId,
    name: `${symbols} · ${task.strategy_type ?? "martingale_grid"}`,
    status: humanizeStatus(task.status, lang),
    progress,
    stage,
    updatedAt: formatDate(task.updated_at || task.created_at),
  };
}

function normalizeCandidate(candidate: ApiCandidate, lang: UiLanguage): BacktestCandidate {
  const summary = candidate.summary ?? {};
  const config = candidate.config ?? {};
  const portfolio = readObject(config.portfolio_config) ?? config;
  const strategies = readArray(readObject(portfolio)?.strategies);
  const firstStrategy = readObject(strategies[0]) ?? {};
  const symbols = uniqueStrings(strategies.map((strategy) => readObject(strategy)?.symbol)).join("/")
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
  return {
    id: candidate.candidate_id ?? "",
    symbol: symbols,
    market: markets,
    direction: directions,
    searchMode: readString(summary.search_mode) || readString(summary.source) || pickText(lang, "Worker 候选", "Worker candidate"),
    score: score == null ? "—" : score.toFixed(2),
    drawdown: drawdown == null ? "—" : `${drawdown.toFixed(2)}%`,
    decision: humanizeCandidateDecision(candidate.status, summary, lang),
  };
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

function readProgress(summary: Record<string, unknown>) {
  const progress = readNumber(summary.progress_pct) ?? readNumber(summary.progress);
  return progress == null ? "—" : `${Math.round(progress)}%`;
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
