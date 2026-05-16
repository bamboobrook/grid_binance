import type { ChangeEvent } from "react";
import type { WizardForm } from "@/components/backtest/backtest-wizard";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";

export function RiskRuleEditor({ form, lang, onChange }: { form: WizardForm; lang: UiLanguage; onChange: (event: ChangeEvent<HTMLInputElement>) => void }) {
  return (
    <section className="rounded-2xl border border-border bg-card p-4">
      <div className="mb-4">
        <h3 className="text-lg font-semibold">{pickText(lang, "风险规则编辑器", "Risk rule editor")}</h3>
        <p className="text-sm text-muted-foreground">{pickText(lang, "这些阈值会写入任务配置，用于筛选候选和后续发布复核。", "These thresholds are saved to the task config for candidate filtering and publish review.")}</p>
      </div>
      <div className="grid gap-4 lg:grid-cols-2">
        <RiskField description={pickText(lang, "超过该 Portfolio 回撤则不进入发布复核。", "Reject candidates above this portfolio drawdown.")} label={pickText(lang, "Portfolio 回撤上限 %", "Portfolio drawdown cap %")} name="maxDrawdownPct" onChange={onChange} value={form.maxDrawdownPct} />
        <RiskField description={pickText(lang, "限制测试窗口内止损次数。", "Limit stop-loss count in the test window.")} label={pickText(lang, "最大止损次数", "Max stop-loss count")} name="maxStopLossCount" onChange={onChange} value={form.maxStopLossCount} />
        <RiskField description={pickText(lang, "整体权益回撤触发止损。", "Portfolio equity drawdown stop.")} label={pickText(lang, "整体止损 %", "Portfolio stop-loss %")} name="portfolioStopLossPct" onChange={onChange} value={form.portfolioStopLossPct} />
        <RiskField description={pickText(lang, "单一策略回撤触发止损。", "Single strategy drawdown stop.")} label={pickText(lang, "单策略止损 %", "Per-strategy stop-loss %")} name="perStrategyStopLossPct" onChange={onChange} value={form.perStrategyStopLossPct} />
      </div>
    </section>
  );
}

function RiskField({ description, label, name, onChange, value }: { description: string; label: string; name: keyof WizardForm; onChange: (event: ChangeEvent<HTMLInputElement>) => void; value: string }) {
  return (
    <label className="rounded-xl border border-border bg-background p-4 text-sm">
      <span className="block text-sm font-semibold">{label}</span>
      <span className="mt-1 block text-xs text-muted-foreground">{description}</span>
      <input className="mt-3 w-full rounded-lg border border-border bg-card px-3 py-2" min="0" name={name} onChange={onChange} step="0.1" type="number" value={value} />
    </label>
  );
}
