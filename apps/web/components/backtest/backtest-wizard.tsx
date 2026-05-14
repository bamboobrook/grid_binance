"use client";

import { type ChangeEvent, type ReactNode, useState } from "react";
import { requestBacktestApi } from "@/components/backtest/request-client";
import { IndicatorRuleEditor } from "@/components/backtest/indicator-rule-editor";
import { MartingaleParameterEditor } from "@/components/backtest/martingale-parameter-editor";
import { MartingaleRiskWarning } from "@/components/backtest/martingale-risk-warning";
import { RiskRuleEditor } from "@/components/backtest/risk-rule-editor";
import { ScoringWeightEditor } from "@/components/backtest/scoring-weight-editor";
import { SearchConfigEditor } from "@/components/backtest/search-config-editor";
import { TimeSplitEditor } from "@/components/backtest/time-split-editor";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";

const DEFAULT_SYMBOLS = ["BTCUSDT", "ETHUSDT", "SOLUSDT", "BNBUSDT"];
const MAX_SYMBOLS = 20;
const DEFAULT_MAX_DRAWDOWN_BY_RISK = {
  conservative: 20,
  balanced: 25,
  aggressive: 30,
} as const;

const STEPS = [
  { key: "search", titleZh: "1. 市场与搜索", titleEn: "1. Market and search", descriptionZh: "配置 symbol 池、市场类型、搜索方式与候选数量。", descriptionEn: "Configure symbol pools, market type, search mode, and candidate budgets." },
  { key: "martingale", titleZh: "2. 马丁参数", titleEn: "2. Martingale parameters", descriptionZh: "定义方向、杠杆、间距、加仓、止盈和止损框架。", descriptionEn: "Define direction, leverage, spacing, sizing, take-profit, and stop-loss rules." },
  { key: "indicator", titleZh: "3. 指标规则", titleEn: "3. Indicator rules", descriptionZh: "为 ATR、MA/EMA、RSI、Bollinger、ADX 设置过滤逻辑。", descriptionEn: "Set filter logic for ATR, MA/EMA, RSI, Bollinger, and ADX." },
  { key: "time", titleZh: "4. 时间切分", titleEn: "4. Time splits", descriptionZh: "用日期选择器指定训练、验证、测试区间。", descriptionEn: "Use date pickers for train, validate, and test windows." },
  { key: "risk", titleZh: "5. 风险与评分", titleEn: "5. Risk and scoring", descriptionZh: "配置回撤、止损频率和发布门槛。", descriptionEn: "Configure drawdown, stop-loss frequency, and publish gates." },
] as const;

export type WizardForm = {
  symbolPoolMode: "all_usdt" | "whitelist" | "blacklist";
  whitelist: string;
  blacklist: string;
  searchMode: "random" | "intelligent";
  parameterPreset: "conservative" | "balanced" | "aggressive" | "custom";
  randomSeed: string;
  candidateBudget: string;
  intelligentRounds: string;
  topN: string;
  market: "spot" | "usd_m_futures";
  directionMode: "long_only" | "short_only" | "long_and_short";
  hedgeModeRequired: boolean;
  marginMode: "isolated" | "cross";
  minLeverage: string;
  maxLeverage: string;
  initialOrderUsdt: string;
  spacingPct: string;
  orderMultiplier: string;
  maxLegs: string;
  takeProfitPct: string;
  trailingPct: string;
  stopLossMode: "range" | "atr" | "portfolio_drawdown" | "strategy_drawdown";
  timeMode: "auto_recent" | "manual";
  trainStart: string;
  trainEnd: string;
  validateStart: string;
  validateEnd: string;
  testStart: string;
  testEnd: string;
  interval: "1m" | "5m" | "15m" | "1h" | "4h" | "1d";
  maxDrawdownPct: string;
  maxStopLossCount: string;
  portfolioStopLossPct: string;
  perStrategyStopLossPct: string;
};

