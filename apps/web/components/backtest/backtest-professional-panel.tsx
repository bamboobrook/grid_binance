"use client";

import { useState, type FormEvent } from "react";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";
import { requestBacktestApi } from "@/components/backtest/request-client";

const SAMPLE_PAYLOAD = JSON.stringify(
  {
    strategy_type: "martingale_grid",
    symbol_pool: {
      mode: "all_usdt",
      whitelist: ["BTCUSDT", "ETHUSDT"],
      blacklist: ["1000PEPEUSDT"],
    },
    market: "usd_m_futures",
    direction_mode: "long_and_short",
    hedge_mode_required: true,
    margin_mode: "isolated",
    leverage_range: [2, 4],
    search: {
      mode: "intelligent",
      rounds: 4,
      candidate_budget: 160,
      random_seed: 20260509,
    },
    time_split: {
      mode: "walk_forward",
      train_days: 120,
      validate_days: 30,
      test_days: 30,
      stress_windows: ["flash_crash", "trend_up"],
    },
    scoring: {
      profile: "survival_first",
      max_drawdown_pct: 18,
      max_stop_loss_count: 3,
    },
  },
  null,
  2,
);

export function BacktestProfessionalPanel({
  lang,
  onTaskCreated,
}: {
  lang: UiLanguage;
  onTaskCreated?: () => void | Promise<void>;
}) {
  const [feedback, setFeedback] = useState<string>("");
  const [status, setStatus] = useState<"idle" | "success" | "error" | "pending">("idle");

  async function handleSubmit(event: FormEvent<HTMLFormElement>) {
    event.preventDefault();
    setStatus("pending");
    setFeedback(pickText(lang, "正在创建回测任务…", "Creating backtest task..."));

    const formData = new FormData(event.currentTarget);
    const payload = String(formData.get("payload") ?? SAMPLE_PAYLOAD);

    const result = await requestBacktestApi("/api/user/backtest/tasks", {
      method: "POST",
      headers: {
        "content-type": "application/json",
      },
      body: payload,
    });

    if (result.ok) {
      setStatus("success");
      setFeedback(pickText(lang, "回测任务已创建，正在刷新任务列表。", "Backtest task created; refreshing the task list."));
      await onTaskCreated?.();
      return;
    }

    setStatus("error");
    setFeedback(result.message);
  }

  return (
    <div className="space-y-4">
      <div className="rounded-xl border border-border bg-background p-4">
        <div className="flex flex-wrap items-center justify-between gap-3">
          <div>
            <h2 className="text-lg font-semibold">
              {pickText(lang, "Professional Console", "Professional Console")}
            </h2>
            <p className="text-sm text-muted-foreground">
              {pickText(
                lang,
                "直接提交结构化 JSON 到任务代理路由，适合复制、审阅和复现。",
                "Submit structured JSON straight to the task proxy route for copyable, reviewable, reproducible runs.",
              )}
            </p>
          </div>
          <code className="rounded bg-secondary/50 px-3 py-1 text-xs">POST /api/user/backtest/tasks</code>
        </div>
      </div>

      <form className="space-y-4" onSubmit={handleSubmit}>
        <label className="block space-y-2">
          <span className="text-sm font-medium">{pickText(lang, "任务 JSON", "Task JSON")}</span>
          <textarea
            className="min-h-[360px] w-full rounded-xl border border-border bg-background px-4 py-3 font-mono text-xs outline-none ring-0"
            defaultValue={SAMPLE_PAYLOAD}
            name="payload"
          />
        </label>

        <div className="grid gap-3 md:grid-cols-3">
          <TipCard
            description={pickText(lang, "固定 seed 与代码版本，便于复现。", "Pin the seed and code version for reproducibility.")}
            title={pickText(lang, "复现性", "Reproducibility")}
          />
          <TipCard
            description={pickText(lang, "先海选，再精测，降低成本。", "Screen first, then refine, to control compute cost.")}
            title={pickText(lang, "两阶段", "Two-stage")}
          />
          <TipCard
            description={pickText(lang, "发布前仍需 Portfolio 风险复核。", "Portfolio risk review still gates publishing.")}
            title={pickText(lang, "发布门", "Publish gate")}
          />
        </div>

        <div className="flex flex-wrap gap-3">
          <button className="rounded-full bg-primary px-4 py-2 text-sm font-medium text-primary-foreground disabled:opacity-60" disabled={status === "pending"} type="submit">
            {pickText(lang, "创建回测任务", "Create backtest task")}
          </button>
          <button className="rounded-full border border-border px-4 py-2 text-sm font-medium" type="reset">
            {pickText(lang, "重置示例", "Reset sample")}
          </button>
        </div>
        <p
          aria-live="polite"
          className={
            status === "error"
              ? "text-sm text-red-600"
              : status === "success"
                ? "text-sm text-emerald-600"
                : "text-sm text-muted-foreground"
          }
        >
          {feedback}
        </p>
      </form>
    </div>
  );
}

function TipCard({ description, title }: { description: string; title: string }) {
  return (
    <div className="rounded-xl border border-border bg-card p-3">
      <p className="text-sm font-semibold">{title}</p>
      <p className="mt-1 text-xs text-muted-foreground">{description}</p>
    </div>
  );
}
