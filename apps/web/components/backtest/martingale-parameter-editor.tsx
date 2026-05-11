import type { ChangeEvent, ReactNode } from "react";
import type { WizardForm } from "@/components/backtest/backtest-wizard";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";

export function MartingaleParameterEditor({ form, lang, onChange }: { form: WizardForm; lang: UiLanguage; onChange: (event: ChangeEvent<HTMLInputElement | HTMLSelectElement>) => void }) {
  return (
    <section className="rounded-2xl border border-border bg-card p-4">
      <div className="mb-4">
        <h3 className="text-lg font-semibold">{pickText(lang, "马丁参数编辑器", "Martingale parameter editor")}</h3>
        <p className="text-sm text-muted-foreground">{pickText(lang, "可设置币本组合、倍投间隔、加仓倍率、整体止盈和止损。", "Configure symbols, spacing, multipliers, portfolio take-profit, and stop-loss.")}</p>
      </div>
      <div className="grid gap-4 lg:grid-cols-2">
        <FieldCard title={pickText(lang, "参数预设", "Parameter preset")}>
          <SelectField label={pickText(lang, "自动参数空间", "Automatic search space")} name="parameterPreset" onChange={onChange} value={form.parameterPreset}>
            <option value="conservative">{pickText(lang, "保守", "Conservative")}</option>
            <option value="balanced">{pickText(lang, "均衡", "Balanced")}</option>
            <option value="aggressive">{pickText(lang, "激进", "Aggressive")}</option>
            <option value="custom">{pickText(lang, "手动", "Custom")}</option>
          </SelectField>
          <p className="text-xs text-muted-foreground">{pickText(lang, "系统会围绕预设自动组合间距、加仓倍率、止盈、层数等参数。", "The system combines spacing, multiplier, take-profit, and max-leg ranges around the preset.")}</p>
        </FieldCard>

        <FieldCard title={pickText(lang, "市场与方向", "Market and direction")}>
          <SelectField label={pickText(lang, "市场", "Market")} name="market" onChange={onChange} value={form.market}>
            <option value="spot">Spot</option>
            <option value="usd_m_futures">USDT-M Futures</option>
          </SelectField>
          <SelectField label={pickText(lang, "方向", "Direction")} name="directionMode" onChange={onChange} value={form.directionMode}>
            <option value="long_only">Long</option>
            <option value="short_only">Short</option>
            <option value="long_and_short">Long + Short</option>
          </SelectField>
          <label className="flex items-center gap-2 rounded-lg border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-xs text-amber-700">
            <input checked={form.hedgeModeRequired} name="hedgeModeRequired" onChange={onChange} type="checkbox" />
            {pickText(lang, "Long+Short 需要 Hedge Mode，实盘前必须在交易所/API 确认。", "Long+Short requires Hedge Mode; confirm it on exchange/API before live trading.")}
          </label>
        </FieldCard>
        <FieldCard title={pickText(lang, "保证金与杠杆", "Margin and leverage")}>
          <SelectField label={pickText(lang, "保证金模式", "Margin mode")} name="marginMode" onChange={onChange} value={form.marginMode}>
            <option value="isolated">{pickText(lang, "逐仓", "Isolated")}</option>
            <option value="cross">{pickText(lang, "全仓", "Cross")}</option>
          </SelectField>
          <div className="grid gap-3 sm:grid-cols-2">
            <InputField label={pickText(lang, "最小杠杆", "Min leverage")} name="minLeverage" onChange={onChange} value={form.minLeverage} />
            <InputField label={pickText(lang, "最大杠杆", "Max leverage")} name="maxLeverage" onChange={onChange} value={form.maxLeverage} />
          </div>
        </FieldCard>
        <FieldCard title={pickText(lang, "间距与加仓", "Spacing and sizing")}>
          <div className="grid gap-3 sm:grid-cols-2">
            <InputField label={pickText(lang, "首单 USDT", "Initial USDT")} name="initialOrderUsdt" onChange={onChange} value={form.initialOrderUsdt} />
            <InputField label={pickText(lang, "间隔 %", "Spacing %")} name="spacingPct" onChange={onChange} step="0.1" value={form.spacingPct} />
            <InputField label={pickText(lang, "加仓倍率", "Order multiplier")} name="orderMultiplier" onChange={onChange} step="0.1" value={form.orderMultiplier} />
            <InputField label={pickText(lang, "最大层数", "Max legs")} name="maxLegs" onChange={onChange} value={form.maxLegs} />
          </div>
        </FieldCard>
        <FieldCard title={pickText(lang, "止盈与止损", "Take-profit and stop-loss")}>
          <div className="grid gap-3 sm:grid-cols-2">
            <InputField label={pickText(lang, "整体止盈 %", "Portfolio TP %")} name="takeProfitPct" onChange={onChange} step="0.1" value={form.takeProfitPct} />
            <InputField label={pickText(lang, "移动回撤 %", "Trailing %")} name="trailingPct" onChange={onChange} step="0.1" value={form.trailingPct} />
          </div>
          <SelectField label={pickText(lang, "止损模式", "Stop-loss mode")} name="stopLossMode" onChange={onChange} value={form.stopLossMode}>
            <option value="range">{pickText(lang, "区间止损", "Range stop")}</option>
            <option value="atr">ATR</option>
            <option value="portfolio_drawdown">{pickText(lang, "整体回撤", "Portfolio drawdown")}</option>
            <option value="strategy_drawdown">{pickText(lang, "单策略回撤", "Strategy drawdown")}</option>
          </SelectField>
        </FieldCard>
      </div>
    </section>
  );
}

function FieldCard({ children, title }: { children: ReactNode; title: string }) {
  return <div className="space-y-3 rounded-xl border border-border bg-background p-4"><h4 className="text-sm font-semibold">{title}</h4>{children}</div>;
}

function SelectField({ children, label, name, onChange, value }: { children: ReactNode; label: string; name: keyof WizardForm; onChange: (event: ChangeEvent<HTMLSelectElement>) => void; value: string }) {
  return <label className="flex flex-col gap-1 text-sm"><span className="text-xs uppercase tracking-wide text-muted-foreground">{label}</span><select className="rounded-lg border border-border bg-card px-3 py-2" name={name} onChange={onChange} value={value}>{children}</select></label>;
}

function InputField({ label, name, onChange, step = "1", value }: { label: string; name: keyof WizardForm; onChange: (event: ChangeEvent<HTMLInputElement>) => void; step?: string; value: string }) {
  return <label className="flex flex-col gap-1 text-sm"><span className="text-xs uppercase tracking-wide text-muted-foreground">{label}</span><input className="rounded-lg border border-border bg-card px-3 py-2" min="0" name={name} onChange={onChange} step={step} type="number" value={value} /></label>;
}
