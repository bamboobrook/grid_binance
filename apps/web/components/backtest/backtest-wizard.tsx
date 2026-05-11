import { IndicatorRuleEditor } from "@/components/backtest/indicator-rule-editor";
import { MartingaleParameterEditor } from "@/components/backtest/martingale-parameter-editor";
import { RiskRuleEditor } from "@/components/backtest/risk-rule-editor";
import { SearchConfigEditor } from "@/components/backtest/search-config-editor";
import { TimeSplitEditor } from "@/components/backtest/time-split-editor";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";

const STEPS = [
  {
    key: "search",
    titleZh: "1. 市场与搜索",
    titleEn: "1. Market and search",
    descriptionZh: "配置 symbol 池、市场类型、搜索方式与候选数量。",
    descriptionEn: "Configure symbol pools, market type, search mode, and candidate budgets.",
  },
  {
    key: "martingale",
    titleZh: "2. 马丁参数",
    titleEn: "2. Martingale parameters",
    descriptionZh: "定义方向、杠杆、间距、加仓、止盈和止损框架。",
    descriptionEn: "Define direction, leverage, spacing, sizing, take-profit, and stop-loss rules.",
  },
  {
    key: "indicator",
    titleZh: "3. 指标规则",
    titleEn: "3. Indicator rules",
    descriptionZh: "为 ATR、MA/EMA、RSI、Bollinger、ADX 设置过滤逻辑。",
    descriptionEn: "Set filter logic for ATR, MA/EMA, RSI, Bollinger, and ADX.",
  },
  {
    key: "time",
    titleZh: "4. 时间切分",
    titleEn: "4. Time splits",
    descriptionZh: "指定 Data ranges、walk-forward 和 stress windows。",
    descriptionEn: "Specify data ranges, walk-forward windows, and stress windows.",
  },
  {
    key: "risk",
    titleZh: "5. 风险与评分",
    titleEn: "5. Risk and scoring",
    descriptionZh: "在 Portfolio 维度配置生存优先筛选和发布门槛。",
    descriptionEn: "Configure survival-first filters and publish gates at the Portfolio level.",
  },
] as const;

export function BacktestWizard({ lang }: { lang: UiLanguage }) {
  return (
    <div className="space-y-4">
      <div className="grid gap-3 md:grid-cols-5">
        {STEPS.map((step) => (
          <div className="rounded-xl border border-border bg-background p-3" key={step.key}>
            <p className="text-sm font-semibold">{pickText(lang, step.titleZh, step.titleEn)}</p>
            <p className="mt-1 text-xs text-muted-foreground">
              {pickText(lang, step.descriptionZh, step.descriptionEn)}
            </p>
          </div>
        ))}
      </div>

      <div className="grid gap-4">
        <SearchConfigEditor lang={lang} />
        <MartingaleParameterEditor lang={lang} />
        <IndicatorRuleEditor lang={lang} />
        <TimeSplitEditor lang={lang} />
        <RiskRuleEditor lang={lang} />
      </div>
    </div>
  );
}
