"use client";

import Link from "next/link";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";

type OnboardingGuideProps = {
  lang: UiLanguage;
  hasExchange: boolean;
};

export function OnboardingGuide({ lang, hasExchange }: OnboardingGuideProps) {
  const steps = [
    {
      step: 1,
      title: pickText(lang, "绑定交易所", "Connect Exchange"),
      desc: pickText(
        lang,
        "添加您的币安 API Key 以开始交易",
        "Add your Binance API key to start trading",
      ),
      href: "/exchange",
      done: hasExchange,
      icon: "🔑",
    },
    {
      step: 2,
      title: pickText(lang, "创建网格策略", "Create Grid Strategy"),
      desc: pickText(
        lang,
        "选择交易对和参数，启动您的第一个网格机器人",
        "Choose a trading pair and parameters to launch your first grid bot",
      ),
      href: "/strategies/new",
      done: false,
      icon: "📊",
    },
    {
      step: 3,
      title: pickText(lang, "监控运行", "Monitor & Optimize"),
      desc: pickText(
        lang,
        "实时查看策略表现，随时调整参数",
        "Track strategy performance in real-time and adjust parameters",
      ),
      href: "/analytics",
      done: false,
      icon: "📈",
    },
  ];

  const nextStep = steps.find((s) => !s.done);

  return (
    <div className="rounded-xl border bg-card p-6">
      <div className="mb-5">
        <h2 className="text-lg font-semibold">
          {pickText(lang, "开始使用网格交易", "Get Started with Grid Trading")}
        </h2>
        <p className="mt-1 text-sm text-muted-foreground">
          {pickText(
            lang,
            "按照以下步骤启动您的第一个网格机器人",
            "Follow these steps to launch your first grid bot",
          )}
        </p>
      </div>

      <div className="space-y-3">
        {steps.map((s) => (
          <div
            key={s.step}
            className={`flex items-start gap-3 rounded-lg border p-3 transition-colors ${
              s.done
                ? "border-emerald-500/20 bg-emerald-500/5"
                : nextStep?.step === s.step
                  ? "border-primary/30 bg-primary/5"
                  : "border-border bg-muted/30 opacity-60"
            }`}
          >
            <span className="mt-0.5 text-lg">{s.icon}</span>
            <div className="flex-1 min-w-0">
              <div className="flex items-center gap-2">
                <span className="text-sm font-medium">{s.title}</span>
                {s.done && (
                  <span className="rounded bg-emerald-500/10 px-1.5 py-0.5 text-[10px] font-bold text-emerald-500">
                    {pickText(lang, "已完成", "Done")}
                  </span>
                )}
              </div>
              <p className="mt-0.5 text-xs text-muted-foreground">{s.desc}</p>
            </div>
            {!s.done && nextStep?.step === s.step && (
              <Link
                href={s.href}
                className="shrink-0 rounded-md bg-primary px-3 py-1.5 text-xs font-medium text-primary-foreground hover:bg-primary/90"
              >
                {pickText(lang, "前往", "Go")}
              </Link>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
