import { pickText, type UiLanguage } from "@/lib/ui/preferences";

export function HelpStrategyTypes({ lang }: { lang: UiLanguage }) {
  const types = [
    {
      name: pickText(lang, "普通网格", "Ordinary Grid"),
      desc: pickText(
        lang,
        "在设定价格区间内等距或等比分布网格线，低买高卖循环获利。适合震荡行情。",
        "Places grid lines at equal or geometric intervals within a price range, buying low and selling high in cycles. Best for ranging markets.",
      ),
    },
    {
      name: pickText(lang, "经典双向网格", "Classic Bilateral Grid"),
      desc: pickText(
        lang,
        "以参考价为中心，向上和向下各设网格。上方只卖，下方只买，适合不确定方向但预期波动的行情。",
        "Centers on a reference price with grids above and below. Sells above, buys below. Best when direction is uncertain but volatility is expected.",
      ),
    },
    {
      name: pickText(lang, "合约做多网格", "Futures Long Grid"),
      desc: pickText(
        lang,
        "在合约市场以做多为主，网格从当前价格向上排列，适合看涨行情。",
        "Grid lines upward from current price in futures market with a long bias. Best for bullish markets.",
      ),
    },
    {
      name: pickText(lang, "合约做空网格", "Futures Short Grid"),
      desc: pickText(
        lang,
        "在合约市场以做空为主，网格从当前价格向下排列，适合看跌行情。",
        "Grid lines downward from current price in futures market with a short bias. Best for bearish markets.",
      ),
    },
  ];

  return (
    <div className="space-y-4">
      <h2 className="text-lg font-bold">{pickText(lang, "策略类型详解", "Strategy Types Explained")}</h2>
      {types.map((t) => (
        <div key={t.name} className="rounded-lg border p-4">
          <h3 className="font-medium">{t.name}</h3>
          <p className="mt-1 text-sm text-muted-foreground">{t.desc}</p>
        </div>
      ))}
    </div>
  );
}

export function HelpRiskParams({ lang }: { lang: UiLanguage }) {
  const params = [
    {
      name: pickText(lang, "整体止盈", "Overall Take Profit"),
      desc: pickText(lang, "当累计收益达到设定百分比时触发，可选择停止策略或重建网格。", "Triggers when cumulative PnL reaches the set percentage. Can stop the strategy or rebuild the grid."),
    },
    {
      name: pickText(lang, "整体止损", "Overall Stop Loss"),
      desc: pickText(lang, "当累计亏损达到设定百分比时触发，保护本金。", "Triggers when cumulative loss reaches the set percentage, protecting your capital."),
    },
    {
      name: pickText(lang, "追踪止盈", "Trailing Take Profit"),
      desc: pickText(lang, "收益达到目标后，继续追踪最高点，回撤超过阈值才平仓，锁定更多利润。", "After reaching target PnL, trails the peak and only closes on a drawdown beyond the threshold, locking in more profit."),
    },
  ];

  return (
    <div className="space-y-4">
      <h2 className="text-lg font-bold">{pickText(lang, "风控参数说明", "Risk Control Parameters")}</h2>
      {params.map((p) => (
        <div key={p.name} className="rounded-lg border p-4">
          <h3 className="font-medium">{p.name}</h3>
          <p className="mt-1 text-sm text-muted-foreground">{p.desc}</p>
        </div>
      ))}
    </div>
  );
}

export function HelpFAQ({ lang }: { lang: UiLanguage }) {
  const faqs = [
    {
      q: pickText(lang, "网格间距设多少合适？", "What grid spacing should I use?"),
      a: pickText(lang, "一般建议 0.5%-2%，波动大的币种可适当放宽。间距越小交易越频繁但单次利润越薄。", "Generally 0.5%-2%. Wider for volatile pairs. Smaller spacing means more frequent trades but thinner margins."),
    },
    {
      q: pickText(lang, "需要多少资金？", "How much capital do I need?"),
      a: pickText(lang, "建议至少覆盖 2-3 个网格格的资金量，确保有足够余额执行买入。", "Enough to cover 2-3 grid levels, ensuring sufficient balance for buy orders."),
    },
    {
      q: pickText(lang, "策略异常阻塞怎么办？", "What if my strategy is error-paused?"),
      a: pickText(lang, "检查交易所连接、API权限和余额是否充足，修复后点击恢复即可。", "Check exchange connection, API permissions, and balance. Fix the issue then click Resume."),
    },
  ];

  return (
    <div className="space-y-4">
      <h2 className="text-lg font-bold">{pickText(lang, "常见问题", "FAQ")}</h2>
      {faqs.map((f) => (
        <details key={f.q} className="rounded-lg border p-4">
          <summary className="cursor-pointer font-medium">{f.q}</summary>
          <p className="mt-2 text-sm text-muted-foreground">{f.a}</p>
        </details>
      ))}
    </div>
  );
}
