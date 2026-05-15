import { DataTable, type DataTableRow } from "@/components/ui/table";
import type { MartingaleBacktestCandidateSummary } from "@/lib/api-types";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";

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
};

type DynamicResultSummary = MartingaleBacktestCandidateSummary & {
  return_drawdown_ratio?: number;
  profit_drawdown_ratio?: number;
  rebalance_count?: number;
  forced_exit_count?: number;
  max_drawdown_limit_passed?: boolean;
  live_recommended?: boolean;
  can_recommend_live?: boolean;
  annualized_return_pct?: number;
  max_drawdown_pct?: number;
  stop_loss_count?: number;
  fee_quote?: number;
  slippage_quote?: number;
  long_weight_pct?: number;
  short_weight_pct?: number;
  actual_long_weight_pct?: number;
  actual_short_weight_pct?: number;
  discarded_symbols_from_portfolio_top10?: string[];
  portfolio_top10_discarded_symbols?: string[];
  cost_summary?: {
    fee_quote?: number;
    slippage_quote?: number;
    stop_loss_quote?: number;
    forced_exit_quote?: number;
    rebalance_count?: number;
    forced_exit_count?: number;
  };
};

type BacktestResultTableProps = {
  candidates: BacktestCandidate[];
  lang: UiLanguage;
  onAddToBasket?: (candidate: BacktestCandidate) => void;
  onSelect?: (candidate: BacktestCandidate) => void;
  selectedId?: string;
  selectedTaskStatus?: string;
  taskName?: string;
};

export function BacktestResultTable({
  candidates,
  lang,
  onAddToBasket,
  onSelect,
  selectedId,
  selectedTaskStatus,
  taskName,
}: BacktestResultTableProps) {
  const groupedCandidates = groupCandidatesBySymbol(candidates);
  const isSuccessfulWithoutCandidates = candidates.length === 0 && ["succeeded", "completed"].includes(selectedTaskStatus ?? "");

  return (
    <section className="space-y-3 rounded-2xl border border-border bg-card p-4 shadow-sm">
      <div className="flex items-center justify-between gap-3">
        <div>
          <h2 className="text-lg font-semibold">{pickText(lang, "候选结果表", "Candidate result table")}</h2>
          <p className="text-sm text-muted-foreground">
            {taskName
              ? pickText(lang, `当前任务：${taskName}`, `Current task: ${taskName}`)
              : pickText(lang, "暂无回测任务，选择币种后开始自动搜索 Top 10", "No backtest tasks yet; select symbols to start automatic Top 10 search.")}
          </p>
        </div>
        <code className="rounded bg-secondary/50 px-3 py-1 text-xs">GET /api/user/backtest/tasks/:id/candidates</code>
      </div>

      {groupedCandidates.length === 0 ? (
        <DataTable
          caption={pickText(lang, "每个币种 Top 10", "Per-symbol Top 10")}
          columns={candidateColumns(lang)}
          emptyMessage={isSuccessfulWithoutCandidates
            ? pickText(lang, "回测完成但没有可用候选，请放宽风控或参数范围后重试。", "Backtest completed but no usable candidates were found; relax risk rules or parameter ranges and retry.")
            : pickText(lang, "暂无回测任务，选择币种后开始自动搜索 Top 10", "No backtest tasks yet; select symbols to start automatic Top 10 search.")}
          rows={[]}
        />
      ) : (
        groupedCandidates.map((group) => (
          <div className="space-y-2" key={group.symbol}>
            <div>
              <h3 className="text-base font-semibold">{pickText(lang, `每个币种 Top 10 · ${group.symbol}`, `Per-symbol Top 10 · ${group.symbol}`)}</h3>
              <p className="text-xs text-muted-foreground">{pickText(lang, "按参数排名挑选每个币种最优候选。", "Sorted by parameter rank for each symbol.")}</p>
            </div>
            <DataTable
              caption={pickText(lang, "每个币种 Top 10", "Per-symbol Top 10")}
              columns={candidateColumns(lang)}
              emptyMessage={pickText(lang, "暂无候选结果；请等待 Worker 完成海选和精测。", "No candidates yet; wait for the worker to finish screening and refinement.")}
              rows={group.candidates.map((candidate) => candidateRow(candidate, lang, selectedId, onSelect, onAddToBasket))}
            />
            <DiscardedSymbolsNotice candidates={group.candidates} lang={lang} />
          </div>
        ))
      )}
    </section>
  );
}

