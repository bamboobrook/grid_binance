"use client";

import { pickText, type UiLanguage } from "@/lib/ui/preferences";

type StrategyStatusBadgeProps = {
  lang: UiLanguage;
  status: string;
  size?: "sm" | "md";
};

export function StrategyStatusBadge({
  lang,
  status,
  size = "sm",
}: StrategyStatusBadgeProps) {
  const config = statusConfig(status, lang);
  const sizeClass = size === "sm"
    ? "px-1.5 py-0.5 text-[9px]"
    : "px-2.5 py-1 text-xs";

  return (
    <span className={`inline-flex items-center gap-1.5 rounded border font-bold uppercase tracking-widest ${sizeClass} ${config.className}`}>
      {config.icon}
      {config.label}
    </span>
  );
}

function statusConfig(status: string, lang: UiLanguage) {
  switch (status) {
    case "Running":
      return {
        className: "border-emerald-500/20 bg-emerald-500/10 text-emerald-400",
        icon: <span className="relative flex h-2 w-2"><span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-emerald-400 opacity-75" /><span className="relative inline-flex h-2 w-2 rounded-full bg-emerald-500" /></span>,
        label: pickText(lang, "运行中", "Running"),
      };
    case "Paused":
      return {
        className: "border-amber-500/20 bg-amber-500/10 text-amber-400",
        icon: <span className="inline-flex h-2 w-2 rounded-sm bg-amber-500" />,
        label: pickText(lang, "已暂停", "Paused"),
      };
    case "ErrorPaused":
      return {
        className: "border-red-500/20 bg-red-500/10 text-red-400",
        icon: <span className="relative flex h-2 w-2"><span className="absolute inline-flex h-full w-full animate-ping rounded-full bg-red-400 opacity-75" /><span className="relative inline-flex h-2 w-2 rounded-full bg-red-500" /></span>,
        label: pickText(lang, "异常阻塞", "Blocked"),
      };
    case "Draft":
      return {
        className: "border-blue-500/20 bg-blue-500/10 text-blue-400 border-dashed",
        icon: null,
        label: pickText(lang, "草稿", "Draft"),
      };
    case "Stopping":
      return {
        className: "border-orange-500/20 bg-orange-500/10 text-orange-400",
        icon: <span className="inline-flex h-2 w-2 rounded-full bg-orange-500 animate-pulse" />,
        label: pickText(lang, "停止中", "Stopping"),
      };
    case "Stopped":
      return {
        className: "border-border bg-secondary text-muted-foreground",
        icon: <span className="inline-flex h-2 w-2 rounded-full bg-muted-foreground/50" />,
        label: pickText(lang, "已停止", "Stopped"),
      };
    case "Completed":
      return {
        className: "border-border bg-secondary text-muted-foreground",
        icon: null,
        label: pickText(lang, "已完成", "Completed"),
      };
    default:
      return {
        className: "border-border bg-secondary text-muted-foreground",
        icon: null,
        label: status,
      };
  }
}