const INITIAL_FORM: WizardForm = {
  symbolPoolMode: "whitelist",
  whitelist: "BTCUSDT, ETHUSDT",
  blacklist: "",
  searchMode: "intelligent",
  parameterPreset: "balanced",
  randomSeed: "20260509",
  candidateBudget: "160",
  intelligentRounds: "4",
  topN: "20",
  market: "usd_m_futures",
  directionMode: "long_and_short",
  hedgeModeRequired: true,
  marginMode: "isolated",
  minLeverage: "2",
  maxLeverage: "10",
  initialOrderUsdt: "10",
  spacingPct: "1",
  orderMultiplier: "2",
  maxLegs: "6",
  takeProfitPct: "1",
  trailingPct: "0.4",
  stopLossMode: "portfolio_drawdown",
  timeMode: "auto_recent",
  trainStart: "2023-01-01",
  trainEnd: "2024-12-31",
  validateStart: "2025-01-01",
  validateEnd: "2025-03-31",
  testStart: "2025-04-01",
  testEnd: "2025-06-30",
  interval: "1m",
  maxDrawdownPct: String(DEFAULT_MAX_DRAWDOWN_BY_RISK.balanced),
  maxStopLossCount: "3",
  portfolioStopLossPct: "18",
  perStrategyStopLossPct: "8",
};

export function BacktestWizard({ lang, onTaskCreated }: { lang: UiLanguage; onTaskCreated?: () => void | Promise<void> }) {
  const [form, setForm] = useState<WizardForm>(() => ({ ...INITIAL_FORM, ...resolveAutoTimeSplit() }));
  const [feedback, setFeedback] = useState("");
  const [pending, setPending] = useState(false);
  const [indicators, setIndicators] = useState<Record<string, unknown>>({});
  const [scoringWeights, setScoringWeights] = useState<Record<string, number> | null>(null);
  const [manualDrawdownOverride, setManualDrawdownOverride] = useState(false);

  function onChange(event: ChangeEvent<HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement>) {
    const { name, type, value } = event.currentTarget;
    const nextValue = type === "checkbox" ? (event.currentTarget as HTMLInputElement).checked : value;
    if (name === "maxDrawdownPct") {
      setManualDrawdownOverride(true);
    }
    setForm((current) => {
      const next = { ...current, [name]: nextValue };
      if (name === "parameterPreset" && !manualDrawdownOverride && isDefaultRiskProfile(nextValue)) {
        next.maxDrawdownPct = String(DEFAULT_MAX_DRAWDOWN_BY_RISK[nextValue]);
      }
      return next;
    });
  }

  async function createTask() {
    const symbols = symbolsForForm(form);
    if (symbols.length === 0) {
      setFeedback(pickText(lang, "请至少选择 1 个可回测币种。", "Select at least one symbol to backtest."));
      return;
    }
    if (symbols.length > MAX_SYMBOLS) {
      setFeedback(pickText(lang, "白名单最多支持 20 个币种，请减少后再启动。", "Whitelist supports up to 20 symbols. Reduce the list before starting."));
      return;
    }

    setPending(true);
    setFeedback(pickText(lang, "正在创建并启动回测任务…", "Creating and starting the backtest task..."));
    const result = await requestBacktestApi("/api/user/backtest/tasks", {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(buildWizardPayload(form, indicators, scoringWeights)),
    });
    setPending(false);
    if (!result.ok) {
      setFeedback(result.message);
      return;
    }
    setFeedback(pickText(lang, "回测任务已进入队列，请在下方任务列表查看进度。", "Backtest task queued. Check progress in the task list below."));
    await onTaskCreated?.();
  }

  return (
    <div className="space-y-4">
      <MartingaleRiskWarning lang={lang} />

      <div className="grid gap-3 md:grid-cols-5">
        {STEPS.map((step) => (
          <div className="rounded-xl border border-border bg-background p-3" key={step.key}>
            <p className="text-sm font-semibold">{pickText(lang, step.titleZh, step.titleEn)}</p>
            <p className="mt-1 text-xs text-muted-foreground">{pickText(lang, step.descriptionZh, step.descriptionEn)}</p>
          </div>
        ))}
      </div>

      <div className="rounded-xl border border-border bg-background p-4">
        <div className="flex flex-col gap-3 md:flex-row md:items-center md:justify-between">
          <div>
            <p className="text-sm font-semibold">{pickText(lang, "可编辑向导任务", "Editable wizard task")}</p>
            <p className="text-xs text-muted-foreground">
              {pickText(lang, "只需填写币种、市场、方向与风险档位，系统自动搜索每个币种 Top 10。", "Only fill symbols, market, direction, and risk profile; the system searches each symbol's Top 10 automatically.")}
            </p>
          </div>
          <button className="rounded-full bg-primary px-4 py-2 text-sm font-medium text-primary-foreground disabled:opacity-60" disabled={pending} onClick={() => void createTask()} type="button">
            {pending ? pickText(lang, "创建中…", "Creating...") : pickText(lang, "开始自动搜索 Top 10", "Start automatic Top 10 search")}
          </button>
        </div>
        <p aria-live="polite" className="mt-3 text-sm text-muted-foreground">{feedback}</p>
      </div>

      <div className="grid gap-4">
        <AutomaticSearchPanel form={form} lang={lang} onChange={onChange} />
        <details className="rounded-2xl border border-dashed border-border bg-card/60 p-4 text-muted-foreground">
          <summary className="cursor-pointer text-base font-semibold text-foreground">
            {pickText(lang, "高级参数搜索范围", "Advanced parameter search space")}
          </summary>
          <div className="mt-4 grid gap-4 opacity-90">
            <SearchConfigEditor form={form} lang={lang} onChange={onChange} />
            <MartingaleParameterEditor form={form} lang={lang} onChange={onChange} />
            <IndicatorRuleEditor lang={lang} onChange={setIndicators} />
            <TimeSplitEditor form={form} lang={lang} onChange={onChange} />
            <RiskRuleEditor form={form} lang={lang} onChange={onChange} />
            <ScoringWeightEditor lang={lang} onChange={setScoringWeights} />
          </div>
        </details>
      </div>
    </div>
  );
}

