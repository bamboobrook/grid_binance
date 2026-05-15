"use client";

import { pickText, type UiLanguage } from "@/lib/ui/preferences";

export interface MartingaleRiskWarningProps {
  lang: UiLanguage;
  compact?: boolean;
}

const WARNINGS = [
  {
    key: "cross_margin",
    zh: "全仓模式下，一个策略的浮亏可能耗尽整个账户余额",
    en: "Under cross margin, a single strategy's unrealized loss can drain the entire account balance",
  },
  {
    key: "leverage",
    zh: "合约杠杆放大收益的同时也放大亏损，强平价格可能高于预期",
    en: "Leverage amplifies both gains and losses; liquidation price may be closer than expected",
  },
  {
    key: "hedge_mode",
    zh: "双向持仓模式下，同 symbol 的多空策略共享保证金，强平风险叠加",
    en: "In hedge mode, long and short strategies on the same symbol share margin, compounding liquidation risk",
  },
  {
    key: "martingale_tail",
    zh: "马丁格尔策略在极端行情下需要无限资金补仓，尾部风险不可控",
    en: "Martingale strategies require infinite capital to keep averaging down in extreme markets; tail risk is unbounded",
  },
  {
    key: "backtest_disclaimer",
    zh: "回测结果不代表未来收益，历史表现无法保证未来盈利",
    en: "Backtest results do not guarantee future returns; past performance is not indicative of future results",
  },
];

export function MartingaleRiskWarning({ lang, compact }: MartingaleRiskWarningProps) {
  return (
    <div className="rounded-xl border border-amber-500/40 bg-amber-500/5 px-4 py-3">
      <div className="flex items-start gap-2">
        <span className="mt-0.5 text-amber-600 dark:text-amber-400 text-base leading-none" aria-hidden="true">
          &#9888;
        </span>
        <div className="flex-1 min-w-0">
          <p className="text-sm font-semibold text-amber-700 dark:text-amber-300">
            {pickText(lang, "马丁格尔风险提示", "Martingale Risk Warning")}
          </p>
          <ul className={`mt-1.5 space-y-1 text-xs text-muted-foreground ${compact ? "columns-1" : "columns-1 sm:columns-2"}`}>
            {WARNINGS.map((w) => (
              <li key={w.key} className="break-inside-avoid">
                {pickText(lang, w.zh, w.en)}
              </li>
            ))}
          </ul>
        </div>
      </div>
    </div>
  );
}
