"use client";

import type { MartingaleRiskSummary } from "@/lib/api-types";
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
  riskSummary?: MartingaleRiskSummary | null;
};

function readObject(value: unknown): Record<string, unknown> | null {
  return value && typeof value === "object" && !Array.isArray(value) ? value as Record<string, unknown> : null;
}

function readNumber(value: unknown): number | null {
  if (typeof value === "number" && Number.isFinite(value)) return value;
  if (typeof value === "string" && value.trim()) {
    const parsed = Number(value);
    return Number.isFinite(parsed) ? parsed : null;
  }
  return null;
}

function readString(value: unknown): string {
  return typeof value === "string" ? value : "";
}

function readStringArray(value: unknown): string[] {
  return Array.isArray(value) ? value.filter((v): v is string => typeof v === "string") : [];
}

function fmtPct(v: number | null | undefined): string {
  if (v == null) return "—";
  return `${(v * 100).toFixed(2)}%`;
}

function fmtNum(v: number | null | undefined, decimals = 2): string {
  if (v == null) return "—";
  return v.toLocaleString(undefined, { minimumFractionDigits: decimals, maximumFractionDigits: decimals });
}

/* ------------------------------------------------------------------ */
/*  Human-readable risk summary                                       */
/* ------------------------------------------------------------------ */

function RiskSummaryDisplay({ risk, lang }: { risk: MartingaleRiskSummary; lang: UiLanguage }) {
  return (
    <div className="space-y-3">
      <div className="grid grid-cols-2 gap-x-4 gap-y-2 text-sm">
        <ReviewRow label={pickText(lang, "策略实例数", "Strategy instances")} value={risk.strategy_count ?? "—"} />
        <ReviewRow label={pickText(lang, "交易对", "Symbols")} value={risk.symbols?.join(", ") ?? "—"} />
        <ReviewRow label={pickText(lang, "最大杠杆", "Max leverage")} value={risk.max_leverage != null ? `${risk.max_leverage}x` : "—"} />
        <ReviewRow label={pickText(lang, "需要合约", "Requires futures")} value={risk.requires_futures ? pickText(lang, "是", "Yes") : pickText(lang, "否", "No")} />
        <ReviewRow label={pickText(lang, "最大回撤", "Max drawdown")} value={fmtPct(risk.max_drawdown)} highlight="danger" />
        <ReviewRow label={pickText(lang, "距强平", "Liquidation distance")} value={fmtPct(risk.liquidation_distance_pct)} />
        <ReviewRow label={pickText(lang, "资金费率估算", "Funding fee est.")} value={risk.funding_fee_estimate ?? "—"} />
        <ReviewRow label={pickText(lang, "总预算", "Total budget")} value={fmtNum(risk.total_budget_quote)} suffix=" USDT" />
        <ReviewRow label={pickText(lang, "单策略最大预算", "Max single budget")} value={fmtNum(risk.max_single_strategy_budget)} suffix=" USDT" />
      </div>
      {risk.warnings && risk.warnings.length > 0 && (
        <div className="rounded-lg border border-amber-500/40 bg-amber-500/5 px-3 py-2">
          <p className="text-xs font-semibold text-amber-700 dark:text-amber-300 mb-1">
            {pickText(lang, "风险警告", "Risk warnings")}
          </p>
          <ul className="list-disc list-inside text-xs text-muted-foreground space-y-0.5">
            {risk.warnings.map((w, i) => (
              <li key={i}>{w}</li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

function ReviewRow({
  label,
  value,
  highlight,
  suffix,
}: {
  label: string;
  value: React.ReactNode;
  highlight?: "danger" | "success";
  suffix?: string;
}) {
  const valueClass = highlight === "danger"
    ? "text-red-600 font-semibold"
    : highlight === "success"
      ? "text-emerald-600 font-semibold"
      : "text-foreground font-medium";
  return (
    <div className="flex justify-between items-baseline gap-2">
      <dt className="text-muted-foreground shrink-0">{label}</dt>
      <dd className={`${valueClass} text-right truncate`}>
        {value}{suffix ?? ""}
      </dd>
    </div>
  );
}

/* ------------------------------------------------------------------ */
/*  Main component                                                    */
/* ------------------------------------------------------------------ */

export function PortfolioCandidateReview({
  candidate,
  lang,
  locale,
}: {
  candidate: BacktestCandidate | null;
  lang: UiLanguage;
  locale: string;
}) {
  if (!candidate) {
    return (
      <section className="rounded-2xl border border-border bg-card p-4 shadow-sm">
        <h2 className="text-lg font-semibold mb-2">
          {pickText(lang, "候选复核", "Candidate review")}
        </h2>
        <p className="text-sm text-muted-foreground">
          {pickText(lang, "请先选择一个候选以查看详情", "Select a candidate to view details")}
        </p>
      </section>
    );
  }

  const risk = candidate.riskSummary;

  return (
    <section className="rounded-2xl border border-border bg-card p-4 shadow-sm space-y-4">
      <h2 className="text-lg font-semibold">
        {pickText(lang, "候选复核", "Candidate review")}
      </h2>

      {/* Key metrics */}
      <div className="grid grid-cols-2 gap-x-4 gap-y-2 text-sm">
        <ReviewRow label={pickText(lang, "交易对", "Symbol")} value={candidate.symbol} />
        <ReviewRow label={pickText(lang, "市场", "Market")} value={candidate.market} />
        <ReviewRow label={pickText(lang, "方向", "Direction")} value={candidate.direction} />
        <ReviewRow label={pickText(lang, "搜索模式", "Search mode")} value={candidate.searchMode} />
        <ReviewRow label={pickText(lang, "评分", "Score")} value={candidate.score} highlight="success" />
        <ReviewRow label={pickText(lang, "最大回撤", "Max drawdown")} value={candidate.drawdown} highlight="danger" />
        <ReviewRow label={pickText(lang, "总收益", "Return")} value={candidate.returnPct} />
        <ReviewRow label={pickText(lang, "交易次数", "Trades")} value={candidate.tradeCount} />
        <ReviewRow label={pickText(lang, "参数摘要", "Parameters")} value={candidate.parameters} />
        <ReviewRow label={pickText(lang, "决策", "Decision")} value={candidate.decision} />
      </div>

      {/* Risk summary — human readable */}
      {risk ? (
        <div>
          <h3 className="text-sm font-semibold mb-2">
            {pickText(lang, "发布风险摘要", "Publish risk summary")}
          </h3>
          <RiskSummaryDisplay risk={risk} lang={lang} />
        </div>
      ) : (
        <p className="text-sm text-muted-foreground">
          {pickText(lang, "尚未发起发布意图，风险摘要暂不可用", "No publish intent issued yet, risk summary unavailable")}
        </p>
      )}
    </section>
  );
}
