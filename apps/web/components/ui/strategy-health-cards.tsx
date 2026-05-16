"use client";

import { pickText, type UiLanguage } from "@/lib/ui/preferences";

type StrategyHealth = {
  running: number;
  paused: number;
  errorPaused: number;
  stopped: number;
  draft: number;
};

export function StrategyHealthCards({
  health,
  lang,
}: {
  health: StrategyHealth;
  lang: UiLanguage;
}) {
  const cards = [
    {
      label: pickText(lang, "运行中", "Running"),
      value: health.running,
      color: "text-green-600 dark:text-green-400",
      bg: "bg-green-50 dark:bg-green-950/30",
      dot: "bg-green-500",
    },
    {
      label: pickText(lang, "已暂停", "Paused"),
      value: health.paused,
      color: "text-yellow-600 dark:text-yellow-400",
      bg: "bg-yellow-50 dark:bg-yellow-950/30",
      dot: "bg-yellow-500",
    },
    {
      label: pickText(lang, "异常阻塞", "Error Paused"),
      value: health.errorPaused,
      color: "text-red-600 dark:text-red-400",
      bg: "bg-red-50 dark:bg-red-950/30",
      dot: "bg-red-500",
    },
    {
      label: pickText(lang, "已停止", "Stopped"),
      value: health.stopped,
      color: "text-muted-foreground",
      bg: "bg-muted/50",
      dot: "bg-muted-foreground",
    },
  ];

  return (
    <div className="grid grid-cols-2 gap-3 sm:grid-cols-4">
      {cards.map((card) => (
        <div
          key={card.label}
          className={`rounded-lg border p-3 ${card.bg}`}
        >
          <div className="flex items-center gap-2">
            <span className={`inline-block h-2 w-2 rounded-full ${card.dot}`} />
            <span className="text-xs text-muted-foreground">{card.label}</span>
          </div>
          <p className={`mt-1 text-2xl font-semibold ${card.color}`}>
            {card.value}
          </p>
        </div>
      ))}
    </div>
  );
}
