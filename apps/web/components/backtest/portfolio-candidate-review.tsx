"use client";

import Link from "next/link";
import { useState } from "react";
import { requestBacktestApi } from "@/components/backtest/request-client";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";

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

type PortfolioCandidateReviewProps = {
  candidate: BacktestCandidate | null;
  lang: UiLanguage;
  locale: string;
};

export function PortfolioCandidateReview({ candidate, lang, locale }: PortfolioCandidateReviewProps) {
  const [feedback, setFeedback] = useState<string>("");
  const [pending, setPending] = useState(false);
  const [riskSummary, setRiskSummary] = useState<Record<string, unknown> | null>(null);
  const [portfolioId, setPortfolioId] = useState("");

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
    if (result.ok) {
      const data = result.data && typeof result.data === "object" ? result.data as Record<string, unknown> : {};
      const summary = data.risk_summary && typeof data.risk_summary === "object" ? data.risk_summary as Record<string, unknown> : data;
      setRiskSummary(summary);
      setPortfolioId(typeof data.portfolio_id === "string" ? data.portfolio_id : "");
      setFeedback(pickText(lang, "已创建待确认 Portfolio，请打开组合页人工确认启动。", "Pending Portfolio created. Open the portfolio page to confirm live start."));
      return;
    }

    setFeedback(result.message);
  }

  return (
    <section className="space-y-4 rounded-2xl border border-border bg-card p-4 shadow-sm">
      <div className="flex items-center justify-between gap-3">
        <div>
          <h2 className="text-lg font-semibold">{pickText(lang, "Portfolio 候选复核", "Portfolio candidate review")}</h2>
          <p className="text-sm text-muted-foreground">
            {pickText(lang, "发布前先拿到风险摘要，再决定是否创建待启动 Portfolio。", "Fetch the risk summary before deciding whether to create a pending-start Portfolio.")}
          </p>
        </div>
        <code className="rounded bg-secondary/50 px-3 py-1 text-xs">POST /api/user/backtest/candidates/:id/publish-intent</code>
      </div>

      <div className="rounded-xl border border-border bg-background p-4">
        {candidate ? (
          <dl className="space-y-3 text-sm">
            <ReviewRow label={pickText(lang, "候选 ID", "Candidate ID")} value={candidate.id} />
            <ReviewRow label="Symbol" value={candidate.symbol} />
            <ReviewRow label={pickText(lang, "市场/方向", "Market / Direction")} value={`${candidate.market} · ${candidate.direction}`} />
            <ReviewRow label={pickText(lang, "评分", "Score")} value={candidate.score} />
            <ReviewRow label={pickText(lang, "最大回撤", "Max drawdown")} value={candidate.drawdown} />
            <ReviewRow label={pickText(lang, "结论", "Decision")} value={candidate.decision} />
          </dl>
        ) : (
          <p className="text-sm text-muted-foreground">
            {pickText(lang, "暂无可复核候选；Worker 完成后会在这里显示真实候选。", "No candidate selected; real worker candidates appear here after completion.")}
          </p>
        )}
      </div>

      {portfolioId ? (
        <div className="rounded-xl border border-emerald-500/30 bg-emerald-500/5 p-4 text-sm">
          <p className="font-semibold text-emerald-700 dark:text-emerald-300">
            {pickText(lang, "待确认 Portfolio 已创建", "Pending Portfolio created")}
          </p>
          <p className="mt-1 text-muted-foreground">
            {pickText(lang, "下一步：进入马丁组合页，检查状态后点击“启动 Portfolio”。", "Next: open Martingale Portfolios, review the status, then click Start portfolio.")}
          </p>
          <Link className="mt-3 inline-flex rounded-full bg-primary px-4 py-2 text-sm font-medium text-primary-foreground" href={`/${locale}/app/martingale-portfolios`}>
            {pickText(lang, "去确认启动", "Confirm start")} · {portfolioId}
          </Link>
        </div>
      ) : null}

      {riskSummary ? (
        <div className="rounded-xl border border-border bg-background p-4 text-sm">
          <p className="font-semibold">{pickText(lang, "后端风险摘要", "Backend risk summary")}</p>
          <pre className="mt-2 max-h-52 overflow-auto whitespace-pre-wrap text-xs text-muted-foreground">
            {JSON.stringify(riskSummary, null, 2)}
          </pre>
        </div>
      ) : null}

      <ul className="space-y-2 text-sm text-muted-foreground">
        <li>{pickText(lang, "不自动启动实盘，必须手动确认。", "Live launch stays manual and explicit.")}</li>
        <li>{pickText(lang, "同 symbol 杠杆和保证金模式冲突必须阻断。", "Same-symbol leverage and margin conflicts must block publishing.")}</li>
        <li>{pickText(lang, "Long+Short 发布前继续要求 Hedge Mode。", "Long+Short still requires Hedge Mode before publishing.")}</li>
      </ul>

      <div className="space-y-3">
        <button
          className="w-full rounded-full bg-primary px-4 py-2 text-sm font-medium text-primary-foreground disabled:opacity-60"
          disabled={pending || !candidate}
          onClick={handlePublishIntent}
          type="button"
        >
          {pickText(lang, "创建待确认 Portfolio", "Create pending Portfolio")}
        </button>
        <p aria-live="polite" className="text-sm text-muted-foreground">{feedback}</p>
      </div>
    </section>
  );
}

function ReviewRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="flex items-start justify-between gap-3">
      <dt className="text-muted-foreground">{label}</dt>
      <dd className="max-w-[18rem] text-right font-medium">{value}</dd>
    </div>
  );
}