function AutomaticSearchPanel({ form, lang, onChange }: { form: WizardForm; lang: UiLanguage; onChange: (event: ChangeEvent<HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement>) => void }) {
  return (
    <section className="rounded-2xl border border-primary/20 bg-card p-4 shadow-sm">
      <div className="mb-4">
        <h3 className="text-lg font-semibold">{pickText(lang, "自动搜索向导", "Automatic search wizard")}</h3>
        <p className="text-sm text-muted-foreground">{pickText(lang, "默认提交 automatic search payload；高级参数已折叠，可按风险档位自动生成搜索范围。", "Submits an automatic search payload by default; advanced parameters stay collapsed and are generated from the risk profile.")}</p>
      </div>
      <div className="grid gap-4 lg:grid-cols-2">
        <label className="flex flex-col gap-1 text-sm lg:col-span-2">
          <span className="text-xs uppercase tracking-wide text-muted-foreground">whitelist · max 20</span>
          <textarea className="min-h-20 rounded-lg border border-border bg-background px-3 py-2" name="whitelist" onChange={onChange} placeholder="BTCUSDT, ETHUSDT" value={form.whitelist} />
        </label>
        <label className="flex flex-col gap-1 text-sm lg:col-span-2">
          <span className="text-xs uppercase tracking-wide text-muted-foreground">{pickText(lang, "blacklist（可选）", "blacklist (optional)")}</span>
          <textarea className="min-h-16 rounded-lg border border-border bg-background px-3 py-2 text-muted-foreground" name="blacklist" onChange={onChange} placeholder="DOGEUSDT, PEPEUSDT" value={form.blacklist} />
        </label>
        <WizardSelect label={pickText(lang, "市场", "Market")} name="market" onChange={onChange} value={form.market}>
          <option value="spot">Spot</option>
          <option value="usd_m_futures">USDT-M Futures</option>
        </WizardSelect>
        <WizardSelect label={pickText(lang, "方向", "Direction")} name="directionMode" onChange={onChange} value={form.directionMode}>
          <option value="long_only">Long</option>
          <option value="short_only">Short</option>
          <option value="long_and_short">Long + Short</option>
        </WizardSelect>
        <WizardSelect label={pickText(lang, "风险档位", "Risk profile")} name="parameterPreset" onChange={onChange} value={form.parameterPreset}>
          <option value="conservative">{pickText(lang, "保守", "Conservative")}</option>
          <option value="balanced">{pickText(lang, "均衡", "Balanced")}</option>
          <option value="aggressive">{pickText(lang, "激进", "Aggressive")}</option>
          <option value="custom">{pickText(lang, "手动", "Custom")}</option>
        </WizardSelect>
        <label className="flex flex-col gap-1 text-sm">
          <span className="text-xs uppercase tracking-wide text-muted-foreground">{pickText(lang, "最大回撤限制（%）", "Max drawdown limit (%)")}</span>
          <input className="rounded-lg border border-border bg-background px-3 py-2" min="1" name="maxDrawdownPct" onChange={onChange} step="0.5" type="number" value={form.maxDrawdownPct} />
          <span className="text-xs text-muted-foreground">{pickText(lang, "最大回撤是发布前硬约束（hard constraint）；搜索会优先只保留低于该回撤的候选，若没有候选达标，会展示最接近的结果并提示风险。", "Max drawdown is a pre-publish hard constraint; search first keeps candidates under this drawdown, and if none qualify it shows the closest high-risk results.")}</span>
        </label>
        <div className="rounded-xl border border-emerald-500/20 bg-emerald-500/5 p-3 text-sm">
          <p className="text-xs uppercase tracking-wide text-muted-foreground">{pickText(lang, "自动时间范围", "Automatic time range")}</p>
          <p className="mt-1 font-semibold">{form.trainStart} → {form.testEnd}</p>
          <p className="mt-1 text-xs text-muted-foreground">{pickText(lang, "固定从 2023-01-01 开始，结束于浏览器当前日期的上个月月底。", "Fixed from 2023-01-01 through the previous month end from the browser's current date.")}</p>
        </div>
      </div>
    </section>
  );
}

