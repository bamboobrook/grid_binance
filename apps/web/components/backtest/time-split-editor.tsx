import type { ReactNode } from "react";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";

export function TimeSplitEditor({ lang }: { lang: UiLanguage }) {
  return (
    <section className="rounded-2xl border border-border bg-card p-4">
      <div className="mb-4">
        <h3 className="text-lg font-semibold">{pickText(lang, "时间切分编辑器", "Time split editor")}</h3>
        <p className="text-sm text-muted-foreground">
          {pickText(lang, "支持手动区间、walk-forward 与 stress windows。", "Supports manual ranges, walk-forward windows, and stress windows.")}
        </p>
      </div>

      <div className="grid gap-4 lg:grid-cols-2">
        <SplitCard title="Data ranges">
          <SplitField defaultValue="2023-01-01 ~ 2024-12-31" label={pickText(lang, "训练", "Train")} />
          <SplitField defaultValue="2025-01-01 ~ 2025-03-31" label={pickText(lang, "验证", "Validate")} />
          <SplitField defaultValue="2025-04-01 ~ 2025-06-30" label={pickText(lang, "测试", "Test")} />
        </SplitCard>

        <SplitCard title={pickText(lang, "Walk-forward / Stress", "Walk-forward / Stress")}>
          <SplitField defaultValue="120 / 30 / 30 days" label="walk-forward" />
          <SplitField defaultValue="flash crash, trend up" label="stress windows" />
          <SplitField defaultValue="保守 intrabar 顺序" label={pickText(lang, "执行规则", "Execution rule")} />
        </SplitCard>
      </div>
    </section>
  );
}

function SplitCard({
  children,
  title,
}: {
  children: ReactNode;
  title: string;
}) {
  return (
    <div className="space-y-3 rounded-xl border border-border bg-background p-4">
      <h4 className="text-sm font-semibold">{title}</h4>
      {children}
    </div>
  );
}

function SplitField({ defaultValue, label }: { defaultValue: string; label: string }) {
  return (
    <label className="flex flex-col gap-1">
      <span className="text-xs uppercase tracking-wide text-muted-foreground">{label}</span>
      <input className="rounded-lg border border-border bg-card px-3 py-2 text-sm" defaultValue={defaultValue} />
    </label>
  );
}
