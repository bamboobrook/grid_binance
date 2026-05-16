"use client";

import { useCallback, useEffect, useState } from "react";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";

/* ------------------------------------------------------------------ */
/*  Indicator parameter definitions                                   */
/* ------------------------------------------------------------------ */

type IndicatorKind = "atr" | "sma" | "ema" | "rsi" | "bollinger" | "adx";

interface IndicatorParamDef {
  key: string;
  labelZh: string;
  labelEn: string;
  type: "number" | "select";
  min?: number;
  max?: number;
  step?: number;
  defaultValue: number | string;
  options?: { value: string; labelZh: string; labelEn: string }[];
}

interface IndicatorDef {
  kind: IndicatorKind;
  labelZh: string;
  labelEn: string;
  descriptionZh: string;
  descriptionEn: string;
  params: IndicatorParamDef[];
}

const INDICATOR_DEFS: IndicatorDef[] = [
  {
    kind: "atr",
    labelZh: "ATR (平均真实波幅)",
    labelEn: "ATR (Average True Range)",
    descriptionZh: "衡量市场波动性，常用于动态补仓间隔和止损",
    descriptionEn: "Measures market volatility, commonly used for dynamic spacing and stop-loss",
    params: [
      { key: "period", labelZh: "周期", labelEn: "Period", type: "number", min: 2, max: 200, step: 1, defaultValue: 14 },
    ],
  },
  {
    kind: "sma",
    labelZh: "SMA (简单移动平均)",
    labelEn: "SMA (Simple Moving Average)",
    descriptionZh: "价格趋势判断，金叉/死叉信号",
    descriptionEn: "Price trend identification, golden/death cross signals",
    params: [
      { key: "fast_period", labelZh: "快线周期", labelEn: "Fast period", type: "number", min: 2, max: 500, step: 1, defaultValue: 7 },
      { key: "slow_period", labelZh: "慢线周期", labelEn: "Slow period", type: "number", min: 2, max: 500, step: 1, defaultValue: 25 },
    ],
  },
  {
    kind: "ema",
    labelZh: "EMA (指数移动平均)",
    labelEn: "EMA (Exponential Moving Average)",
    descriptionZh: "对近期价格更敏感的趋势指标",
    descriptionEn: "Trend indicator more responsive to recent prices",
    params: [
      { key: "fast_period", labelZh: "快线周期", labelEn: "Fast period", type: "number", min: 2, max: 500, step: 1, defaultValue: 12 },
      { key: "slow_period", labelZh: "慢线周期", labelEn: "Slow period", type: "number", min: 2, max: 500, step: 1, defaultValue: 26 },
    ],
  },
  {
    kind: "rsi",
    labelZh: "RSI (相对强弱指数)",
    labelEn: "RSI (Relative Strength Index)",
    descriptionZh: "超买超卖判断，常用于入场条件",
    descriptionEn: "Overbought/oversold detection, commonly used for entry conditions",
    params: [
      { key: "period", labelZh: "周期", labelEn: "Period", type: "number", min: 2, max: 100, step: 1, defaultValue: 14 },
      { key: "overbought", labelZh: "超买阈值", labelEn: "Overbought", type: "number", min: 50, max: 100, step: 1, defaultValue: 70 },
      { key: "oversold", labelZh: "超卖阈值", labelEn: "Oversold", type: "number", min: 0, max: 50, step: 1, defaultValue: 30 },
    ],
  },
  {
    kind: "bollinger",
    labelZh: "Bollinger Bands (布林带)",
    labelEn: "Bollinger Bands",
    descriptionZh: "价格通道判断，带宽收缩/扩张信号",
    descriptionEn: "Price channel analysis, band width squeeze/expansion signals",
    params: [
      { key: "period", labelZh: "周期", labelEn: "Period", type: "number", min: 2, max: 200, step: 1, defaultValue: 20 },
      { key: "std_dev", labelZh: "标准差倍数", labelEn: "Std dev multiplier", type: "number", min: 0.5, max: 5, step: 0.1, defaultValue: 2 },
    ],
  },
  {
    kind: "adx",
    labelZh: "ADX (平均方向指数)",
    labelEn: "ADX (Average Directional Index)",
    descriptionZh: "趋势强度判断，区分趋势/震荡行情",
    descriptionEn: "Trend strength measurement, distinguishes trending/ranging markets",
    params: [
      { key: "period", labelZh: "周期", labelEn: "Period", type: "number", min: 2, max: 100, step: 1, defaultValue: 14 },
      { key: "threshold", labelZh: "趋势阈值", labelEn: "Trend threshold", type: "number", min: 0, max: 100, step: 1, defaultValue: 25 },
    ],
  },
];

/* ------------------------------------------------------------------ */
/*  State shape                                                       */
/* ------------------------------------------------------------------ */

interface IndicatorState {
  enabled: boolean;
  params: Record<string, number | string>;
}

type IndicatorStates = Record<IndicatorKind, IndicatorState>;

function buildDefaultStates(): IndicatorStates {
  const states: Partial<IndicatorStates> = {};
  for (const def of INDICATOR_DEFS) {
    const params: Record<string, number | string> = {};
    for (const p of def.params) {
      params[p.key] = p.defaultValue;
    }
    states[def.kind] = { enabled: false, params };
  }
  return states as IndicatorStates;
}

