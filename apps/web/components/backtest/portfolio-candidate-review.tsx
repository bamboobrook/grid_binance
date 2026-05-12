"use client";

import Link from "next/link";
import { useState } from "react";
import { publishPortfolio, requestBacktestApi } from "@/components/backtest/request-client";
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

export type PortfolioBasketItem = {
  localId: string;
  candidateId: string;
  taskId: string;
  selectedTaskId: string;
  symbol: string;
  market: string;
  direction: string;
  riskProfile: string;
  parameters: string;
  recommended_weight_pct?: number;
  recommended_leverage?: number;
  weightPct: string;
  leverage: string;
  enabled: boolean;
  parameterSnapshot: Record<string, unknown>;
  metricsSnapshot: Record<string, unknown>;
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

function fmtPct(v: number | null | undefined): string {
  if (v == null) return "—";
  return `${(v * 100).toFixed(2)}%`;
}

function fmtNum(v: number | null | undefined, decimals = 2): string {
  if (v == null) return "—";
  return v.toLocaleString(undefined, { minimumFractionDigits: decimals, maximumFractionDigits: decimals });
}

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
            {risk.warnings.map((warning, index) => (
              <li key={index}>{warning}</li>
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

export function PortfolioCandidateReview({
  basketItems,
  candidate,
  lang,
  locale,
  onPublish,
  onRemove,
  onUpdate,
}: {
  basketItems: PortfolioBasketItem[];
  candidate: BacktestCandidate | null;
  lang: UiLanguage;
  locale: string;
  onPublish: (payload: Record<string, unknown>) => Promise<unknown>;
  onRemove: (localId: string) => void;
  onUpdate: (localId: string, patch: Partial<PortfolioBasketItem>) => void;
}) {
  const [feedback, setFeedback] = useState("");
  const [pending, setPending] = useState(false);
  const [riskSummary, setRiskSummary] = useState<MartingaleRiskSummary | null>(null);
  const [portfolioId, setPortfolioId] = useState("");
  const [portfolioName, setPortfolioName] = useState("");

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
    setFeedback(pickText(lang, "已创建单候选待确认 Portfolio；也可继续使用组合篮子批量发布。", "Single-candidate pending Portfolio created; you can also batch publish from the basket."));
  }

  async function handleBatchPublish() {
    const enabledItems = basketItems.filter((item) => item.enabled);
    if (enabledItems.length === 0) {
      setFeedback(pickText(lang, "请先向组合篮子加入候选。", "Add candidates to the portfolio basket first."));
      return;
    }
    if (!weightTotalBalanced) {
      setFeedback(pickText(lang, "权重合计必须为 100%。", "Weight total must be 100%."));
      return;
    }

    const first = enabledItems[0];
    const payload = {
      name: portfolioName.trim() || `${first.symbol} basket`,
      task_id: first.taskId || first.selectedTaskId,
      market: first.market,
      direction: first.direction,
      risk_profile: first.riskProfile || "balanced",
      total_weight_pct: 100,
      items: enabledItems.map((item) => ({
        candidate_id: item.candidateId,
        symbol: item.symbol,
        weight_pct: readNumber(item.weightPct) ?? 0,
        leverage: readNumber(item.leverage) ?? 1,
        enabled: item.enabled,
        parameter_snapshot: item.parameterSnapshot,
      })),
    };

    setPending(true);
    setFeedback(pickText(lang, "正在批量发布实盘组合…", "Batch publishing live portfolio..."));
    setPortfolioId("");
    const result = await onPublish(payload) as Awaited<ReturnType<typeof publishPortfolio>>;
    setPending(false);
    if (!result.ok) {
      setFeedback(result.message);
      return;
    }

    const data = readObject(result.data) ?? {};
    const createdPortfolioId = readString(data.portfolio_id) || readString(data.id);
    setPortfolioId(createdPortfolioId);
    setFeedback(pickText(lang, "已批量发布实盘组合，仍需在组合页确认启动。", "Live portfolio batch published; confirm start on the portfolio page."));
  }

  const risk = riskSummary ?? candidate?.riskSummary;
  const weightTotal = basketItems.filter((item) => item.enabled).reduce((sum, item) => sum + (readNumber(item.weightPct) ?? 0), 0);
  const weightTotalBalanced = basketItems.length > 0 && Math.abs(weightTotal - 100) <= 0.01;
  const portfolioHref = portfolioId ? `/${locale}/app/martingale-portfolios/${portfolioId}` : `/${locale}/app/martingale-portfolios`;

  return (
    <section className="rounded-2xl border border-border bg-card p-4 shadow-sm space-y-4">
      <div className="flex items-center justify-between gap-3">
        <div>
          <h2 className="text-lg font-semibold">
            {pickText(lang, "候选复核与组合篮子", "Candidate review and portfolio basket")}
          </h2>
          <p className="text-sm text-muted-foreground">
            {pickText(lang, "可先查看单候选风险摘要，再把多个候选加入组合篮子批量发布。", "Review single-candidate risk, then add multiple candidates to the basket for batch publish.")}
          </p>
        </div>
        <code className="hidden rounded bg-secondary/50 px-3 py-1 text-xs md:block">POST /api/user/backtest/portfolios/publish</code>
      </div>

      {candidate ? (
        <div className="space-y-3">
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

          {risk ? (
            <div>
              <h3 className="text-sm font-semibold mb-2">
                {pickText(lang, "单候选风险摘要", "Single-candidate risk summary")}
              </h3>
              <RiskSummaryDisplay risk={risk} lang={lang} />
            </div>
          ) : (
            <p className="text-sm text-muted-foreground">
              {pickText(lang, "尚未发起单候选发布意图，风险摘要暂不可用。", "No single-candidate publish intent issued yet; risk summary unavailable.")}
            </p>
          )}
        </div>
      ) : (
        <p className="text-sm text-muted-foreground">
          {pickText(lang, "选择候选可查看详情；也可以直接从结果表加入组合篮子。", "Select a candidate to view details, or add candidates from the result table directly to the basket.")}
        </p>
      )}

      <div className="rounded-xl border border-border bg-background p-4">
        <div className="flex items-start justify-between gap-3">
          <div>
            <h3 className="text-sm font-semibold">
              {pickText(lang, "组合篮子", "Portfolio basket")}
            </h3>
            <p className="mt-1 text-xs text-muted-foreground">
              {pickText(lang, "从结果表加入多个候选，调整 enabled、权重与杠杆；权重合计为 100% 后可批量发布。", "Add multiple candidates from the result table, adjust enabled, weight and leverage; publish when total weight is 100%.")}
            </p>
          </div>
        </div>

        <label className="mt-4 block space-y-1 text-sm">
          <span className="text-[11px] uppercase tracking-wide text-muted-foreground">portfolio name</span>
          <input
            className="w-full rounded-lg border border-border bg-background px-3 py-2"
            onChange={(event) => setPortfolioName(event.currentTarget.value)}
            placeholder={pickText(lang, "例如：BTC/ETH 马丁组合", "Example: BTC/ETH martingale basket")}
            value={portfolioName}
          />
        </label>

        <div className="mt-4 space-y-3">
          {basketItems.length === 0 ? (
            <p className="rounded-xl border border-dashed border-border p-4 text-sm text-muted-foreground">
              {pickText(lang, "组合篮子为空，请从结果表点击“加入组合”。", "Portfolio basket is empty; click Add to basket in the result table.")}
            </p>
          ) : basketItems.map((item) => (
            <div className="grid gap-3 rounded-xl border border-border bg-card p-3 md:grid-cols-[minmax(0,1.5fr)_0.6fr_0.8fr_0.8fr_auto]" key={item.localId}>
              <div className="space-y-1">
                <p className="text-sm font-medium">{item.symbol}</p>
                <p className="text-xs text-muted-foreground">{item.candidateId}</p>
                <p className="text-xs text-muted-foreground">{item.parameters}</p>
                <p className="text-xs text-muted-foreground">
                  {pickText(lang, "推荐权重", "Recommended weight")}: {item.recommended_weight_pct ?? "—"}% · {pickText(lang, "推荐杠杆", "Recommended leverage")}: {item.recommended_leverage == null ? "—" : `${item.recommended_leverage}x`}
                </p>
              </div>
              <label className="flex items-center gap-2 text-sm">
                <input
                  checked={item.enabled}
                  onChange={(event) => onUpdate(item.localId, { enabled: event.currentTarget.checked })}
                  type="checkbox"
                />
                <span>{pickText(lang, "启用", "Enabled")}</span>
              </label>
              <label className="space-y-1 text-sm">
                <span className="text-[11px] uppercase tracking-wide text-muted-foreground">{pickText(lang, "权重 %", "Weight %")}</span>
                <input
                  className="w-full rounded-lg border border-border bg-background px-3 py-2"
                  min="0"
                  onChange={(event) => onUpdate(item.localId, { weightPct: event.currentTarget.value })}
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
                  onChange={(event) => onUpdate(item.localId, { leverage: event.currentTarget.value })}
                  step="1"
                  type="number"
                  value={item.leverage}
                />
              </label>
              <button
                className="self-end rounded-full border border-border px-3 py-2 text-xs font-medium"
                onClick={() => onRemove(item.localId)}
                type="button"
              >
                {pickText(lang, "移除", "Remove")}
              </button>
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
            {pickText(lang, "Portfolio 已创建", "Portfolio created")} · {portfolioId}
          </p>
          <Link className="mt-3 inline-flex rounded-full bg-primary px-4 py-2 text-sm font-medium text-primary-foreground" href={portfolioHref}>
            {pickText(lang, "打开实盘组合", "Open live portfolio")}
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
          disabled={pending || basketItems.length === 0 || !weightTotalBalanced}
          onClick={handleBatchPublish}
          type="button"
        >
          {pending ? pickText(lang, "发布中…", "Publishing...") : pickText(lang, "批量发布实盘组合", "Batch publish live portfolio")}
        </button>
        <button
          className="w-full rounded-full border border-border px-4 py-2 text-sm font-medium disabled:opacity-60"
          disabled={pending || !candidate}
          onClick={handlePublishIntent}
          type="button"
        >
          {pending ? pickText(lang, "生成中…", "Generating...") : pickText(lang, "生成单候选风险摘要", "Generate single-candidate risk summary")}
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
