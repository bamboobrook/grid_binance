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
              : pickText(lang, "暂无回测任务，选择币种后开始自动搜索 Top 5", "No backtest tasks yet; select symbols to start automatic Top 5 search.")}
          </p>
        </div>
        <code className="rounded bg-secondary/50 px-3 py-1 text-xs">GET /api/user/backtest/tasks/:id/candidates</code>
      </div>

      {groupedCandidates.length === 0 ? (
        <DataTable
          caption={pickText(lang, "每个币种 Top 5", "Per-symbol Top 5")}
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
              <h3 className="text-base font-semibold">{pickText(lang, `每个币种 Top 5 · ${group.symbol}`, `Per-symbol Top 5 · ${group.symbol}`)}</h3>
              <p className="text-xs text-muted-foreground">{pickText(lang, "按参数排名挑选每个币种最优候选。", "Sorted by parameter rank for each symbol.")}</p>
            </div>
            <DataTable
              caption={pickText(lang, "每个币种 Top 5", "Per-symbol Top 5")}
              columns={candidateColumns(lang)}
              emptyMessage={pickText(lang, "暂无候选结果；请等待 Worker 完成海选和精测。", "No candidates yet; wait for the worker to finish screening and refinement.")}
              rows={group.candidates.map((candidate) => candidateRow(candidate, lang, selectedId, onSelect, onAddToBasket))}
            />
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
      .slice(0, 5),
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
    { key: "drawdown", label: pickText(lang, "最大回撤", "Max DD"), align: "right" as const },
    { key: "tradeCount", label: pickText(lang, "交易数", "Trades"), align: "right" as const },
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
