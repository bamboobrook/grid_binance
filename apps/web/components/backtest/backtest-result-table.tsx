import { DataTable, type DataTableRow } from "@/components/ui/table";
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
};

type BacktestResultTableProps = {
  candidates: BacktestCandidate[];
  lang: UiLanguage;
  onSelect?: (candidate: BacktestCandidate) => void;
  selectedId?: string;
  taskName?: string;
};

export function BacktestResultTable({
  candidates,
  lang,
  onSelect,
  selectedId,
  taskName,
}: BacktestResultTableProps) {
  const rows: DataTableRow[] = candidates.map((candidate) => ({
    id: candidate.id,
    symbol: (
      <button
        className="text-left"
        onClick={() => onSelect?.(candidate)}
        type="button"
      >
        <p className="font-medium">{candidate.symbol}</p>
        <p className="text-xs text-muted-foreground">{candidate.market}</p>
        {selectedId === candidate.id ? (
          <p className="text-xs text-primary">{pickText(lang, "已选中", "Selected")}</p>
        ) : null}
      </button>
    ),
    direction: candidate.direction,
    searchMode: candidate.searchMode,
    score: candidate.score,
    returnPct: candidate.returnPct,
    drawdown: candidate.drawdown,
    tradeCount: candidate.tradeCount,
    parameters: candidate.parameters,
    decision: candidate.decision,
  }));

  return (
    <section className="space-y-3 rounded-2xl border border-border bg-card p-4 shadow-sm">
      <div className="flex items-center justify-between gap-3">
        <div>
          <h2 className="text-lg font-semibold">{pickText(lang, "候选结果表", "Candidate result table")}</h2>
          <p className="text-sm text-muted-foreground">
            {taskName
              ? pickText(lang, `当前任务：${taskName}`, `Current task: ${taskName}`)
              : pickText(lang, "选择任务后查看 Worker 生成的真实候选。", "Select a task to view real worker-generated candidates.")}
          </p>
        </div>
        <code className="rounded bg-secondary/50 px-3 py-1 text-xs">GET /api/user/backtest/tasks/:id/candidates</code>
      </div>

      <DataTable
        caption={pickText(lang, "Top Candidates", "Top Candidates")}
        columns={[
          { key: "symbol", label: "Symbol" },
          { key: "direction", label: pickText(lang, "方向", "Direction") },
          { key: "searchMode", label: pickText(lang, "回测级别", "Mode") },
          { key: "parameters", label: pickText(lang, "马丁参数", "Martingale parameters") },
          { key: "returnPct", label: pickText(lang, "收益", "Return"), align: "right" },
          { key: "drawdown", label: pickText(lang, "最大回撤", "Max DD"), align: "right" },
          { key: "tradeCount", label: pickText(lang, "交易数", "Trades"), align: "right" },
          { key: "score", label: pickText(lang, "评分", "Score"), align: "right" },
          { key: "decision", label: pickText(lang, "结论", "Decision") },
        ]}
        emptyMessage={pickText(lang, "暂无候选结果；请等待 Worker 完成海选和精测。", "No candidates yet; wait for the worker to finish screening and refinement.")}
        rows={rows}
      />
    </section>
  );
}