function WizardSelect({ children, label, name, onChange, value }: { children: ReactNode; label: string; name: keyof WizardForm; onChange: (event: ChangeEvent<HTMLSelectElement>) => void; value: string }) {
  return <label className="flex flex-col gap-1 text-sm"><span className="text-xs uppercase tracking-wide text-muted-foreground">{label}</span><select className="rounded-lg border border-border bg-background px-3 py-2" name={name} onChange={onChange} value={value}>{children}</select></label>;
}

export function parseSymbolList(value: string) {
  return Array.from(new Set(value.split(/[\s,，;；]+/).map((item) => item.trim().toUpperCase()).filter(Boolean)));
}

export function symbolsForForm(form: WizardForm) {
  const whitelist = parseSymbolList(form.whitelist);
  const blacklist = new Set(parseSymbolList(form.blacklist));
  const source = form.symbolPoolMode === "whitelist" ? whitelist : DEFAULT_SYMBOLS;
  return source.filter((symbol) => !blacklist.has(symbol));
}

function isDefaultRiskProfile(profile: unknown): profile is keyof typeof DEFAULT_MAX_DRAWDOWN_BY_RISK {
  return profile === "conservative" || profile === "balanced" || profile === "aggressive";
}

function numberValue(value: string, fallback: number) {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}

function integerValue(value: string, fallback: number) {
  return Math.max(1, Math.round(numberValue(value, fallback)));
}

function percentToBps(value: string, fallback: number) {
  return Math.max(0, Math.round(numberValue(value, fallback) * 100));
}

function dateToMs(value: string, endOfDay = false) {
  const suffix = endOfDay ? "T23:59:59.999Z" : "T00:00:00.000Z";
  const ms = Date.parse(`${value}${suffix}`);
  return Number.isFinite(ms) ? ms : 0;
}

export function resolveAutomaticTimeRange(now = new Date()) {
  return {
    trainStart: "2023-01-01",
    testEnd: formatLocalDate(new Date(now.getFullYear(), now.getMonth(), 0)),
  };
}

export function resolveAutoTimeSplit(now = new Date()) {
  const { trainStart, testEnd } = resolveAutomaticTimeRange(now);
  const trainStartDate = new Date(`${trainStart}T00:00:00.000Z`);
  const lastDayOfPreviousMonth = new Date(`${testEnd}T00:00:00.000Z`);
  const totalDays = daysBetweenInclusive(trainStartDate, lastDayOfPreviousMonth);
  const trainDays = Math.max(1, Math.floor(totalDays * 0.7));
  const validateDays = Math.max(1, Math.floor(totalDays * 0.15));
  const trainEnd = addDays(trainStartDate, trainDays - 1);
  const validateStart = addDays(trainEnd, 1);
  const validateEnd = addDays(validateStart, validateDays - 1);
  const testStart = addDays(validateEnd, 1);
  return {
    trainStart,
    trainEnd: formatDate(trainEnd),
    validateStart: formatDate(validateStart),
    validateEnd: formatDate(validateEnd),
    testStart: formatDate(testStart),
    testEnd,
  };
}

