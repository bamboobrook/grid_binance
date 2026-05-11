import type { ReactNode } from "react";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";

export function MartingaleParameterEditor({ lang }: { lang: UiLanguage }) {
  return (
    <section className="rounded-2xl border border-border bg-card p-4">
      <div className="mb-4">
        <h3 className="text-lg font-semibold">
          {pickText(lang, "马丁参数编辑器", "Martingale parameter editor")}
        </h3>
        <p className="text-sm text-muted-foreground">
          {pickText(
            lang,
            "覆盖 Spot/futures、Long/short/long+short、杠杆区间和补仓结构。",
            "Covers spot/futures, long/short/long+short, leverage ranges, and leg structures.",
          )}
        </p>
      </div>

      <div className="grid gap-4 lg:grid-cols-2">
        <FieldCard title={pickText(lang, "市场与方向", "Market and direction")}>
          <FieldRow label={pickText(lang, "市场", "Market")}>
            <OptionPill label="Spot" />
            <OptionPill label="Futures" />
          </FieldRow>
          <FieldRow label={pickText(lang, "方向", "Direction")}>
            <OptionPill label="Long" />
            <OptionPill label="Short" />
            <OptionPill label="Long+Short" />
          </FieldRow>
          <p className="rounded-lg border border-amber-500/30 bg-amber-500/10 px-3 py-2 text-xs text-amber-700">
            {pickText(lang, "Long+Short 需要 Hedge Mode，且不应静默切换交易所设置。", "Long+Short requires Hedge Mode and should never silently switch exchange settings.")}
          </p>
        </FieldCard>

        <FieldCard title={pickText(lang, "保证金与杠杆", "Margin and leverage")}>
          <FieldRow label={pickText(lang, "保证金模式", "Margin mode")}>
            <OptionPill label="逐仓" />
            <OptionPill label="全仓" />
          </FieldRow>
          <FieldRow label={pickText(lang, "杠杆范围", "Leverage range")}>
            <InlineInput defaultValue="2" label={pickText(lang, "最小", "Min")} />
            <InlineInput defaultValue="4" label={pickText(lang, "最大", "Max")} />
          </FieldRow>
        </FieldCard>

        <FieldCard title={pickText(lang, "间距与加仓", "Spacing and sizing")}>
          <FieldRow label={pickText(lang, "Spacing", "Spacing")}>
            <InlineInput defaultValue="0.8%" label={pickText(lang, "首层", "First leg")} />
            <InlineInput defaultValue="1.35x" label={pickText(lang, "倍率", "Multiplier")} />
          </FieldRow>
          <FieldRow label={pickText(lang, "Sizing", "Sizing")}>
            <InlineInput defaultValue="150 USDT" label={pickText(lang, "首单", "Initial")} />
            <InlineInput defaultValue="6" label={pickText(lang, "层数", "Legs")} />
          </FieldRow>
        </FieldCard>

        <FieldCard title={pickText(lang, "止盈与止损", "Take-profit and stop-loss")}>
          <FieldRow label={pickText(lang, "Take-profit", "Take-profit")}>
            <InlineInput defaultValue="1.4%" label={pickText(lang, "目标", "Target")} />
            <InlineInput defaultValue="0.4%" label={pickText(lang, "回撤", "Trail")} />
          </FieldRow>
          <FieldRow label={pickText(lang, "Stop-loss", "Stop-loss")}>
            <InlineInput defaultValue="8%" label={pickText(lang, "单策略", "Per strategy")} />
            <InlineInput defaultValue="18%" label={pickText(lang, "Portfolio", "Portfolio")} />
          </FieldRow>
        </FieldCard>
      </div>
    </section>
  );
}

function FieldCard({
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

function FieldRow({
  children,
  label,
}: {
  children: ReactNode;
  label: string;
}) {
  return (
    <div className="space-y-2">
      <p className="text-xs font-medium uppercase tracking-wide text-muted-foreground">{label}</p>
      <div className="flex flex-wrap gap-2">{children}</div>
    </div>
  );
}

function OptionPill({ label }: { label: string }) {
  return <span className="rounded-full border border-border px-3 py-1 text-sm">{label}</span>;
}

function InlineInput({ defaultValue, label }: { defaultValue: string; label: string }) {
  return (
    <label className="flex min-w-[112px] flex-1 flex-col gap-1 rounded-lg border border-border px-3 py-2 text-sm">
      <span className="text-[11px] uppercase tracking-wide text-muted-foreground">{label}</span>
      <input className="bg-transparent outline-none" defaultValue={defaultValue} />
    </label>
  );
}
