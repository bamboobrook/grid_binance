"use client";

import { useCallback, useEffect, useState } from "react";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";

/* ------------------------------------------------------------------ */
/*  Weight definitions                                                */
/* ------------------------------------------------------------------ */

interface WeightDef {
  key: string;
  labelZh: string;
  labelEn: string;
  descriptionZh: string;
  descriptionEn: string;
  min: number;
  max: number;
  step: number;
  defaultValue: number;
}

const WEIGHT_DEFS: WeightDef[] = [
  {
    key: "weight_return",
    labelZh: "收益率权重",
    labelEn: "Return weight",
    descriptionZh: "总收益率在评分中的占比",
    descriptionEn: "Weight of total return in scoring",
    min: 0, max: 1, step: 0.05, defaultValue: 0.15,
  },
  {
    key: "weight_calmar",
    labelZh: "Calmar 比率权重",
    labelEn: "Calmar ratio weight",
    descriptionZh: "收益/最大回撤比率在评分中的占比",
    descriptionEn: "Weight of return/max-drawdown ratio in scoring",
    min: 0, max: 1, step: 0.05, defaultValue: 0.25,
  },
  {
    key: "weight_sortino",
    labelZh: "Sortino 比率权重",
    labelEn: "Sortino ratio weight",
    descriptionZh: "下行风险调整收益在评分中的占比",
    descriptionEn: "Weight of downside risk-adjusted return in scoring",
    min: 0, max: 1, step: 0.05, defaultValue: 0.15,
  },
  {
    key: "weight_drawdown",
    labelZh: "回撤惩罚权重",
    labelEn: "Drawdown penalty weight",
    descriptionZh: "最大回撤对评分的惩罚力度",
    descriptionEn: "Penalty weight of max drawdown in scoring",
    min: 0, max: 1, step: 0.05, defaultValue: 0.25,
  },
  {
    key: "weight_stop_frequency",
    labelZh: "止损频率惩罚",
    labelEn: "Stop frequency penalty",
    descriptionZh: "频繁触发止损对评分的惩罚力度",
    descriptionEn: "Penalty applied when stop-loss events are frequent",
    min: 0, max: 1, step: 0.05, defaultValue: 0.10,
  },
  {
    key: "weight_capital_utilization",
    labelZh: "资金利用权重",
    labelEn: "Capital utilization weight",
    descriptionZh: "资金利用效率在评分中的占比",
    descriptionEn: "Weight of capital utilization in scoring",
    min: 0, max: 1, step: 0.05, defaultValue: 0.05,
  },
  {
    key: "weight_trade_stability",
    labelZh: "交易稳定权重",
    labelEn: "Trade stability weight",
    descriptionZh: "交易次数稳定性在评分中的占比",
    descriptionEn: "Weight of trade stability in scoring",
    min: 0, max: 1, step: 0.05, defaultValue: 0.05,
  },
];

/* ------------------------------------------------------------------ */
/*  Component                                                         */
/* ------------------------------------------------------------------ */

export interface ScoringWeightEditorProps {
  lang: UiLanguage;
  value?: Record<string, number> | null;
  onChange?: (weights: Record<string, number>) => void;
}

export function ScoringWeightEditor({ lang, value: externalValue, onChange }: ScoringWeightEditorProps) {
  const defaultWeights = Object.fromEntries(WEIGHT_DEFS.map((d) => [d.key, d.defaultValue]));
  const [weights, setWeights] = useState<Record<string, number>>(defaultWeights);

  useEffect(() => {
    if (!externalValue) return;
    setWeights((prev) => ({ ...prev, ...externalValue }));
  }, [externalValue]);

  const total = WEIGHT_DEFS.reduce((sum, d) => sum + (weights[d.key] ?? 0), 0);
  const isNormalized = Math.abs(total - 1) < 0.01;

  const updateWeight = useCallback(
    (key: string, raw: string) => {
      const val = Number(raw);
      if (!Number.isFinite(val)) return;
      setWeights((prev) => {
        const next = { ...prev, [key]: val };
        onChange?.(next);
        return next;
      });
    },
    [onChange],
  );

  const autoNormalize = useCallback(() => {
    setWeights((prev) => {
      const currentTotal = WEIGHT_DEFS.reduce((sum, d) => sum + (prev[d.key] ?? 0), 0);
      if (currentTotal === 0) return prev;
      const next: Record<string, number> = {};
      for (const d of WEIGHT_DEFS) {
        next[d.key] = Math.round(((prev[d.key] ?? 0) / currentTotal) * 100) / 100;
      }
      onChange?.(next);
      return next;
    });
  }, [onChange]);

  const resetDefaults = useCallback(() => {
    const next = { ...defaultWeights };
    setWeights(next);
    onChange?.(next);
  }, [onChange]);

  return (
    <div className="space-y-3">
      <div className="flex items-center justify-between">
        <p className="text-sm text-muted-foreground">
          {pickText(lang, "调整评分权重，所有权重之和应为 1.0", "Adjust scoring weights; total should sum to 1.0")}
        </p>
        <div className="flex gap-2">
          <button
            className="rounded-md border border-border px-2 py-1 text-xs hover:bg-secondary/50"
            onClick={autoNormalize}
            type="button"
          >
            {pickText(lang, "自动归一化", "Auto normalize")}
          </button>
          <button
            className="rounded-md border border-border px-2 py-1 text-xs hover:bg-secondary/50"
            onClick={resetDefaults}
            type="button"
          >
            {pickText(lang, "恢复默认", "Reset defaults")}
          </button>
        </div>
      </div>

      {WEIGHT_DEFS.map((def) => {
        const val = weights[def.key] ?? def.defaultValue;
        return (
          <div key={def.key} className="space-y-1">
            <div className="flex items-center justify-between">
              <label className="text-sm font-medium" htmlFor={`weight-${def.key}`}>
                {pickText(lang, def.labelZh, def.labelEn)}
              </label>
              <span className="text-sm font-mono tabular-nums">{val.toFixed(2)}</span>
            </div>
            <p className="text-xs text-muted-foreground">{pickText(lang, def.descriptionZh, def.descriptionEn)}</p>
            <input
              className="w-full accent-primary"
              id={`weight-${def.key}`}
              max={def.max}
              min={def.min}
              onChange={(e) => updateWeight(def.key, e.target.value)}
              step={def.step}
              type="range"
              value={val}
            />
          </div>
        );
      })}

      <div className={`text-sm font-medium ${isNormalized ? "text-emerald-600" : "text-amber-600"}`}>
        {pickText(lang, `权重总和: ${total.toFixed(2)}`, `Total: ${total.toFixed(2)}`)}
        {!isNormalized && ` — ${pickText(lang, "建议归一化", "consider normalizing")}`}
      </div>
    </div>
  );
}
