"use client";

import { pickText, type UiLanguage } from "@/lib/ui/preferences";

export function EmptyStateGuide({ lang, locale }: { lang: UiLanguage; locale: string }) {
  const steps = [
    {
      step: 1,
      title: pickText(lang, "绑定交易所", "Connect Exchange"),
      desc: pickText(lang, "添加 Binance API 密钥以开始交易", "Add your Binance API key to start trading"),
      href: `/${locale}/app/exchange`,
      cta: pickText(lang, "去设置", "Set Up"),
    },
    {
      step: 2,
      title: pickText(lang, "创建策略", "Create Strategy"),
      desc: pickText(lang, "选择交易对和网格参数，启动你的第一个网格机器人", "Pick a trading pair and grid params, launch your first grid bot"),
      href: `/${locale}/app/strategies/new`,
      cta: pickText(lang, "去创建", "Create"),
    },
    {
      step: 3,
      title: pickText(lang, "监控运行", "Monitor"),
      desc: pickText(lang, "实时查看策略收益和状态", "Watch your strategy PnL and status in real time"),
      href: `/${locale}/app/dashboard`,
      cta: pickText(lang, "去总览", "Dashboard"),
    },
  ];

  return (
    <div className="rounded-xl border-2 border-dashed border-muted-foreground/25 p-8">
      <h2 className="mb-2 text-center text-lg font-semibold">
        {pickText(lang, "开始你的第一个网格策略", "Start Your First Grid Strategy")}
      </h2>
      <p className="mb-6 text-center text-sm text-muted-foreground">
        {pickText(lang, "按以下步骤快速上手", "Follow these steps to get started")}
      </p>
      <div className="grid gap-4 sm:grid-cols-3">
        {steps.map((s) => (
          <div key={s.step} className="rounded-lg border bg-card p-4 text-center">
            <span className="inline-flex h-8 w-8 items-center justify-center rounded-full bg-primary text-sm font-bold text-primary-foreground">
              {s.step}
            </span>
            <h3 className="mt-2 text-sm font-medium">{s.title}</h3>
            <p className="mt-1 text-xs text-muted-foreground">{s.desc}</p>
            <a
              href={s.href}
              className="mt-3 inline-block rounded-md bg-primary px-3 py-1.5 text-xs font-medium text-primary-foreground hover:bg-primary/90"
            >
              {s.cta}
            </a>
          </div>
        ))}
      </div>
    </div>
  );
}
