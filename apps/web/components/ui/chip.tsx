import { cn } from "@/lib/utils";
import type { ReactNode } from "react";
import { resolveUiLanguage, pickText } from "@/lib/ui/preferences";

export function Chip({
  children,
  className,
  tone = "default",
}: {
  children: ReactNode;
  className?: string;
  tone?: "default" | "info" | "success" | "warning" | "danger";
}) {
  return (
    <span
      className={cn(
        "inline-flex items-center justify-center px-2 py-0.5 rounded-[2px] text-[10px] font-bold uppercase tracking-widest border",
        {
          "bg-secondary/50 text-muted-foreground border-border/50": tone === "default",
          "bg-blue-100 text-blue-800 border-blue-200 dark:bg-blue-500/10 dark:text-blue-300 dark:border-blue-500/20": tone === "info",
          "bg-emerald-100 text-emerald-800 border-emerald-200 dark:bg-emerald-500/10 dark:text-emerald-300 dark:border-emerald-500/20": tone === "success",
          "bg-amber-100 text-amber-800 border-amber-200 dark:bg-amber-500/10 dark:text-amber-300 dark:border-amber-500/20": tone === "warning",
          "bg-red-100 text-red-800 border-red-200 dark:bg-red-500/10 dark:text-red-300 dark:border-red-500/20": tone === "danger",
        },
        className,
      )}
    >
      {children}
    </span>
  );
}

export function useUiCopy(zh: string, en: string) {
  // Try to read the language from the document element on the client
  // Fall back to English if it can't be determined
  let lang: "zh" | "en" = "en";
  if (typeof document !== "undefined") {
    lang = resolveUiLanguage(document.documentElement.lang);
  }
  return pickText(lang, zh, en);
}
