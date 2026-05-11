import { pickText, type UiLanguage } from "@/lib/ui/preferences";

export function RiskRuleEditor({ lang }: { lang: UiLanguage }) {
  return (
    <section className="rounded-2xl border border-border bg-card p-4">
      <div className="mb-4">
        <h3 className="text-lg font-semibold">{pickText(lang, "风险规则编辑器", "Risk rule editor")}</h3>
        <p className="text-sm text-muted-foreground">
          {pickText(
            lang,
            "先做生存优先淘汰，再做综合评分，避免高收益掩盖爆仓风险。",
            "Filter with survival-first rules before composite scoring so yield cannot hide liquidation risk.",
          )}
        </p>
      </div>

      <div className="grid gap-4 lg:grid-cols-2">
        <MetricCard
          description={pickText(lang, "先淘汰爆仓、超预算、过度止损与数据不完整候选。", "Filter liquidations, budget breaches, excess stop losses, and incomplete data first.")}
          title={pickText(lang, "生存优先", "Survival first")}
          value={pickText(lang, "启用", "Enabled")}
        />
        <MetricCard
          description={pickText(lang, "Portfolio 全局回撤一旦超限，候选不得进入发布复核。", "If global Portfolio drawdown breaches the cap, the candidate cannot reach publish review.")}
          title={pickText(lang, "Portfolio 回撤上限", "Portfolio drawdown cap")}
          value="18%"
        />
        <MetricCard
          description={pickText(lang, "限制单位时间止损次数，避免连续追单。", "Limit stop-loss count per time window to avoid cascade chasing.")}
          title={pickText(lang, "止损频率", "Stop-loss frequency")}
          value="3 / 30d"
        />
        <MetricCard
          description={pickText(lang, "评分保留收益、回撤、Calmar、资金利用率等权重。", "Score keeps yield, drawdown, Calmar, and capital utilization weights.")}
          title={pickText(lang, "综合评分", "Composite score")}
          value={pickText(lang, "可调权重", "Adjustable weights")}
        />
      </div>
    </section>
  );
}

function MetricCard({
  description,
  title,
  value,
}: {
  description: string;
  title: string;
  value: string;
}) {
  return (
    <div className="rounded-xl border border-border bg-background p-4">
      <div className="flex items-start justify-between gap-3">
        <div>
          <p className="text-sm font-semibold">{title}</p>
          <p className="mt-1 text-xs text-muted-foreground">{description}</p>
        </div>
        <span className="rounded-full bg-secondary/50 px-3 py-1 text-xs font-medium">{value}</span>
      </div>
    </div>
  );
}
