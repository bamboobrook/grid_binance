"use client";

import { useState } from "react";
import { requestBacktestApi } from "@/components/backtest/request-client";
import { IndicatorRuleEditor } from "@/components/backtest/indicator-rule-editor";
import { MartingaleParameterEditor } from "@/components/backtest/martingale-parameter-editor";
import { RiskRuleEditor } from "@/components/backtest/risk-rule-editor";
import { SearchConfigEditor } from "@/components/backtest/search-config-editor";
import { TimeSplitEditor } from "@/components/backtest/time-split-editor";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";

const STEPS = [
  {
    key: "search",
    titleZh: "1. 市场与搜索",
    titleEn: "1. Market and search",
    descriptionZh: "配置 symbol 池、市场类型、搜索方式与候选数量。",
    descriptionEn: "Configure symbol pools, market type, search mode, and candidate budgets.",
  },
  {
    key: "martingale",
    titleZh: "2. 马丁参数",
    titleEn: "2. Martingale parameters",
    descriptionZh: "定义方向、杠杆、间距、加仓、止盈和止损框架。",
    descriptionEn: "Define direction, leverage, spacing, sizing, take-profit, and stop-loss rules.",
  },
  {
    key: "indicator",
    titleZh: "3. 指标规则",
    titleEn: "3. Indicator rules",
    descriptionZh: "为 ATR、MA/EMA、RSI、Bollinger、ADX 设置过滤逻辑。",
    descriptionEn: "Set filter logic for ATR, MA/EMA, RSI, Bollinger, and ADX.",
  },
  {
    key: "time",
    titleZh: "4. 时间切分",
    titleEn: "4. Time splits",
    descriptionZh: "指定 Data ranges、walk-forward 和 stress windows。",
    descriptionEn: "Specify data ranges, walk-forward windows, and stress windows.",
  },
  {
    key: "risk",
    titleZh: "5. 风险与评分",
    titleEn: "5. Risk and scoring",
    descriptionZh: "在 Portfolio 维度配置生存优先筛选和发布门槛。",
    descriptionEn: "Configure survival-first filters and publish gates at the Portfolio level.",
  },
] as const;

const WIZARD_PAYLOAD = {
  strategy_type: "martingale_grid",
  symbol_pool: {
    mode: "all_usdt",
    whitelist: ["BTCUSDT", "ETHUSDT"],
    blacklist: [],
  },
  symbols: ["BTCUSDT", "ETHUSDT"],
  market: "usd_m_futures",
  direction_mode: "long_and_short",
  hedge_mode_required: true,
  margin_mode: "isolated",
  leverage_range: [2, 4],
  search: {
    mode: "intelligent",
    rounds: 4,
    candidate_budget: 160,
    top_n_refine: 20,
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
};

export function BacktestWizard({
  lang,
  onTaskCreated,
}: {
  lang: UiLanguage;
  onTaskCreated?: () => void | Promise<void>;
}) {
  const [feedback, setFeedback] = useState("");
  const [pending, setPending] = useState(false);

  async function createTask() {
    setPending(true);
    setFeedback(pickText(lang, "正在按向导配置创建回测任务…", "Creating a backtest task from the wizard configuration..."));
    const result = await requestBacktestApi("/api/user/backtest/tasks", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(WIZARD_PAYLOAD),
    });
    setPending(false);
    if (!result.ok) {
      setFeedback(result.message);
      return;
    }
    setFeedback(pickText(lang, "向导回测任务已创建，请等待 Worker 生成候选。", "Wizard backtest task created. Wait for the worker to produce candidates."));
    await onTaskCreated?.();
  }

  return (
    <div className="space-y-4">
      <div className="grid gap-3 md:grid-cols-5">
        {STEPS.map((step) => (
          <div className="rounded-xl border border-border bg-background p-3" key={step.key}>
            <p className="text-sm font-semibold">{pickText(lang, step.titleZh, step.titleEn)}</p>
            <p className="mt-1 text-xs text-muted-foreground">
              {pickText(lang, step.descriptionZh, step.descriptionEn)}
            </p>
          </div>
        ))}
      </div>

      <div className="rounded-xl border border-border bg-background p-4">
        <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
          <div>
            <p className="text-sm font-semibold">{pickText(lang, "向导默认任务", "Wizard default task")}</p>
            <p className="text-xs text-muted-foreground">
              {pickText(
                lang,
                "使用当前向导展示的混合马丁模板创建真实回测任务；高级参数可切到 Professional Console 改 JSON。",
                "Create a real backtest task from the wizard template; switch to Professional Console for advanced JSON edits.",
              )}
            </p>
          </div>
          <button
            className="rounded-full bg-primary px-4 py-2 text-sm font-medium text-primary-foreground disabled:opacity-60"
            disabled={pending}
            onClick={() => void createTask()}
            type="button"
          >
            {pickText(lang, "创建向导回测任务", "Create wizard backtest task")}
          </button>
        </div>
        <p aria-live="polite" className="mt-3 text-sm text-muted-foreground">{feedback}</p>
      </div>

      <div className="grid gap-4">
        <SearchConfigEditor lang={lang} />
        <MartingaleParameterEditor lang={lang} />
        <IndicatorRuleEditor lang={lang} />
        <TimeSplitEditor lang={lang} />
        <RiskRuleEditor lang={lang} />
      </div>
    </div>
  );
}
