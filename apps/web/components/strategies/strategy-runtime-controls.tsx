"use client";

import { useState } from "react";

import { Button, Field, Input, Select } from "@/components/ui/form";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";
import type { StrategyWorkspaceValues } from "@/components/strategies/strategy-workspace-form";

type RuntimeControlsProps = {
  lang: UiLanguage;
  onOverallStopLossChange: (value: string) => void;
  onOverallTakeProfitChange: (value: string) => void;
  onPostTriggerChange: (value: StrategyWorkspaceValues["postTrigger"]) => void;
  overallStopLoss: string;
  overallTakeProfit: string;
  postTrigger: StrategyWorkspaceValues["postTrigger"];
  strategyStatus?: string;
};

export function StrategyRuntimeControls({
  lang,
  onOverallStopLossChange,
  onOverallTakeProfitChange,
  onPostTriggerChange,
  overallStopLoss,
  overallTakeProfit,
  postTrigger,
  strategyStatus,
}: RuntimeControlsProps) {
  const [drainOnlyConfirmed, setDrainOnlyConfirmed] = useState(false);
  const [stopAfterTpConfirmed, setStopAfterTpConfirmed] = useState(false);
  const isRunning = strategyStatus === "Running";

  return (
    <div className="space-y-6">
      <div className="grid gap-3 md:grid-cols-2">
        <Field label={pickText(lang, "整体止盈 (%)", "Overall Take Profit (%)")}>
          <Input
            inputMode="decimal"
            name="overallTakeProfit"
            onChange={(event) => onOverallTakeProfitChange(event.target.value)}
            value={overallTakeProfit}
          />
        </Field>
        <Field
          hint={pickText(lang, "留空表示不启用整体止损", "Leave empty to disable overall stop loss")}
          label={pickText(lang, "整体止损 (%)", "Overall Stop Loss (%)")}
        >
          <Input
            inputMode="decimal"
            name="overallStopLoss"
            onChange={(event) => onOverallStopLossChange(event.target.value)}
            value={overallStopLoss}
          />
        </Field>
      </div>

      <Field label={pickText(lang, "触发后行为", "Post Trigger Action")}>
        <Select
          name="postTrigger"
          onChange={(event) => onPostTriggerChange(event.target.value as StrategyWorkspaceValues["postTrigger"])}
          value={postTrigger}
        >
          <option value="stop">{pickText(lang, "执行后停止", "Stop After Trigger")}</option>
          <option value="rebuild">{pickText(lang, "重建继续", "Rebuild and Continue")}</option>
        </Select>
      </Field>

      {isRunning && (
        <div className="space-y-3 rounded-xl border border-amber-300 bg-amber-50 p-4 dark:border-amber-500/30 dark:bg-amber-500/10">
          <p className="text-sm font-semibold text-amber-900 dark:text-amber-100">
            {pickText(lang, "运行时控制", "Runtime Controls")}
          </p>
          <label className="flex items-center gap-3">
            <input
              checked={drainOnlyConfirmed}
              onChange={(event) => setDrainOnlyConfirmed(event.target.checked)}
              type="checkbox"
            />
            <span className="text-sm text-amber-900 dark:text-amber-100">
              {pickText(lang, "只卖不买（排空模式）", "Sell-only / drain mode")}
            </span>
          </label>
          <label className="flex items-center gap-3">
            <input
              checked={stopAfterTpConfirmed}
              onChange={(event) => setStopAfterTpConfirmed(event.target.checked)}
              type="checkbox"
            />
            <span className="text-sm text-amber-900 dark:text-amber-100">
              {pickText(lang, "止盈后停止策略", "Stop strategy after take profit")}
            </span>
          </label>
          {drainOnlyConfirmed && (
            <input name="drainOnly" type="hidden" value="true" />
          )}
          {stopAfterTpConfirmed && (
            <input name="stopAfterTakeProfit" type="hidden" value="true" />
          )}
        </div>
      )}
    </div>
  );
}
