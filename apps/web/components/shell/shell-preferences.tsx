"use client";

import { usePathname, useRouter } from "next/navigation";
import { useState, useTransition } from "react";

import {
  buildPreferenceCookie,
  pickText,
  UI_LANGUAGE_COOKIE,
  UI_THEME_COOKIE,
  type UiLanguage,
  type UiTheme,
} from "../../lib/ui/preferences";

type ShellPreferencesProps = {
  lang: UiLanguage;
  theme: UiTheme | null;
};

function withLocale(pathname: string, nextLanguage: UiLanguage) {
  if (pathname.startsWith("/zh/") || pathname === "/zh" || pathname.startsWith("/en/") || pathname === "/en") {
    return pathname.replace(/^\/(zh|en)(?=\/|$)/, `/${nextLanguage}`);
  }
  return `/${nextLanguage}${pathname.startsWith("/") ? pathname : `/${pathname}`}`;
}

export function ShellPreferences({ lang, theme }: ShellPreferencesProps) {
  const pathname = usePathname();
  const router = useRouter();
  const [currentLang, setCurrentLang] = useState<UiLanguage>(lang);
  const [currentTheme, setCurrentTheme] = useState<UiTheme>(() => {
    if (theme) {
      return theme;
    }
    if (typeof window !== "undefined" && window.matchMedia("(prefers-color-scheme: dark)").matches) {
      return "dark";
    }
    return "light";
  });
  const [isPending, startTransition] = useTransition();

  function persist(name: string, value: string) {
    document.cookie = buildPreferenceCookie(name, value);
  }

  function applyTheme(nextTheme: UiTheme) {
    setCurrentTheme(nextTheme);
    persist(UI_THEME_COOKIE, nextTheme);
    document.documentElement.dataset.theme = nextTheme;
    document.documentElement.style.colorScheme = nextTheme;
  }

  function applyLanguage(nextLanguage: UiLanguage) {
    if (nextLanguage === currentLang) {
      return;
    }
    setCurrentLang(nextLanguage);
    persist(UI_LANGUAGE_COOKIE, nextLanguage);
    document.documentElement.lang = nextLanguage;
    startTransition(() => {
      router.push(withLocale(pathname, nextLanguage));
    });
  }

  return (
    <section aria-label={pickText(currentLang, "界面偏好", "Interface preferences")} className="shell-preferences">
      <div className="shell-preferences__group">
        <span className="shell-preferences__label">{pickText(currentLang, "语言", "Lang")}</span>
        <div className="shell-preferences__options">
          <button
            aria-pressed={currentLang === "zh"}
            className={currentLang === "zh" ? "button button--ghost shell-preferences__option is-active" : "button button--ghost shell-preferences__option"}
            onClick={() => applyLanguage("zh")}
            type="button"
          >
            中文
          </button>
          <button
            aria-pressed={currentLang === "en"}
            className={currentLang === "en" ? "button button--ghost shell-preferences__option is-active" : "button button--ghost shell-preferences__option"}
            onClick={() => applyLanguage("en")}
            type="button"
          >
            EN
          </button>
        </div>
      </div>
      <div className="shell-preferences__group">
        <span className="shell-preferences__label">{pickText(currentLang, "主题", "Theme")}</span>
        <div className="shell-preferences__options">
          <button
            aria-pressed={currentTheme === "light"}
            className={currentTheme === "light" ? "button button--ghost shell-preferences__option is-active" : "button button--ghost shell-preferences__option"}
            onClick={() => applyTheme("light")}
            type="button"
          >
            {pickText(currentLang, "浅色", "Light")}
          </button>
          <button
            aria-pressed={currentTheme === "dark"}
            className={currentTheme === "dark" ? "button button--ghost shell-preferences__option is-active" : "button button--ghost shell-preferences__option"}
            onClick={() => applyTheme("dark")}
            type="button"
          >
            {pickText(currentLang, "深色", "Dark")}
          </button>
        </div>
      </div>
      {isPending ? <span className="shell-preferences__hint">{pickText(currentLang, "正在切换…", "Switching...")}</span> : null}
    </section>
  );
}
