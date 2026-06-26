import Link from "next/link";
import {
  Activity,
  ArrowRight,
  Bot,
  CheckCircle2,
  LineChart,
  PlayCircle,
  ShieldCheck,
  SlidersHorizontal,
  WalletCards,
} from "lucide-react";

import { Button } from "@/components/ui/form";
import { LocaleToggle } from "@/components/ui/locale-toggle";
import { ThemeToggle } from "@/components/ui/theme-toggle";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";

type PageProps = {
  params: Promise<{ locale: string }>;
};

export default async function HomePage({ params }: PageProps) {
  const { locale } = await params;
  const lang = (locale === "zh" ? "zh" : "en") as UiLanguage;
  const steps = [
    {
      icon: WalletCards,
      title: pickText(lang, "连接币安 API", "Connect Binance API"),
      text: pickText(lang, "只需要读取和交易权限，提现权限必须关闭。", "Use read and trade permissions only. Withdrawal access must stay off."),
    },
    {
      icon: SlidersHorizontal,
      title: pickText(lang, "选择网格或马丁", "Choose Grid or DCA"),
      text: pickText(lang, "先用模板和小资金试跑，再进入高级参数。", "Start with a template and small capital before opening advanced settings."),
    },
    {
      icon: Activity,
      title: pickText(lang, "监控收益和风险", "Monitor PnL and Risk"),
      text: pickText(lang, "在一个控制台看运行状态、成交、告警和会员状态。", "Track bot status, fills, alerts, and membership from one workspace."),
    },
  ];
  const botTypes = [
    {
      title: pickText(lang, "普通网格", "Spot Grid"),
      badge: pickText(lang, "震荡行情", "Sideways market"),
      text: pickText(lang, "价格上下波动时自动低买高卖，适合作为第一个机器人。", "Automatically buys low and sells high in ranging markets. Best for a first bot."),
    },
    {
      title: pickText(lang, "合约网格", "Futures Grid"),
      badge: pickText(lang, "趋势 + 风控", "Trend + guardrails"),
      text: pickText(lang, "支持做多/做空，但必须设置止损、杠杆和仓位上限。", "Supports long or short grids, with required stop loss, leverage, and position limits."),
    },
    {
      title: pickText(lang, "马丁组合", "DCA Portfolio"),
      badge: pickText(lang, "分批补仓", "Scaled entries"),
      text: pickText(lang, "通过回测挑选组合，再按权重发布到实盘。", "Use backtests to select a portfolio before publishing weighted live bots."),
    },
  ];
  const guardrails = [
    pickText(lang, "API 不保存提现权限", "No withdrawal API permission"),
    pickText(lang, "启动前显示投入、区间和止损", "Shows capital, range, and stop loss before launch"),
    pickText(lang, "会员、告警、订单在控制台集中展示", "Membership, alerts, and orders stay visible"),
  ];
  const highlights = [
    {
      label: pickText(lang, "一键创建", "One-click start"),
      value: pickText(lang, "模板预设网格区间", "Template grid ranges"),
    },
    {
      label: pickText(lang, "易上手", "Beginner friendly"),
      value: pickText(lang, "先选场景再调参数", "Scenario first, parameters later"),
    },
    {
      label: pickText(lang, "可控风险", "Risk controls"),
      value: pickText(lang, "止盈止损启动前确认", "Confirm TP/SL before launch"),
    },
  ];

  return (
    <div className="min-h-screen bg-background text-foreground">
      <nav className="sticky top-0 z-30 border-b border-border bg-background/95 backdrop-blur">
        <div className="mx-auto flex h-16 w-full max-w-7xl items-center justify-between px-4 sm:px-6">
          <Link className="flex items-center gap-3 font-black tracking-tight text-foreground" href={`/${locale}`}>
            <span className="flex h-9 w-9 items-center justify-center rounded-md bg-primary text-primary-foreground">
              <Bot className="h-5 w-5" />
            </span>
            <span className="text-lg sm:text-xl">Grid.Binance</span>
          </Link>
          <div className="flex items-center gap-2 sm:gap-3">
            <ThemeToggle />
            <LocaleToggle />
            <Link className="hidden text-sm font-semibold text-muted-foreground hover:text-foreground sm:inline-flex" href={`/${locale}/login`}>
              {pickText(lang, "登录", "Sign in")}
            </Link>
            <Link href={`/${locale}/register`}>
              <Button className="h-10 rounded-md px-4 text-sm font-bold sm:px-5">
                {pickText(lang, "注册", "Register")}
              </Button>
            </Link>
          </div>
        </div>
      </nav>

      <main>
        <section className="border-b border-border bg-muted/30">
          <div className="mx-auto grid w-full max-w-7xl gap-8 px-4 py-10 sm:px-6 lg:grid-cols-[minmax(0,1fr)_26rem] lg:py-12">
            <div className="max-w-3xl">
              <div className="mb-5 inline-flex items-center gap-2 rounded-md border border-border bg-card px-3 py-2 text-xs font-semibold text-muted-foreground">
                <CheckCircle2 className="h-4 w-4 text-emerald-500" />
                {pickText(lang, "简单、易上手、一键开启", "Simple, easy to use, one-click start")}
              </div>
              <h1 className="text-4xl font-black leading-tight tracking-tight text-foreground sm:text-5xl lg:text-6xl">
                {pickText(lang, "从一个安全模板开始，逐步启动网格和马丁机器人。", "Start from a safe template, then launch Grid and DCA bots step by step.")}
              </h1>
              <p className="mt-5 max-w-2xl text-base leading-7 text-muted-foreground sm:text-lg">
                {pickText(
                  lang,
                  "Grid.Binance 是一个构建在币安 API 上的网格与马丁交易机器人。选择模板后即可一键开启，订单、收益和风险提醒都会在控制台集中查看。",
                  "Grid.Binance is a Grid and DCA trading bot built on the Binance API. Pick a template to start quickly, then monitor orders, PnL, and risk alerts in one console.",
                )}
              </p>
              <div className="mt-8 flex flex-col gap-3 sm:flex-row">
                <Link href={`/${locale}/register`}>
                  <Button className="h-12 w-full rounded-md px-6 text-base font-bold sm:w-auto">
                    <PlayCircle className="mr-2 h-5 w-5" />
                    {pickText(lang, "创建第一个机器人", "Create first bot")}
                  </Button>
                </Link>
                <Link href={`/${locale}/login`}>
                  <Button className="h-12 w-full rounded-md px-6 text-base font-bold sm:w-auto" tone="outline">
                    {pickText(lang, "进入控制台", "Open console")}
                    <ArrowRight className="ml-2 h-5 w-5" />
                  </Button>
                </Link>
              </div>
              <div className="mt-7 grid gap-3 text-sm text-muted-foreground sm:grid-cols-3">
                {guardrails.map((item) => (
                  <div className="flex items-start gap-2" key={item}>
                    <ShieldCheck className="mt-0.5 h-4 w-4 shrink-0 text-emerald-500" />
                    <span>{item}</span>
                  </div>
                ))}
              </div>
              <div className="mt-6 grid gap-3 sm:grid-cols-3">
                {highlights.map((item) => (
                  <div className="rounded-md border border-border bg-card p-3" key={item.label}>
                    <p className="text-xs font-semibold text-primary">{item.label}</p>
                    <p className="mt-1 text-sm font-bold text-foreground">{item.value}</p>
                  </div>
                ))}
              </div>
            </div>

            <div className="rounded-md border border-border bg-card p-4 shadow-sm lg:mt-10">
              <div className="flex items-center justify-between border-b border-border pb-3">
                <div>
                  <p className="text-xs font-semibold uppercase text-muted-foreground">{pickText(lang, "推荐开始路径", "Recommended path")}</p>
                  <h2 className="mt-1 text-lg font-bold">{pickText(lang, "先模拟，再小额实盘", "Test first, then go small live")}</h2>
                </div>
                <LineChart className="h-6 w-6 text-primary" />
              </div>
              <div className="mt-4 space-y-3">
                {steps.map((step, index) => (
                  <div className="grid grid-cols-[2rem_1fr] gap-3 rounded-md border border-border bg-background p-3" key={step.title}>
                    <span className="flex h-8 w-8 items-center justify-center rounded-md bg-primary/10 text-primary">
                      <step.icon className="h-4 w-4" />
                    </span>
                    <div>
                      <p className="text-sm font-bold">
                        {index + 1}. {step.title}
                      </p>
                      <p className="mt-1 text-xs leading-5 text-muted-foreground">{step.text}</p>
                    </div>
                  </div>
                ))}
              </div>
            </div>
          </div>
        </section>

        <section className="mx-auto w-full max-w-7xl px-4 py-12 sm:px-6">
          <div className="max-w-2xl">
            <p className="text-xs font-bold uppercase text-primary">{pickText(lang, "机器人类型", "Bot types")}</p>
            <h2 className="mt-2 text-2xl font-black tracking-tight sm:text-3xl">
              {pickText(lang, "不用先理解所有参数，先选场景。", "Choose a scenario before learning every parameter.")}
            </h2>
          </div>
          <div className="mt-6 grid gap-4 md:grid-cols-3">
            {botTypes.map((bot) => (
              <article className="rounded-md border border-border bg-card p-5" key={bot.title}>
                <span className="inline-flex rounded-md bg-secondary px-2 py-1 text-xs font-semibold text-muted-foreground">{bot.badge}</span>
                <h3 className="mt-4 text-lg font-bold">{bot.title}</h3>
                <p className="mt-2 text-sm leading-6 text-muted-foreground">{bot.text}</p>
              </article>
            ))}
          </div>
        </section>
      </main>
    </div>
  );
}
