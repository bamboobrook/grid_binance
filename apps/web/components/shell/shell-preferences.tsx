"use client";

import { useEffect, useState } from "react";
import { usePathname } from "next/navigation";

import {
  pickText,
  type UiLanguage,
  type UiTheme,
} from "../../lib/ui/preferences";

type ShellPreferencesProps = {
  lang: UiLanguage;
  theme: UiTheme | null;
};

export function ShellPreferences({ lang, theme }: ShellPreferencesProps) {
  const pathname = usePathname();
  const [returnTo, setReturnTo] = useState(pathname);
  const currentTheme = theme ?? "light";

  useEffect(() => {
    setReturnTo(`${window.location.pathname}${window.location.search}${window.location.hash}`);
  }, [pathname]);

  return (
    <section aria-label={pickText(lang, "界面偏好", "Interface preferences")} className="shell-preferences">
      <form action="/api/ui/preferences" className="shell-preferences__group" method="post">
        <input name="returnTo" type="hidden" value={returnTo} />
        <span className="shell-preferences__label">{pickText(lang, "语言", "Lang")}</span>
        <div className="shell-preferences__options">
          <button
            aria-pressed={lang === "zh"}
            className={lang === "zh" ? "button button--ghost shell-preferences__option is-active" : "button button--ghost shell-preferences__option"}
            name="lang"
            type="submit"
            value="zh"
          >
            中文
          </button>
          <button
            aria-pressed={lang === "en"}
            className={lang === "en" ? "button button--ghost shell-preferences__option is-active" : "button button--ghost shell-preferences__option"}
            name="lang"
            type="submit"
            value="en"
          >
            EN
          </button>
        </div>
      </form>
      <form action="/api/ui/preferences" className="shell-preferences__group" method="post">
        <input name="returnTo" type="hidden" value={returnTo} />
        <span className="shell-preferences__label">{pickText(lang, "主题", "Theme")}</span>
        <div className="shell-preferences__options">
          <button
            aria-pressed={currentTheme === "light"}
            className={currentTheme === "light" ? "button button--ghost shell-preferences__option is-active" : "button button--ghost shell-preferences__option"}
            name="theme"
            type="submit"
            value="light"
          >
            {pickText(lang, "浅色", "Light")}
          </button>
          <button
            aria-pressed={currentTheme === "dark"}
            className={currentTheme === "dark" ? "button button--ghost shell-preferences__option is-active" : "button button--ghost shell-preferences__option"}
            name="theme"
            type="submit"
            value="dark"
          >
            {pickText(lang, "深色", "Dark")}
          </button>
        </div>
      </form>
    </section>
  );
}