function groupCandidatesBySymbol(candidates: BacktestCandidate[]) {
  const groups = new Map<string, BacktestCandidate[]>();
  for (const candidate of candidates) {
    const symbol = candidate.summary?.symbol || candidate.symbol || "—";
    groups.set(symbol, [...(groups.get(symbol) ?? []), candidate]);
  }
  return Array.from(groups.entries()).map(([symbol, groupCandidates]) => ({
    symbol,
    candidates: [...groupCandidates]
      .sort((left, right) => candidateRank(left) - candidateRank(right))
      .slice(0, 10),
  }));
}

function candidateRank(candidate: BacktestCandidate) {
  return candidate.summary?.parameter_rank_for_symbol ?? candidate.rank ?? Number.POSITIVE_INFINITY;
}

function candidateColumns(lang: UiLanguage) {
  return [
    { key: "symbol", label: "Symbol" },
    { key: "parameterRank", label: pickText(lang, "参数排名", "Parameter rank"), align: "right" as const },
    { key: "direction", label: pickText(lang, "方向", "Direction") },
    { key: "leverage", label: pickText(lang, "杠杆 / 仓位", "Leverage / notional") },
    { key: "searchMode", label: pickText(lang, "回测级别", "Mode") },
    { key: "parameters", label: pickText(lang, "马丁参数", "Martingale parameters") },
    { key: "returnPct", label: pickText(lang, "收益", "Return"), align: "right" as const },
    { key: "score", label: "Score /100", align: "right" as const },
    { key: "drawdown", label: pickText(lang, "最大回撤", "Max DD"), align: "right" as const },
    { key: "tradeCount", label: pickText(lang, "交易数", "Trades"), align: "right" as const },
    { key: "dynamicMetrics", label: pickText(lang, "动态指标", "Dynamic metrics") },
    { key: "score", label: pickText(lang, "评分", "Score"), align: "right" as const },
    { key: "decision", label: pickText(lang, "结论", "Decision") },
    { key: "actions", label: pickText(lang, "操作", "Actions") },
  ];
}

