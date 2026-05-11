import type { ChangeEvent, ReactNode } from "react";
import type { WizardForm } from "@/components/backtest/backtest-wizard";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";

export function TimeSplitEditor({ form, lang, onChange }: { form: WizardForm; lang: UiLanguage; onChange: (event: ChangeEvent<HTMLInputElement | HTMLSelectElement>) => void }) {
  return (
    <section className="rounded-2xl border border-border bg-card p-4">
      <div className="mb-4">
        <h3 className="text-lg font-semibold">{pickText(lang, "时间切分编辑器", "Time split editor")}</h3>
        <p className="text-sm text-muted-foreground">{pickText(lang, "请用日期选择器点击选择，不需要手动拼接时间字符串。", "Use date pickers instead of manually typing range strings.")}</p>
      </div>
      <div className="grid gap-4 lg:grid-cols-2">
        <SplitCard title="Data ranges">
          <label className="flex flex-col gap-1 text-sm">
            <span className="text-xs uppercase tracking-wide text-muted-foreground">{pickText(lang, "时间模式", "Time mode")}</span>
            <select className="rounded-lg border border-border bg-card px-3 py-2" name="timeMode" onChange={onChange} value={form.timeMode}>
              <option value="auto_recent">{pickText(lang, "自动最近区间", "Automatic recent windows")}</option>
              <option value="manual">{pickText(lang, "手动覆盖", "Manual override")}</option>
            </select>
          </label>
          <p className="rounded-lg border border-border bg-card px-3 py-2 text-xs text-muted-foreground">{pickText(lang, "默认自动使用最近 365 天做大区间回测，并切分训练/验证/测试。", "By default, use the latest 365 days and split into train/validate/test windows.")}</p>
          <div className="grid gap-3 sm:grid-cols-2">
            <DateField label={pickText(lang, "训练开始", "Train start")} name="trainStart" onChange={onChange} value={form.trainStart} />
            <DateField label={pickText(lang, "训练结束", "Train end")} name="trainEnd" onChange={onChange} value={form.trainEnd} />
            <DateField label={pickText(lang, "验证开始", "Validate start")} name="validateStart" onChange={onChange} value={form.validateStart} />
            <DateField label={pickText(lang, "验证结束", "Validate end")} name="validateEnd" onChange={onChange} value={form.validateEnd} />
            <DateField label={pickText(lang, "测试开始", "Test start")} name="testStart" onChange={onChange} value={form.testStart} />
            <DateField label={pickText(lang, "测试结束", "Test end")} name="testEnd" onChange={onChange} value={form.testEnd} />
          </div>
        </SplitCard>
        <SplitCard title={pickText(lang, "K线与压力窗口", "Kline and stress windows")}>
          <label className="flex flex-col gap-1 text-sm">
            <span className="text-xs uppercase tracking-wide text-muted-foreground">interval</span>
            <select className="rounded-lg border border-border bg-card px-3 py-2" name="interval" onChange={onChange} value={form.interval}>
              <option value="1m">1m</option><option value="5m">5m</option><option value="15m">15m</option><option value="1h">1h</option><option value="4h">4h</option><option value="1d">1d</option>
            </select>
          </label>
          <p className="rounded-lg border border-border bg-card px-3 py-2 text-xs text-muted-foreground">stress windows: flash_crash, trend_up</p>
        </SplitCard>
      </div>
    </section>
  );
}

function SplitCard({ children, title }: { children: ReactNode; title: string }) {
  return <div className="space-y-3 rounded-xl border border-border bg-background p-4"><h4 className="text-sm font-semibold">{title}</h4>{children}</div>;
}

function DateField({ label, name, onChange, value }: { label: string; name: keyof WizardForm; onChange: (event: ChangeEvent<HTMLInputElement>) => void; value: string }) {
  return <label className="flex flex-col gap-1 text-sm"><span className="text-xs uppercase tracking-wide text-muted-foreground">{label}</span><input className="rounded-lg border border-border bg-card px-3 py-2" name={name} onChange={onChange} type="date" value={value} /></label>;
}