function addDays(date: Date, days: number) {
  const next = new Date(date);
  next.setUTCDate(next.getUTCDate() + days);
  return next;
}

function formatDate(date: Date) {
  return date.toISOString().slice(0, 10);
}

function formatLocalDate(date: Date) {
  const year = date.getFullYear();
  const month = `${date.getMonth() + 1}`.padStart(2, "0");
  const day = `${date.getDate()}`.padStart(2, "0");
  return `${year}-${month}-${day}`;
}

function daysBetweenInclusive(start: Date, end: Date) {
  const msPerDay = 24 * 60 * 60 * 1000;
  return Math.max(1, Math.floor((end.getTime() - start.getTime()) / msPerDay) + 1);
}

export function presetSearchSpaces(form: WizardForm) {
  const customLeverage = [integerValue(form.minLeverage, 1), integerValue(form.maxLeverage, integerValue(form.minLeverage, 1))];
  const custom = {
    spacing_bps: [percentToBps(form.spacingPct, 1)],
    first_order_quote: [numberValue(form.initialOrderUsdt, 10)],
    order_multiplier: [numberValue(form.orderMultiplier, 2)],
    take_profit_bps: [percentToBps(form.takeProfitPct, 1)],
    max_legs: [integerValue(form.maxLegs, 6)],
    leverage: form.market === "spot" ? [] : Array.from(new Set(customLeverage)).sort((left, right) => left - right),
  };
  if (form.parameterPreset === "conservative") {
    return {
      spacing_bps: [120, 160, 220, 300],
      first_order_quote: [numberValue(form.initialOrderUsdt, 10)],
      order_multiplier: [1.25, 1.4, 1.6],
      take_profit_bps: [60, 80, 100],
      max_legs: [3, 4, 5],
      leverage: form.market === "spot" ? [] : Array.from({ length: 9 }, (_, index) => index + 2),
    };
  }
  if (form.parameterPreset === "aggressive") {
    return {
      spacing_bps: [50, 80, 120, 160],
      first_order_quote: [numberValue(form.initialOrderUsdt, 10)],
      order_multiplier: [1.6, 2, 2.4],
      take_profit_bps: [100, 130, 180],
      max_legs: [5, 6, 8],
      leverage: form.market === "spot" ? [] : Array.from({ length: 9 }, (_, index) => index + 2),
    };
  }
  if (form.parameterPreset === "balanced") {
    return {
      spacing_bps: [80, 120, 160, 220],
      first_order_quote: [numberValue(form.initialOrderUsdt, 10)],
      order_multiplier: [1.4, 1.6, 2],
      take_profit_bps: [80, 100, 130],
      max_legs: [4, 5, 6],
      leverage: form.market === "spot" ? [] : Array.from({ length: 9 }, (_, index) => index + 2),
    };
  }
  return custom;
}

function riskProfileDefaults(profile: WizardForm["parameterPreset"]) {
  if (profile === "conservative") {
    return { maxDrawdownPct: DEFAULT_MAX_DRAWDOWN_BY_RISK.conservative, maxStopLossCount: 1, perStrategyStopLossPct: 6 };
  }
  if (profile === "aggressive") {
    return { maxDrawdownPct: DEFAULT_MAX_DRAWDOWN_BY_RISK.aggressive, maxStopLossCount: 8, perStrategyStopLossPct: 18 };
  }
  if (profile === "balanced") {
    return { maxDrawdownPct: DEFAULT_MAX_DRAWDOWN_BY_RISK.balanced, maxStopLossCount: 3, perStrategyStopLossPct: 10 };
  }
  return {
    maxDrawdownPct: numberValue(INITIAL_FORM.maxDrawdownPct, 18),
    maxStopLossCount: integerValue(INITIAL_FORM.maxStopLossCount, 3),
    perStrategyStopLossPct: numberValue(INITIAL_FORM.perStrategyStopLossPct, 8),
  };
}

