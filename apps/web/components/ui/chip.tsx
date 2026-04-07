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
          "bg-slate-800/50 text-slate-400 border-slate-700/50": tone === "default",
          "bg-blue-500/10 text-blue-400 border-blue-500/20": tone === "info",
          "bg-emerald-500/10 text-emerald-400 border-emerald-500/20": tone === "success",
          "bg-amber-500/10 text-amber-400 border-amber-500/20": tone === "warning",
          "bg-red-500/10 text-red-400 border-red-500/20": tone === "danger",
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
