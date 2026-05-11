import { pickText, type UiLanguage } from "@/lib/ui/preferences";

const PATH = "M0,82 C28,70 48,44 72,38 C96,32 116,56 140,47 C164,38 184,15 208,18 C232,21 248,48 276,42 C302,36 326,10 360,14";

export function BacktestCharts({ lang }: { lang: UiLanguage }) {
  return (
    <section className="grid gap-4 lg:grid-cols-[1.3fr_1fr]">
      <div className="rounded-2xl border border-border bg-card p-4 shadow-sm">
        <div className="mb-3">
          <h2 className="text-lg font-semibold">{pickText(lang, "回测图表", "Backtest charts")}</h2>
          <p className="text-sm text-muted-foreground">
            {pickText(lang, "用轻量 SVG 展示权益曲线，不引入额外图表依赖。", "Use lightweight SVG curves instead of pulling in a heavy chart dependency.")}
          </p>
        </div>
        <svg aria-label={pickText(lang, "权益曲线预览", "Equity curve preview")} className="h-52 w-full rounded-xl border border-border bg-background p-4" viewBox="0 0 360 100">
          <path d={PATH} fill="none" stroke="currentColor" strokeWidth="3" />
          <path d="M0,90 L360,90" fill="none" opacity="0.25" stroke="currentColor" strokeDasharray="4 6" />
        </svg>
      </div>

      <div className="rounded-2xl border border-border bg-card p-4 shadow-sm">
        <div className="mb-3">
          <h2 className="text-lg font-semibold">{pickText(lang, "压力窗口", "Stress windows")}</h2>
          <p className="text-sm text-muted-foreground">
            {pickText(lang, "查看极端行情下的剩余生存空间。", "Review residual survival room under extreme conditions.")}
          </p>
        </div>
        <div className="space-y-3">
          <BarRow label={pickText(lang, "暴跌", "Flash crash")} value="72%" />
          <BarRow label={pickText(lang, "暴涨", "Parabolic rise")} value="81%" />
          <BarRow label={pickText(lang, "长单边", "Long trend")} value="64%" />
          <BarRow label={pickText(lang, "插针", "Wick shock")} value="77%" />
        </div>
      </div>
    </section>
  );
}

function BarRow({ label, value }: { label: string; value: string }) {
  return (
    <div className="space-y-1">
      <div className="flex items-center justify-between text-sm">
        <span>{label}</span>
        <span className="font-medium">{value}</span>
      </div>
      <div className="h-2 rounded-full bg-secondary/40">
        <div className="h-2 rounded-full bg-primary" style={{ width: value }} />
      </div>
    </div>
  );
}