/* ------------------------------------------------------------------ */
/*  Component                                                         */
/* ------------------------------------------------------------------ */

export interface IndicatorRuleEditorProps {
  lang: UiLanguage;
  value?: Record<string, unknown> | null;
  onChange?: (indicators: Record<string, unknown>) => void;
}

export function IndicatorRuleEditor({ lang, value: _externalValue, onChange }: IndicatorRuleEditorProps) {
  const [states, setStates] = useState<IndicatorStates>(buildDefaultStates);

  // Sync from external value on mount
  useEffect(() => {
    if (!_externalValue || typeof _externalValue !== "object") return;
    setStates((prev) => {
      const next = { ...prev };
      for (const def of INDICATOR_DEFS) {
        const ext = _externalValue[def.kind];
        if (ext && typeof ext === "object" && !Array.isArray(ext)) {
          const obj = ext as Record<string, unknown>;
          const filteredParams: Record<string, number | string> = {};
          for (const [k, v] of Object.entries(obj)) {
            if (k !== "enabled" && (typeof v === "number" || typeof v === "string")) {
              filteredParams[k] = v;
            }
          }
          next[def.kind] = {
            enabled: typeof obj.enabled === "boolean" ? obj.enabled : prev[def.kind].enabled,
            params: { ...prev[def.kind].params, ...filteredParams },
          };
        }
      }
      return next;
    });
  }, [_externalValue]);

  const emitChange = useCallback(
    (next: IndicatorStates) => {
      if (!onChange) return;
      const payload: Record<string, unknown> = {};
      for (const def of INDICATOR_DEFS) {
        const state = next[def.kind];
        if (state.enabled) {
          payload[def.kind] = { enabled: true, ...state.params };
        }
      }
      onChange(payload);
    },
    [onChange],
  );

  const toggleIndicator = useCallback(
    (kind: IndicatorKind) => {
      setStates((prev) => {
        const next = { ...prev, [kind]: { ...prev[kind], enabled: !prev[kind].enabled } };
        emitChange(next);
        return next;
      });
    },
    [emitChange],
  );

  const updateParam = useCallback(
    (kind: IndicatorKind, paramKey: string, rawValue: string) => {
      setStates((prev) => {
        const parsed = Number(rawValue);
        const val = Number.isFinite(parsed) ? parsed : rawValue;
        const next = {
          ...prev,
          [kind]: { ...prev[kind], params: { ...prev[kind].params, [paramKey]: val } },
        };
        emitChange(next);
        return next;
      });
    },
    [emitChange],
  );

  return (
    <div className="space-y-3">
      {INDICATOR_DEFS.map((def) => {
        const state = states[def.kind];
        return (
          <div
            key={def.kind}
            className={`rounded-lg border p-3 transition-colors ${
              state.enabled ? "border-primary/40 bg-primary/5" : "border-border bg-background"
            }`}
          >
            <div className="flex items-start gap-3">
              <input
                checked={state.enabled}
                className="mt-1 h-4 w-4 shrink-0 rounded border-border"
                id={`indicator-${def.kind}`}
                onChange={() => toggleIndicator(def.kind)}
                type="checkbox"
              />
              <div className="flex-1 min-w-0">
                <label className="text-sm font-medium cursor-pointer" htmlFor={`indicator-${def.kind}`}>
                  {pickText(lang, def.labelZh, def.labelEn)}
                </label>
                <p className="text-xs text-muted-foreground mt-0.5">
                  {pickText(lang, def.descriptionZh, def.descriptionEn)}
                </p>
                {state.enabled && (
                  <div className="mt-2 grid grid-cols-2 gap-2 sm:grid-cols-3">
                    {def.params.map((p) => (
                      <div key={p.key} className="space-y-1">
                        <label className="text-xs text-muted-foreground" htmlFor={`ind-${def.kind}-${p.key}`}>
                          {pickText(lang, p.labelZh, p.labelEn)}
                        </label>
                        {p.type === "select" && p.options ? (
                          <select
                            className="w-full rounded-md border border-border bg-background px-2 py-1 text-sm"
                            id={`ind-${def.kind}-${p.key}`}
                            onChange={(e) => updateParam(def.kind, p.key, e.target.value)}
                            value={String(state.params[p.key] ?? p.defaultValue)}
                          >
                            {p.options.map((opt) => (
                              <option key={opt.value} value={opt.value}>
                                {pickText(lang, opt.labelZh, opt.labelEn)}
                              </option>
                            ))}
                          </select>
                        ) : (
                          <input
                            className="w-full rounded-md border border-border bg-background px-2 py-1 text-sm"
                            id={`ind-${def.kind}-${p.key}`}
                            max={p.max}
                            min={p.min}
                            onChange={(e) => updateParam(def.kind, p.key, e.target.value)}
                            step={p.step ?? 1}
                            type="number"
                            value={state.params[p.key] ?? p.defaultValue}
                          />
                        )}
                      </div>
                    ))}
                  </div>
                )}
              </div>
            </div>
          </div>
        );
      })}
    </div>
  );
}
