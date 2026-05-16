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

type BacktestResultTableProps = {
  candidates: BacktestCandidate[];
  lang: UiLanguage;
  onAddToBasket?: (candidate: BacktestCandidate) => void;
  onSelect?: (candidate: BacktestCandidate) => void;
  selectedId?: string;
  selectedTaskStatus?: string;
  taskName?: string;
  portfolioTop3?: Array<{ candidate_id: string; source_candidate_id: string; symbol: string; return_pct: number; max_drawdown_pct: number; trade_count: number; score: number }>;
};

export function BacktestResultTable({
  candidates,
  lang,
  onAddToBasket,
  onSelect,
  selectedId,
  selectedTaskStatus,
  taskName,
  portfolioTop3 = [],
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
            : pickText(lang, "暂无回测任务，选择币种后开始自动搜索 Top 5", "No backtest tasks yet; select symbols to start automatic Top 5 search.")}
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
          </div>
        ))
      )}
      {portfolioTop3.length > 0 && (
        <div className="mt-6 space-y-3">
          <h3 className="text-base font-semibold">{pickText(lang, "组合 Top 3", "Portfolio Top 3")}</h3>
          <div className="grid grid-cols-1 md:grid-cols-3 gap-3">
            {portfolioTop3.map((entry, idx) => (
              <div key={idx} className="rounded-xl border border-border bg-card p-4 space-y-2">
                <div className="flex items-center justify-between">
                  <span className="text-xs font-mono text-muted-foreground">#{idx + 1}</span>
                  <span className="text-sm font-semibold">{entry.symbol}</span>
                </div>
                <div className="grid grid-cols-2 gap-x-4 gap-y-1 text-xs">
                  <span className="text-muted-foreground">{pickText(lang, "收益", "Return")}</span>
                  <span className={entry.return_pct >= 0 ? "text-green-600" : "text-red-600"}>{entry.return_pct.toFixed(2)}%</span>
                  <span className="text-muted-foreground">{pickText(lang, "回撤", "Drawdown")}</span>
                  <span className="text-red-600">{entry.max_drawdown_pct.toFixed(2)}%</span>
                  <span className="text-muted-foreground">{pickText(lang, "交易", "Trades")}</span>
                  <span>{entry.trade_count}</span>
                  <span className="text-muted-foreground">{pickText(lang, "评分", "Score")}</span>
                  <span className="font-semibold">{entry.score.toFixed(3)}</span>
                </div>
              </div>
            ))}
          </div>
        </div>
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
    { key: "searchMode", label: pickText(lang, "回测级别", "Mode") },
    { key: "parameters", label: pickText(lang, "马丁参数", "Martingale parameters") },
    { key: "returnPct", label: pickText(lang, "收益", "Return"), align: "right" as const },
    { key: "annualized", label: pickText(lang, "年化收益", "Annualized"), align: "right" as const },
    { key: "drawdown", label: pickText(lang, "最大回撤", "Max DD"), align: "right" as const },
    { key: "returnDrawdownRatio", label: pickText(lang, "收益回撤比", "Return/DD"), align: "right" as const },
    { key: "tradeCount", label: pickText(lang, "交易数", "Trades"), align: "right" as const },
    { key: "score", label: pickText(lang, "评分（百分制 0–100）", "Score (0–100 scale)"), align: "right" as const },
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
    direction: candidate.direction,
    searchMode: candidate.searchMode,
    score: candidate.score,
    returnPct: candidate.returnPct,
    drawdown: candidate.drawdown,
    tradeCount: candidate.tradeCount,
    parameters: candidate.parameters,
    decision: candidate.decision,
    actions: (
      <button className="rounded-full border border-border px-3 py-1 text-xs font-medium" onClick={() => onAddToBasket?.(candidate)} type="button">
        {pickText(lang, "加入组合", "Add to basket")}
      </button>
    ),
  };
}
