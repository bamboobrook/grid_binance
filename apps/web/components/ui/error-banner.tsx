"use client";

import { pickText, type UiLanguage } from "@/lib/ui/preferences";

type ErrorLevel = "actionable" | "system" | "warning";

type ErrorBannerProps = {
  lang: UiLanguage;
  level?: ErrorLevel;
  title: string;
  detail?: string;
  action?: { label: string; href?: string; onClick?: () => void };
  helpHref?: string;
  onDismiss?: () => void;
};

const levelStyles: Record<ErrorLevel, string> = {
  actionable: "border-amber-500/30 bg-amber-500/5 text-amber-200",
  system: "border-red-500/30 bg-red-500/5 text-red-200",
  warning: "border-blue-500/30 bg-blue-500/5 text-blue-200",
};

export function ErrorBanner({
  lang,
  level = "system",
  title,
  detail,
  action,
  helpHref,
  onDismiss,
}: ErrorBannerProps) {
  return (
    <div className={`rounded-lg border px-4 py-3 ${levelStyles[level]}`}>
      <div className="flex items-start gap-3">
        <span className="mt-0.5 text-base">
          {level === "actionable" ? "⚠" : level === "system" ? "✕" : "ℹ"}
        </span>
        <div className="flex-1 min-w-0">
          <p className="text-sm font-medium">{title}</p>
          {detail && (
            <p className="mt-1 text-xs opacity-80">{detail}</p>
          )}
          <div className="mt-2 flex flex-wrap items-center gap-2">
            {action && (
              <button
                type="button"
                onClick={action.onClick}
                className="rounded-md bg-white/10 px-2.5 py-1 text-xs font-medium hover:bg-white/20 transition-colors"
                {...(action.href ? { as: "a", href: action.href } : {})}
              >
                {action.label}
              </button>
            )}
            {helpHref && (
              <a
                href={helpHref}
                className="text-xs underline opacity-70 hover:opacity-100"
              >
                {pickText(lang, "查看帮助", "Help")}
              </a>
            )}
          </div>
        </div>
        {onDismiss && (
          <button
            type="button"
            onClick={onDismiss}
            className="shrink-0 text-xs opacity-50 hover:opacity-100"
          >
            ✕
          </button>
        )}
      </div>
    </div>
  );
}