function candidateRow(
  candidate: BacktestCandidate,
  lang: UiLanguage,
  selectedId: string | undefined,
  onSelect: ((candidate: BacktestCandidate) => void) | undefined,
  onAddToBasket: ((candidate: BacktestCandidate) => void) | undefined,
): DataTableRow {
  const parameterRank = candidate.summary?.parameter_rank_for_symbol;
  const displayRank = parameterRank ?? candidate.rank;
  const leverage = candidate.summary?.recommended_leverage;
  const initialMargin = readFirstOrderQuote(candidate.summary);
  const initialNotional = initialMargin == null ? null : initialMargin * (leverage ?? 1);
  const totalMarginBudget = readTotalMarginBudgetQuote(candidate.summary);
  const dynamicSummary = candidate.summary as DynamicResultSummary;
  return {
    id: candidate.id,
    symbol: (
      <button className="text-left" onClick={() => onSelect?.(candidate)} type="button">
        <p className="font-medium">{candidate.symbol}</p>
        <p className="text-xs text-muted-foreground">{candidate.market}</p>
        {selectedId === candidate.id ? (
          <p className="text-xs text-primary">{pickText(lang, "已选中", "Selected")}</p>
        ) : null}
      </button>
    ),
    parameterRank: displayRank == null ? "—" : `#${displayRank}`,
    direction: formatDirection(candidate.summary, candidate.direction, lang),
    leverage: (
      <div className="space-y-0.5 text-xs">
        <p className="font-medium">{leverage == null ? "—" : `${leverage}x`}</p>
        <p className="text-muted-foreground">{initialMargin == null ? "—" : pickText(lang, `首单保证金 ${formatQuote(initialMargin)}U`, `Initial margin ${formatQuote(initialMargin)}U`)}</p>
        <p className="text-muted-foreground">{initialNotional == null ? "—" : pickText(lang, `首单仓位 ${formatQuote(initialNotional)}U`, `Initial notional ${formatQuote(initialNotional)}U`)}</p>
        <p className="text-muted-foreground">{totalMarginBudget == null ? "—" : pickText(lang, `总投入 ${formatQuote(totalMarginBudget)}U`, `Total margin ${formatQuote(totalMarginBudget)}U`)}</p>
      </div>
    ),
    searchMode: candidate.searchMode,
    score: candidate.score,
    returnPct: candidate.returnPct,
    drawdown: candidate.drawdown,
    tradeCount: candidate.tradeCount,
    dynamicMetrics: <DynamicMetrics summary={dynamicSummary} lang={lang} />,
    parameters: candidate.parameters,
    decision: candidate.decision,
    actions: (
      <button className="rounded-full border border-border px-3 py-1 text-xs font-medium" onClick={() => onAddToBasket?.(candidate)} type="button">
        {pickText(lang, "加入组合", "Add to basket")}
      </button>
    ),
  };
}

function DynamicMetrics({ summary, lang }: { summary: DynamicResultSummary; lang: UiLanguage }) {
  const rebalanceCount = summary.rebalance_count ?? summary.cost_summary?.rebalance_count;
  const forcedExitCount = summary.forced_exit_count ?? summary.cost_summary?.forced_exit_count;
  return (
    <div className="space-y-0.5 text-xs">
      <p>{pickText(lang, "年化收益", "Annualized return")}: {formatOptionalNumber(summary.annualized_return_pct, 2)}%</p>
      <p>{pickText(lang, "最大回撤", "Max drawdown")}: {formatOptionalNumber(summary.max_drawdown_pct, 2)}%</p>
      <p>{pickText(lang, "固定多空", "Fixed L/S")}: {formatOptionalNumber(summary.long_weight_pct, 0)}/{formatOptionalNumber(summary.short_weight_pct, 0)}</p>
      <p>{pickText(lang, "实际多空", "Actual L/S")}: {formatOptionalNumber(summary.actual_long_weight_pct, 0)}/{formatOptionalNumber(summary.actual_short_weight_pct, 0)}</p>
      <p>{pickText(lang, "止损次数", "Stop losses")}: {formatOptionalNumber(summary.stop_loss_count, 0)}</p>
      <p>{pickText(lang, "收益回撤比", "Return/DD ratio")}: {formatOptionalNumber(readReturnDrawdownRatio(summary), 2)}</p>
      <p>{pickText(lang, "调仓次数", "Rebalances")}: {formatOptionalNumber(rebalanceCount, 0)}</p>
      <p>{pickText(lang, "强平次数", "Forced exits")}: {formatOptionalNumber(forcedExitCount, 0)}</p>
      <p>{pickText(lang, "交易成本", "Trading cost")}: {formatCostSummary(summary.cost_summary)}</p>
      <p>{pickText(lang, "是否满足最大回撤限制", "Max DD limit passed")}: {formatBool(summary.max_drawdown_limit_passed, lang)}</p>
      <p>{pickText(lang, "是否可推荐实盘", "Live recommendable")}: {formatBool(summary.live_recommended ?? summary.can_recommend_live, lang)}</p>
    </div>
  );
}

