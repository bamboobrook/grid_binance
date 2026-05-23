import { DataTable, type DataTableRow } from "@/components/ui/table";
import type { MartingaleBacktestCandidateSummary } from "@/lib/api-types";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";

function normalizePercentPoint(value: number | null | undefined): number | null {
  if (value == null || !Number.isFinite(value)) return null;
  return Math.abs(value) > 1000 ? value / 100 : value;
}

function formatPercentPoint(value: number | null | undefined): string {
  const normalized = normalizePercentPoint(value);
  return normalized == null ? "—" : `${normalized.toFixed(2)}%`;
}

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

export type PortfolioMember = {
  candidate_id: string;
  symbol: string;
  direction: string;
  allocation_pct: number;
  return_pct: number;
  max_drawdown_pct: number;
  annualized_return_pct?: number | null;
  score: number;
  trade_count: number;
  leverage?: number | null;
};

export type PortfolioTop3Row = {
  portfolio_id: string;
  portfolio_rank: number;
  member_count: number;
  members: PortfolioMember[];
  total_return_pct: number;
  return_pct: number;
  max_drawdown_pct: number;
  annualized_return_pct?: number | null;
  score: number;
  trade_count: number;
  equity_curve?: unknown[];
  drawdown_curve?: unknown[];
  trades_preview?: unknown[];
  eligible_candidate_count?: number | null;
};

type BacktestResultTableProps = {
  candidates: BacktestCandidate[];
  lang: UiLanguage;
  onAddToBasket?: (candidate: BacktestCandidate) => void;
  onSelect?: (candidate: BacktestCandidate) => void;
  onSelectPortfolio?: (portfolio: PortfolioTop3Row) => void;
  onEditPortfolio?: (portfolio: PortfolioTop3Row) => void;
  selectedId?: string;
  selectedTaskStatus?: string;
  taskName?: string;
  portfolioTop3?: PortfolioTop3Row[];
  portfolioTopN?: number;
  selectedPortfolioId?: string;
};

export function BacktestResultTable({
  candidates,
  lang,
  onAddToBasket,
  onSelect,
  onSelectPortfolio,
  onEditPortfolio,
  selectedId,
  selectedTaskStatus,
  taskName,
  portfolioTop3 = [],
  portfolioTopN = 3,
  selectedPortfolioId,
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
          <h3 className="text-base font-semibold">
            {pickText(lang, `组合 Top ${portfolioTop3.length}`, `Portfolio Top ${portfolioTop3.length}`)}
          </h3>
          <div className={portfolioTop3.length > 3 ? "overflow-x-auto" : ""}>
            <div className="grid grid-cols-1 md:grid-cols-3 gap-3" style={portfolioTop3.length > 3 ? { minWidth: "900px" } : undefined}>
            {portfolioTop3.map((entry, idx) => (
              <div key={entry.portfolio_id || idx} className={`rounded-xl border bg-card p-4 space-y-2 ${selectedPortfolioId === entry.portfolio_id ? "border-primary shadow-sm" : "border-border"}`}>
                <div className="flex items-center justify-between">
                  <span className="text-xs font-mono text-muted-foreground">#{entry.portfolio_rank || idx + 1}</span>
                  <span className="text-sm font-semibold">{pickText(lang, `${entry.member_count} 成员组合`, `${entry.member_count}-member portfolio`)}</span>
                </div>
                <div className="grid grid-cols-2 gap-x-4 gap-y-1 text-xs">
                  <span className="text-muted-foreground">{pickText(lang, "组合收益", "Portfolio return")}</span>
                  <span className={entry.return_pct >= 0 ? "text-green-600" : "text-red-600"}>{formatPercentPoint(entry.return_pct)}</span>
                  <span className="text-muted-foreground">{pickText(lang, "组合年化", "Annualized")}</span>
                  <span>{formatPercentPoint(entry.annualized_return_pct)}</span>
                  <span className="text-muted-foreground">{pickText(lang, "组合回撤", "Drawdown")}</span>
                  <span className="text-red-600">{formatPercentPoint(entry.max_drawdown_pct)}</span>
                  <span className="text-muted-foreground">{pickText(lang, "交易", "Trades")}</span>
                  <span>{entry.trade_count}</span>
                  <span className="text-muted-foreground">{pickText(lang, "评分", "Score")}</span>
                  <span className="font-semibold">{entry.score.toFixed(3)}</span>
                  <span className="text-muted-foreground">{pickText(lang, "最大单币权重", "Max single weight")}</span>
                  <span>{Math.max(...entry.members.map((m) => m.allocation_pct), 0).toFixed(1)}%</span>
                </div>
                {entry.members.length > 0 && (
                  <div className="mt-2 space-y-1">
                    <p className="text-xs font-medium text-muted-foreground">{pickText(lang, "成员权重", "Member weights")}</p>
                    {entry.members.map((m) => (
                      <div key={m.candidate_id} className="flex items-center justify-between text-xs">
                        <span>{m.symbol} ({m.direction})</span>
                        <span>{m.allocation_pct.toFixed(1)}%</span>
                      </div>
                    ))}
                  </div>
                )}
                <div className="mt-2 flex flex-wrap gap-2">
                  <button
                    className="rounded-full border border-border px-3 py-1 text-xs font-medium"
                    onClick={() => onSelectPortfolio?.(entry)}
                    type="button"
                  >
                    {pickText(lang, "查看组合图表/明细", "View portfolio charts/details")}
                  </button>
                  <button
                    className="rounded-full bg-primary px-3 py-1 text-xs font-medium text-primary-foreground"
                    onClick={() => onEditPortfolio?.(entry)}
                    type="button"
                  >
                    {pickText(lang, "编辑组合", "Edit portfolio")}
                  </button>
                </div>
              </div>
            ))}
          </div>
          {portfolioTop3.length > 3 ? (
            <p className="mt-2 text-xs text-muted-foreground">
              {pickText(lang, "横向滚动查看更多组合 →", "Scroll horizontally for more portfolios →")}
            </p>
          ) : null}
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
    { key: "leverage", label: pickText(lang, "杠杆", "Leverage"), align: "right" as const },
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
    annualized: formatPercentPoint(candidate.summary?.annualized_return_pct),
    leverage: candidate.summary?.max_leverage_used != null ? `${candidate.summary.max_leverage_used}x` : "—",
    returnDrawdownRatio: candidate.summary?.return_drawdown_ratio != null ? candidate.summary.return_drawdown_ratio.toFixed(2) : "—",
    actions: (
      <div className="flex gap-2">
        <button className="rounded-full border border-border px-3 py-1 text-xs font-medium" onClick={() => onSelect?.(candidate)} type="button">
          {pickText(lang, "查看详情", "View details")}
        </button>
        <button className="rounded-full border border-border px-3 py-1 text-xs font-medium" onClick={() => onAddToBasket?.(candidate)} type="button">
          {pickText(lang, "加入组合", "Add to basket")}
        </button>
      </div>
    ),
  };
}
