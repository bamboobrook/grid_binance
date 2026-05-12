"use client";

import { type ChangeEvent, useCallback, useState } from "react";
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
  maxLeverage: "4",
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
  interval: "1h",
  maxDrawdownPct: "18",
  maxStopLossCount: "3",
  portfolioStopLossPct: "18",
  perStrategyStopLossPct: "8",
};

export function BacktestWizard({ lang, onTaskCreated }: { lang: UiLanguage; onTaskCreated?: () => void | Promise<void> }) {
  const [form, setForm] = useState<WizardForm>(INITIAL_FORM);
  const [feedback, setFeedback] = useState("");
  const [pending, setPending] = useState(false);
  const [indicators, setIndicators] = useState<Record<string, unknown>>({});
  const [scoringWeights, setScoringWeights] = useState<Record<string, number> | null>(null);

  function onChange(event: ChangeEvent<HTMLInputElement | HTMLSelectElement | HTMLTextAreaElement>) {
    const { name, type, value } = event.currentTarget;
    const nextValue = type === "checkbox" ? (event.currentTarget as HTMLInputElement).checked : value;
    setForm((current) => ({ ...current, [name]: nextValue }));
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
              {pickText(lang, "按下面表单设置币种、马丁参数、时间区间和风险规则，然后启动真实回测。", "Set symbols, martingale parameters, time ranges, and risk rules below, then start a real backtest.")}
            </p>
          </div>
          <button className="rounded-full bg-primary px-4 py-2 text-sm font-medium text-primary-foreground disabled:opacity-60" disabled={pending} onClick={() => void createTask()} type="button">
            {pending ? pickText(lang, "创建中…", "Creating...") : pickText(lang, "启动回测", "Start backtest")}
          </button>
        </div>
        <p aria-live="polite" className="mt-3 text-sm text-muted-foreground">{feedback}</p>
      </div>

      <div className="grid gap-4">
        <SearchConfigEditor form={form} lang={lang} onChange={onChange} />
        <MartingaleParameterEditor form={form} lang={lang} onChange={onChange} />
        <IndicatorRuleEditor lang={lang} onChange={setIndicators} />
        <TimeSplitEditor form={form} lang={lang} onChange={onChange} />
        <RiskRuleEditor form={form} lang={lang} onChange={onChange} />
        <ScoringWeightEditor lang={lang} onChange={setScoringWeights} />
      </div>
    </div>
  );
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

export function resolveAutoTimeSplit(now = new Date()) {
  const end = new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth(), now.getUTCDate()));
  const start = addDays(end, -365);
  const trainEnd = addDays(start, 255);
  const validateEnd = addDays(trainEnd, 55);
  return {
    trainStart: formatDate(start),
    trainEnd: formatDate(trainEnd),
    validateStart: formatDate(addDays(trainEnd, 1)),
    validateEnd: formatDate(validateEnd),
    testStart: formatDate(addDays(validateEnd, 1)),
    testEnd: formatDate(end),
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

export function presetSearchSpaces(form: WizardForm) {
  const custom = {
    spacing_bps: [percentToBps(form.spacingPct, 1)],
    first_order_quote: [numberValue(form.initialOrderUsdt, 10)],
    order_multiplier: [numberValue(form.orderMultiplier, 2)],
    take_profit_bps: [percentToBps(form.takeProfitPct, 1)],
    max_legs: [integerValue(form.maxLegs, 6)],
  };
  if (form.parameterPreset === "conservative") {
    return { spacing_bps: [100, 150, 200], first_order_quote: [10, 15], order_multiplier: [1.2, 1.4], take_profit_bps: [60, 80], max_legs: [4, 5] };
  }
  if (form.parameterPreset === "aggressive") {
    return { spacing_bps: [50, 75, 100], first_order_quote: [10, 25, 50], order_multiplier: [1.8, 2, 2.4], take_profit_bps: [100, 140, 200], max_legs: [6, 8, 10] };
  }
  if (form.parameterPreset === "balanced") {
    return { spacing_bps: [75, 100, 125], first_order_quote: [10, 20], order_multiplier: [1.4, 1.6, 2], take_profit_bps: [80, 100, 120], max_legs: [5, 6, 7] };
  }
  return custom;
}

export function buildWizardPayload(form: WizardForm, indicators?: Record<string, unknown>, scoringWeights?: Record<string, number> | null) {
  const symbols = symbolsForForm(form);
  const minLeverage = integerValue(form.minLeverage, 1);
  const maxLeverage = integerValue(form.maxLeverage, minLeverage);
  const leverageRange = [Math.min(minLeverage, maxLeverage), Math.max(minLeverage, maxLeverage)];
  const timeSplit = form.timeMode === "auto_recent" ? resolveAutoTimeSplit() : form;
  const searchSpace = presetSearchSpaces(form);
  const market = form.market;
  const leverage = market === "spot" ? null : leverageRange[0];
  const marginMode = market === "spot" ? null : form.marginMode;

  const indicatorConfigs = indicatorConfigsForPayload(indicators);
  const entryTriggers = entryTriggersForPayload(indicators);

  return {
    strategy_type: "martingale_grid",
    symbols,
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
        portfolio_stop_loss_bps: percentToBps(form.portfolioStopLossPct, 18),
        per_strategy_stop_loss_bps: percentToBps(form.perStrategyStopLossPct, 8),
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
        stop_loss: { strategy_drawdown_pct: { pct_bps: percentToBps(form.perStrategyStopLossPct, 8) } },
        indicators: indicatorConfigs,
        entry_triggers: entryTriggers,
        risk_limits: {},
      })),
      risk_limits: {
        max_global_drawdown_quote: null,
      },
    },
    time_split: {
      mode: form.timeMode,
      generated_at: formatDate(new Date()),
      train: { start: timeSplit.trainStart, end: timeSplit.trainEnd },
      validate: { start: timeSplit.validateStart, end: timeSplit.validateEnd },
      test: { start: timeSplit.testStart, end: timeSplit.testEnd },
      stress_windows: ["flash_crash", "trend_up"],
    },
    scoring: {
      profile: "survival_first",
      max_drawdown_pct: numberValue(form.maxDrawdownPct, 18),
      max_stop_loss_count: integerValue(form.maxStopLossCount, 3),
      ...(scoringWeights ? { weights: scoringWeights } : {}),
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
