"use client";

import { pickText, type UiLanguage } from "@/lib/ui/preferences";

type StatusType = "Running" | "Paused" | "ErrorPaused" | "Stopped" | "Draft";

const STATUS_CONFIG: Record<StatusType, { dotClass: string; textClass: string; pulse?: boolean }> = {
  Running: { dotClass: "bg-green-500", textClass: "text-green-600 dark:text-green-400", pulse: true },
  Paused: { dotClass: "bg-yellow-500", textClass: "text-yellow-600 dark:text-yellow-400" },
  ErrorPaused: { dotClass: "bg-red-500", textClass: "text-red-600 dark:text-red-400", pulse: true },
  Stopped: { dotClass: "bg-gray-400", textClass: "text-muted-foreground" },
  Draft: { dotClass: "bg-blue-500", textClass: "text-blue-600 dark:text-blue-400" },
};

export function StrategyStatusBadge({
  status,
  lang,
}: {
  status: string;
  lang: UiLanguage;
}) {
  const cfg = STATUS_CONFIG[status as StatusType] ?? STATUS_CONFIG.Stopped;
  const label: Record<string, string> = {
    Running: pickText(lang, "运行中", "Running"),
    Paused: pickText(lang, "已暂停", "Paused"),
    ErrorPaused: pickText(lang, "异常阻塞", "Error Paused"),
    Stopped: pickText(lang, "已停止", "Stopped"),
    Draft: pickText(lang, "草稿", "Draft"),
  };

  return (
    <span className={`inline-flex items-center gap-1.5 text-xs font-medium ${cfg.textClass}`}>
      <span className={`relative flex h-2 w-2${cfg.pulse ? " animate-pulse" : ""}`}>
        {cfg.pulse && (
          <span className={`absolute inline-flex h-full w-full animate-ping rounded-full ${cfg.dotClass} opacity-75`} />
        )}
        <span className={`relative inline-flex h-2 w-2 rounded-full ${cfg.dotClass}`} />
      </span>
      {label[status] ?? status}
    </span>
  );
}
