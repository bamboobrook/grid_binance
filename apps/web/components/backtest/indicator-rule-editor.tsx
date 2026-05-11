import { pickText, type UiLanguage } from "@/lib/ui/preferences";

const INDICATORS = ["ATR", "MA/EMA", "RSI", "Bollinger", "ADX"] as const;

export function IndicatorRuleEditor({ lang }: { lang: UiLanguage }) {
  return (
    <section className="rounded-2xl border border-border bg-card p-4">
      <div className="mb-4">
        <h3 className="text-lg font-semibold">
          {pickText(lang, "指标规则编辑器", "Indicator rule editor")}
        </h3>
        <p className="text-sm text-muted-foreground">
          {pickText(lang, "启用基础指标后，再决定哪些条件允许开新周期。", "Enable baseline indicators, then decide which conditions allow new cycles.")}
        </p>
      </div>

      <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-5">
        {INDICATORS.map((indicator) => (
          <label className="rounded-xl border border-border bg-background p-3" key={indicator}>
            <div className="flex items-center justify-between gap-2">
              <span className="text-sm font-semibold">{indicator}</span>
              <input defaultChecked={indicator !== "ADX"} type="checkbox" />
            </div>
            <p className="mt-2 text-xs text-muted-foreground">
              {pickText(
                lang,
                `${indicator} 用于过滤趋势、波动率或超买超卖状态。`,
                `${indicator} filters trend, volatility, or overbought/oversold state.`,
              )}
            </p>
          </label>
        ))}
      </div>
    </section>
  );
}
