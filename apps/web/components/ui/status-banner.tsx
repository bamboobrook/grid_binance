"use client";

import { pickText, type UiLanguage } from "@/lib/ui/preferences";

type BannerTone = "info" | "warning" | "error" | "success";

const TONE_STYLES: Record<BannerTone, string> = {
  info: "border-blue-200 bg-blue-50 text-blue-800 dark:border-blue-800 dark:bg-blue-950/30 dark:text-blue-300",
  warning: "border-yellow-200 bg-yellow-50 text-yellow-800 dark:border-yellow-800 dark:bg-yellow-950/30 dark:text-yellow-300",
  error: "border-red-200 bg-red-50 text-red-800 dark:border-red-800 dark:bg-red-950/30 dark:text-red-300",
  success: "border-green-200 bg-green-50 text-green-800 dark:border-green-800 dark:bg-green-950/30 dark:text-green-300",
};

type ActionLink = {
  label: string;
  href: string;
};

export function StatusBanner({
  tone,
  title,
  description,
  action,
  lang,
}: {
  tone: BannerTone;
  title: string;
  description?: string;
  action?: ActionLink;
  lang: UiLanguage;
}) {
  return (
    <div
      aria-live="polite"
      className={`status-banner rounded-lg border p-4 ${TONE_STYLES[tone]}`}
      role="status"
    >
      <div className="flex items-start gap-3">
        <span className="text-lg">
          {tone === "error" ? "⚠" : tone === "warning" ? "⚡" : tone === "success" ? "✓" : "ℹ"}
        </span>
        <div className="status-banner__meta flex-1">
          <p className="font-medium">{title}</p>
          {description && (
            <p className="mt-1 text-sm opacity-80">{description}</p>
          )}
          {action && (
            <a
              href={action.href}
              className="status-banner__actions mt-2 inline-block rounded-md bg-white/50 px-3 py-1 text-xs font-medium hover:bg-white/70 dark:bg-black/20 dark:hover:bg-black/30"
            >
              {action.label}
            </a>
          )}
        </div>
      </div>
    </div>
  );
}

export function ErrorBanner({
  error,
  lang,
  retry,
  helpHref,
}: {
  error: string;
  lang: UiLanguage;
  retry?: () => void;
  helpHref?: string;
}) {
  return (
    <StatusBanner
      tone="error"
      title={pickText(lang, "操作失败", "Action Failed")}
      description={error}
      action={
        retry
          ? { label: pickText(lang, "重试", "Retry"), href: "#" }
          : helpHref
            ? { label: pickText(lang, "查看帮助", "View Help"), href: helpHref }
            : undefined
      }
      lang={lang}
    />
  );
}
