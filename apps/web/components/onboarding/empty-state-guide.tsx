"use client";

import Link from "next/link";

import { pickText, type UiLanguage } from "@/lib/ui/preferences";

export function EmptyStateGuide({ lang, locale }: { lang: UiLanguage; locale: string }) {
  const steps = [
    {
      step: 1,
      title: pickText(lang, "连接币安 API", "Connect Binance API"),
      desc: pickText(lang, "确认只有读取和交易权限，关闭提现。", "Use read and trade permissions only. Disable withdrawals."),
      href: `/${locale}/app/exchange`,
      cta: pickText(lang, "去设置", "Set Up"),
    },
    {
      step: 2,
      title: pickText(lang, "从模板创建", "Create from a template"),
      desc: pickText(lang, "先选普通网格和小资金，再调整高级参数。", "Start with a spot grid and small capital before advanced settings."),
      href: `/${locale}/app/strategies/new`,
      cta: pickText(lang, "去创建", "Create"),
    },
    {
      step: 3,
      title: pickText(lang, "观察再扩大", "Observe before scaling"),
      desc: pickText(lang, "先看订单、收益和告警，再增加投入。", "Watch orders, PnL, and alerts before increasing capital."),
      href: `/${locale}/app/dashboard`,
      cta: pickText(lang, "去总览", "Dashboard"),
    },
  ];

  return (
    <div className="rounded-md border border-dashed border-muted-foreground/30 bg-card p-5 sm:p-6">
      <h2 className="mb-2 text-lg font-bold">
        {pickText(lang, "还没有机器人？按这个顺序开始。", "No bots yet? Start in this order.")}
      </h2>
      <p className="mb-5 text-sm leading-6 text-muted-foreground">
        {pickText(lang, "系统会保留高级能力，但第一次使用只需要完成下面三步。", "Advanced controls remain available, but first-time users only need these three steps.")}
      </p>
      <div className="grid gap-4 sm:grid-cols-3">
        {steps.map((s) => (
          <div key={s.step} className="rounded-md border border-border bg-background p-4">
            <span className="inline-flex h-8 w-8 items-center justify-center rounded-md bg-primary text-sm font-bold text-primary-foreground">
              {s.step}
            </span>
            <h3 className="mt-3 text-sm font-bold">{s.title}</h3>
            <p className="mt-1 min-h-10 text-xs leading-5 text-muted-foreground">{s.desc}</p>
            <Link
              href={s.href}
              className="mt-3 inline-flex rounded-md bg-primary px-3 py-1.5 text-xs font-bold text-primary-foreground hover:bg-primary/90"
            >
              {s.cta}
            </Link>
          </div>
        ))}
      </div>
    </div>
  );
}
