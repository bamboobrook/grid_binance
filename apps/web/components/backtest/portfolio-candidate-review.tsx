"use client";

import Link from "next/link";
import { useEffect, useState } from "react";
import { requestBacktestApi } from "@/components/backtest/request-client";
import type { MartingaleBacktestCandidateSummary, MartingaleRiskSummary } from "@/lib/api-types";
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
  summary?: MartingaleBacktestCandidateSummary | null;
  riskSummary?: MartingaleRiskSummary | null;
};

type BasketItem = {
  candidateId: string;
  symbol: string;
  parameters: string;
  recommendedWeightPct: number | null;
  recommendedLeverage: number | null;
  weightPct: string;
  leverage: string;
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
  const [feedback, setFeedback] = useState("");
  const [pending, setPending] = useState(false);
  const [riskSummary, setRiskSummary] = useState<MartingaleRiskSummary | null>(null);
  const [portfolioId, setPortfolioId] = useState("");
  const [basketItems, setBasketItems] = useState<BasketItem[]>([]);

  useEffect(() => {
    if (!candidate) {
      setBasketItems([]);
      return;
    }

    const recommendedWeightPct = candidate.summary?.recommended_weight_pct ?? 100;
    const recommendedLeverage = candidate.summary?.recommended_leverage ?? null;
    setBasketItems([
      {
        candidateId: candidate.id,
        symbol: candidate.symbol,
        parameters: candidate.parameters,
        recommendedWeightPct,
        recommendedLeverage,
        weightPct: String(recommendedWeightPct),
        leverage: recommendedLeverage == null ? "" : String(recommendedLeverage),
      },
    ]);
  }, [candidate]);

  async function handlePublishIntent() {
    if (!candidate) {
      setFeedback(pickText(lang, "请先选择一个候选。", "Select a candidate first."));
      return;
    }
    setPending(true);
    setFeedback(pickText(lang, "正在生成发布风险摘要…", "Generating publish risk summary..."));
    setRiskSummary(null);
    setPortfolioId("");

    const result = await requestBacktestApi(`/api/user/backtest/candidates/${candidate.id}/publish-intent`, {
      method: "POST",
    });

    setPending(false);
    if (!result.ok) {
      setFeedback(result.message);
      return;
    }

    const data = readObject(result.data) ?? {};
    const summary = readRiskSummary(data.risk_summary) ?? readRiskSummary(data);
    setRiskSummary(summary);
    setPortfolioId(readString(data.portfolio_id));
    setFeedback(pickText(lang, "已创建待确认 Portfolio，请打开组合页人工确认启动。", "Pending Portfolio created. Open the portfolio page to confirm live start."));
  }

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

  const risk = riskSummary ?? candidate.riskSummary;
  const weightTotal = basketItems.reduce((sum, item) => sum + (readNumber(item.weightPct) ?? 0), 0);
  const weightTotalBalanced = basketItems.length > 0 && Math.abs(weightTotal - 100) <= 0.01;

  return (
    <section className="rounded-2xl border border-border bg-card p-4 shadow-sm space-y-4">
      <div className="flex items-center justify-between gap-3">
        <div>
          <h2 className="text-lg font-semibold">
            {pickText(lang, "候选复核", "Candidate review")}
          </h2>
          <p className="text-sm text-muted-foreground">
            {pickText(lang, "发布前先生成风险摘要；实盘启动仍需手动确认。", "Generate the risk summary first; live start remains manual.")}
          </p>
        </div>
        <code className="hidden rounded bg-secondary/50 px-3 py-1 text-xs md:block">POST /api/user/backtest/candidates/:id/publish-intent</code>
      </div>

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

      <div className="rounded-xl border border-border bg-background p-4">
        <div className="flex items-start justify-between gap-3">
          <div>
            <h3 className="text-sm font-semibold">
              {pickText(lang, "组合篮子", "Portfolio basket")}
            </h3>
            <p className="mt-1 text-xs text-muted-foreground">
              {pickText(lang, "当前先按已选候选生成一行篮子，可调整权重与杠杆；发布交互仍保持单候选。", "The basket currently stages the selected candidate as one row with editable weight and leverage; publish remains single-candidate.")}
            </p>
          </div>
          <div className="text-right text-[11px] text-muted-foreground">
            <div>recommended_weight_pct</div>
            <div>recommended_leverage</div>
          </div>
        </div>

        <div className="mt-4 space-y-3">
          {basketItems.map((item, index) => (
            <div className="grid gap-3 rounded-xl border border-border bg-card p-3 md:grid-cols-[minmax(0,1.6fr)_repeat(4,minmax(0,0.8fr))]" key={`${item.candidateId}-${index}`}>
              <div className="space-y-1">
                <p className="text-sm font-medium">{item.symbol}</p>
                <p className="text-xs text-muted-foreground">{item.candidateId}</p>
                <p className="text-xs text-muted-foreground">{item.parameters}</p>
              </div>
              <div className="space-y-1">
                <p className="text-[11px] uppercase tracking-wide text-muted-foreground">recommended_weight_pct</p>
                <p className="text-sm font-medium">{fmtNum(item.recommendedWeightPct, 2)}%</p>
              </div>
              <div className="space-y-1">
                <p className="text-[11px] uppercase tracking-wide text-muted-foreground">recommended_leverage</p>
                <p className="text-sm font-medium">{item.recommendedLeverage == null ? "—" : `${item.recommendedLeverage}x`}</p>
              </div>
              <label className="space-y-1 text-sm">
                <span className="text-[11px] uppercase tracking-wide text-muted-foreground">{pickText(lang, "权重 %", "Weight %")}</span>
                <input
                  className="w-full rounded-lg border border-border bg-background px-3 py-2"
                  min="0"
                  name={`basket-weight-${index}`}
                  onChange={(event) => {
                    const value = event.currentTarget.value;
                    setBasketItems((current) => current.map((basketItem, basketIndex) => (
                      basketIndex === index ? { ...basketItem, weightPct: value } : basketItem
                    )));
                  }}
                  step="0.01"
                  type="number"
                  value={item.weightPct}
                />
              </label>
              <label className="space-y-1 text-sm">
                <span className="text-[11px] uppercase tracking-wide text-muted-foreground">{pickText(lang, "杠杆", "Leverage")}</span>
                <input
                  className="w-full rounded-lg border border-border bg-background px-3 py-2"
                  min="0"
                  name={`basket-leverage-${index}`}
                  onChange={(event) => {
                    const value = event.currentTarget.value;
                    setBasketItems((current) => current.map((basketItem, basketIndex) => (
                      basketIndex === index ? { ...basketItem, leverage: value } : basketItem
                    )));
                  }}
                  step="1"
                  type="number"
                  value={item.leverage}
                />
              </label>
            </div>
          ))}
        </div>

        <div className="mt-4 flex items-center justify-between rounded-lg border border-border bg-card px-3 py-2 text-sm">
          <span className="text-muted-foreground">{pickText(lang, "权重合计", "Weight total")}</span>
          <span className={weightTotalBalanced ? "font-semibold text-emerald-600" : "font-semibold text-amber-600"}>
            {fmtNum(weightTotal, 2)}%
          </span>
        </div>
      </div>

      {portfolioId ? (
        <div className="rounded-xl border border-emerald-500/30 bg-emerald-500/5 p-4 text-sm">
          <p className="font-semibold text-emerald-700 dark:text-emerald-300">
            {pickText(lang, "待确认 Portfolio 已创建", "Pending Portfolio created")}
          </p>
          <p className="mt-1 text-muted-foreground">
            {pickText(lang, "下一步：进入马丁组合页，检查状态后点击启动。", "Next: open Martingale Portfolios, review status, then start manually.")}
          </p>
          <Link className="mt-3 inline-flex rounded-full bg-primary px-4 py-2 text-sm font-medium text-primary-foreground" href={`/${locale}/app/martingale-portfolios`}>
            {pickText(lang, "去确认启动", "Confirm start")} · {portfolioId}
          </Link>
        </div>
      ) : null}

      <ul className="space-y-2 text-sm text-muted-foreground">
        <li>{pickText(lang, "不会自动启动实盘，必须人工确认。", "Live launch is never automatic and requires manual confirmation.")}</li>
        <li>{pickText(lang, "同 symbol 杠杆和保证金模式冲突会被后端阻断。", "Same-symbol leverage and margin conflicts are blocked server-side.")}</li>
        <li>{pickText(lang, "Long+Short 发布前继续要求 Binance Hedge Mode。", "Long+Short still requires Binance Hedge Mode before publishing.")}</li>
      </ul>

      <div className="space-y-3">
        <button
          className="w-full rounded-full bg-primary px-4 py-2 text-sm font-medium text-primary-foreground disabled:opacity-60"
          disabled={pending || !candidate}
          onClick={handlePublishIntent}
          type="button"
        >
          {pending ? pickText(lang, "生成中…", "Generating...") : pickText(lang, "创建待确认 Portfolio", "Create pending Portfolio")}
        </button>
        <p aria-live="polite" className="text-sm text-muted-foreground">{feedback}</p>
      </div>
    </section>
  );
}

function readRiskSummary(value: unknown): MartingaleRiskSummary | null {
  const object = readObject(value);
  return object ? object as MartingaleRiskSummary : null;
}