export function buildWizardPayload(form: WizardForm, indicators?: Record<string, unknown>, scoringWeights?: Record<string, number> | null) {
  const symbols = symbolsForForm(form);
  const minLeverage = integerValue(form.minLeverage, 1);
  const maxLeverage = integerValue(form.maxLeverage, minLeverage);
  const leverageRange = [Math.min(minLeverage, maxLeverage), Math.max(minLeverage, maxLeverage)];
  const timeSplit = form.timeMode === "auto_recent" ? resolveAutoTimeSplit() : form;
  const searchSpace = presetSearchSpaces(form);
  const riskPreset = riskProfileDefaults(form.parameterPreset);
  const market = form.market;
  const leverage = market === "spot" ? null : leverageRange[0];
  const marginMode = market === "spot" ? null : form.marginMode;
  const maxDrawdownPct = numberValue(form.maxDrawdownPct, riskPreset.maxDrawdownPct);
  const existingScoring = {
    profile: "survival_first",
    max_stop_loss_count: integerValue(form.maxStopLossCount, riskPreset.maxStopLossCount),
    ...(scoringWeights ? { weights: scoringWeights } : {}),
  };

  const indicatorConfigs = indicatorConfigsForPayload(indicators);
  const entryTriggers = entryTriggersForPayload(indicators);

  return {
    strategy_type: "martingale_grid",
    time_range_mode: "auto_previous_month_end",
    symbols,
    risk_profile: form.parameterPreset,
    per_symbol_top_n: 10,
    portfolio_top_n: 10,
    dynamic_allocation_enabled: form.directionMode === "long_and_short",
    search_space_mode: "risk_profile_auto",
    train_start: "2023-01-01",
    test_end: timeSplit.testEnd,
    random_seed: integerValue(form.randomSeed, 1),
    random_candidates: integerValue(form.candidateBudget, 16),
    intelligent_rounds: form.searchMode === "intelligent" ? integerValue(form.intelligentRounds, 2) : 1,
    top_n: integerValue(form.topN, 10),
    interval: form.interval,
    start_ms: dateToMs(timeSplit.trainStart),
    end_ms: dateToMs(timeSplit.testEnd, true),
    symbol_pool: {
      mode: form.symbolPoolMode,
      whitelist: parseSymbolList(form.whitelist),
      blacklist: parseSymbolList(form.blacklist),
      effective_symbols: symbols,
    },
    market,
    direction_mode: form.directionMode,
    hedge_mode_required: form.hedgeModeRequired,
    margin_mode: marginMode,
    leverage_range: leverageRange,
    search: {
      mode: form.searchMode,
      rounds: integerValue(form.intelligentRounds, 2),
      candidate_budget: integerValue(form.candidateBudget, 16),
      top_n_refine: integerValue(form.topN, 10),
      random_seed: integerValue(form.randomSeed, 1),
    },
    search_space: searchSpace,
    martingale_template: {
      spacing: { model: "fixed_percent", step_bps: percentToBps(form.spacingPct, 1) },
      sizing: {
        model: "multiplier",
        first_order_quote: numberValue(form.initialOrderUsdt, 10),
        multiplier: numberValue(form.orderMultiplier, 2),
        max_legs: integerValue(form.maxLegs, 6),
      },
      indicators: indicatorConfigs,
      entry_triggers: entryTriggers,
      search_space: searchSpace,
      take_profit: { model: "percent", bps: percentToBps(form.takeProfitPct, 1) },
      trailing_take_profit: { retracement_bps: percentToBps(form.trailingPct, 0.4) },
      stop_loss: {
        mode: form.stopLossMode,
        portfolio_stop_loss_bps: percentToBps(form.portfolioStopLossPct, riskPreset.maxDrawdownPct),
        per_strategy_stop_loss_bps: percentToBps(form.perStrategyStopLossPct, riskPreset.perStrategyStopLossPct),
      },
    },
    portfolio_config: {
      direction_mode: form.directionMode,
      strategies: symbols.map((symbol, index) => ({
        strategy_id: `wizard-${symbol.toLowerCase()}-${index + 1}`,
        symbol,
        market,
        direction: form.directionMode === "short_only" ? "short" : "long",
        direction_mode: form.directionMode,
        margin_mode: marginMode,
        leverage,
        spacing: { fixed_percent: { step_bps: percentToBps(form.spacingPct, 1) } },
        sizing: {
          multiplier: {
            first_order_quote: numberValue(form.initialOrderUsdt, 10).toString(),
            multiplier: numberValue(form.orderMultiplier, 2).toString(),
            max_legs: integerValue(form.maxLegs, 6),
          },
        },
        take_profit: { percent: { bps: percentToBps(form.takeProfitPct, 1) } },
        stop_loss: { strategy_drawdown_pct: { pct_bps: percentToBps(form.perStrategyStopLossPct, riskPreset.perStrategyStopLossPct) } },
        indicators: indicatorConfigs,
        entry_triggers: entryTriggers,
        risk_limits: {},
      })),
      risk_limits: {
        max_global_drawdown_quote: null,
      },
    },
    portfolio_basket: {
      mode: "manual_selection_after_backtest",
      weight_total_pct: 100,
      selection: [],
    },
    time_split: {
      mode: form.timeMode === "auto_recent" ? "auto_previous_month_end" : "manual",
      generated_at: formatDate(new Date()),
      train: { start: timeSplit.trainStart, end: timeSplit.trainEnd },
      validate: { start: timeSplit.validateStart, end: timeSplit.validateEnd },
      test: { start: timeSplit.testStart, end: timeSplit.testEnd },
      stress_windows: ["flash_crash", "trend_up", "trend_down", "high_volatility"],
    },
    scoring: {
      ...existingScoring,
      max_drawdown_pct: maxDrawdownPct,
    },
  };
}