function DiscardedSymbolsNotice({ candidates, lang }: { candidates: BacktestCandidate[]; lang: UiLanguage }) {
  const symbols = Array.from(new Set(candidates.flatMap((candidate) => readDiscardedSymbols(candidate.summary as DynamicResultSummary))));
  if (symbols.length === 0) return null;
  return (
    <p className="rounded-lg border border-amber-500/30 bg-amber-500/5 px-3 py-2 text-xs text-muted-foreground">
      {pickText(lang, "组合 Top10 已剔除币种", "Discarded symbols from portfolio Top10")}: {symbols.join(", ")}
      {pickText(lang, "；通常因为风控、回撤或权重约束未通过。", "; usually due to risk, drawdown, or weight constraints.")}
    </p>
  );
}

function readReturnDrawdownRatio(summary: DynamicResultSummary) {
  const explicitRatio = readFiniteNumber(summary.return_drawdown_ratio) ?? readFiniteNumber(summary.profit_drawdown_ratio);
  if (explicitRatio != null) return explicitRatio;
  const maxDrawdown = readFiniteNumber(summary.max_drawdown_pct);
  const totalReturn = readFiniteNumber(summary.total_return_pct);
  const absDrawdown = maxDrawdown == null ? null : Math.abs(maxDrawdown);
  return totalReturn != null && absDrawdown != null && absDrawdown > 0 ? totalReturn / absDrawdown : null;
}

function readDiscardedSymbols(summary: DynamicResultSummary) {
  return summary.discarded_symbols_from_portfolio_top10 ?? summary.portfolio_top10_discarded_symbols ?? [];
}

function formatOptionalNumber(value: number | null | undefined, decimals: number) {
  return value == null || !Number.isFinite(value) ? "—" : value.toLocaleString(undefined, { minimumFractionDigits: decimals, maximumFractionDigits: decimals });
}

function formatBool(value: boolean | undefined, lang: UiLanguage) {
  if (value == null) return "—";
  return value ? pickText(lang, "是", "Yes") : pickText(lang, "否", "No");
}

function formatCostSummary(costSummary: DynamicResultSummary["cost_summary"]) {
  if (!costSummary) return "—";
  const total = [costSummary.fee_quote, costSummary.slippage_quote, costSummary.stop_loss_quote, costSummary.forced_exit_quote]
    .map(readFiniteNumber)
    .filter((value): value is number => value != null)
    .reduce((sum, value) => sum + value, 0);
  return `${formatOptionalNumber(total, 2)}U`;
}

function readFiniteNumber(value: unknown): number | null {
  if (typeof value === "number") return Number.isFinite(value) ? value : null;
  if (typeof value === "string" && value.trim()) {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : null;
  }
  return null;
}

function readFirstOrderQuote(summary: MartingaleBacktestCandidateSummary) {
  const value = summary.first_order_quote;
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function readTotalMarginBudgetQuote(summary: MartingaleBacktestCandidateSummary) {
  const value = summary.total_margin_budget_quote;
  return typeof value === "number" && Number.isFinite(value) ? value : null;
}

function formatQuote(value: number) {
  return Number.isInteger(value) ? String(value) : value.toFixed(2);
}


function formatDirection(summary: MartingaleBacktestCandidateSummary, fallback: string, lang: UiLanguage) {
  if (summary.direction === "long_and_short") {
    const legs = summary.strategy_legs ?? [];
    const legText = legs
      .map((leg) => {
        const direction = leg.direction === "short" ? "Short" : leg.direction === "long" ? "Long" : "—";
        const spacing = typeof leg.spacing_bps === "number" ? `${(leg.spacing_bps / 100).toFixed(2)}%` : "—";
        const takeProfit = typeof leg.take_profit_bps === "number" ? `${(leg.take_profit_bps / 100).toFixed(2)}%` : "—";
        return `${direction} ${spacing}/${takeProfit}`;
      })
      .join(" · ");
    return (
      <div className="space-y-0.5 text-xs">
        <p className="font-medium">Long + Short</p>
        <p className="text-muted-foreground">{legText || pickText(lang, "双向组合", "Dual-leg portfolio")}</p>
      </div>
    );
  }
  return fallback;
}
