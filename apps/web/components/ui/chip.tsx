"use client";

import { createContext, useContext, type ReactNode } from "react";

import { pickText, type UiLanguage } from "../../lib/ui/preferences";

function cx(...parts: Array<string | false | null | undefined>) {
  return parts.filter(Boolean).join(" ");
}

type ChipTone = string;

const UiLanguageContext = createContext<UiLanguage>("zh");

export function UiLanguageProvider({ children, lang }: { children: ReactNode; lang: UiLanguage }) {
  return <UiLanguageContext.Provider value={lang}>{children}</UiLanguageContext.Provider>;
}

export function useUiLanguage() {
  return useContext(UiLanguageContext);
}

export function useUiCopy(zh: string, en: string) {
  return pickText(useUiLanguage(), zh, en);
}

export function Chip({
  children,
  className,
  tone = "default",
}: {
  children: ReactNode;
  className?: string;
  tone?: ChipTone;
}) {
  return <span className={cx("ui-chip", `ui-chip--${tone}`, className)}>{children}</span>;
}