function indicatorConfigsForPayload(indicators?: Record<string, unknown>) {
  if (!indicators || Object.keys(indicators).length === 0) return [];
  const configs: Array<Record<string, unknown>> = [];
  const source = indicators as Record<string, Record<string, unknown>>;
  if (source.atr) configs.push({ atr: { period: integerFromUnknown(source.atr.period, 14) } });
  if (source.sma) configs.push({ sma: { period: integerFromUnknown(source.sma.slow_period ?? source.sma.period, 25) } });
  if (source.ema) configs.push({ ema: { period: integerFromUnknown(source.ema.slow_period ?? source.ema.period, 26) } });
  if (source.rsi) {
    configs.push({
      rsi: {
        period: integerFromUnknown(source.rsi.period, 14),
        overbought: numberFromUnknown(source.rsi.overbought, 70).toString(),
        oversold: numberFromUnknown(source.rsi.oversold, 30).toString(),
      },
    });
  }
  if (source.bollinger) {
    configs.push({
      bollinger: {
        period: integerFromUnknown(source.bollinger.period, 20),
        std_dev: numberFromUnknown(source.bollinger.std_dev, 2).toString(),
      },
    });
  }
  if (source.adx) configs.push({ adx: { period: integerFromUnknown(source.adx.period, 14) } });
  return configs;
}

function entryTriggersForPayload(indicators?: Record<string, unknown>) {
  if (!indicators || Object.keys(indicators).length === 0) return ["immediate"];
  const source = indicators as Record<string, Record<string, unknown>>;
  const expressions: string[] = [];
  if (source.sma) {
    expressions.push(`sma(${integerFromUnknown(source.sma.fast_period, 7)}) >= sma(${integerFromUnknown(source.sma.slow_period, 25)})`);
  }
  if (source.ema) {
    expressions.push(`ema(${integerFromUnknown(source.ema.fast_period, 12)}) >= ema(${integerFromUnknown(source.ema.slow_period, 26)})`);
  }
  if (source.rsi) {
    expressions.push(`rsi(${integerFromUnknown(source.rsi.period, 14)}) <= ${numberFromUnknown(source.rsi.overbought, 70)}`);
  }
  if (source.bollinger) {
    expressions.push(`close <= bb_upper(${integerFromUnknown(source.bollinger.period, 20)},${numberFromUnknown(source.bollinger.std_dev, 2)})`);
  }
  if (source.adx) {
    expressions.push(`adx(${integerFromUnknown(source.adx.period, 14)}) >= ${numberFromUnknown(source.adx.threshold, 25)}`);
  }
  return expressions.length > 0 ? expressions.map((expression) => ({ indicator_expression: { expression } })) : ["immediate"];
}

function integerFromUnknown(value: unknown, fallback: number) {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? Math.max(1, Math.round(parsed)) : fallback;
}

function numberFromUnknown(value: unknown, fallback: number) {
  const parsed = Number(value);
  return Number.isFinite(parsed) ? parsed : fallback;
}
