export const UI_LANGUAGE_COOKIE = "ui_lang";
export const UI_THEME_COOKIE = "ui_theme";

export type UiLanguage = "zh" | "en";
export type UiTheme = "light" | "dark";

export function resolveUiLanguage(value?: string | null): UiLanguage {
  return value === "en" ? "en" : "zh";
}

export function resolveUiLanguageFromRoute(routeLocale?: string | null, cookieValue?: string | null): UiLanguage {
  if (routeLocale === "zh" || routeLocale === "en") {
    return routeLocale;
  }
  return resolveUiLanguage(cookieValue);
}

export function resolveUiTheme(value?: string | null): UiTheme | null {
  if (value === "light" || value === "dark") {
    return value;
  }
  return null;
}

export function pickText(lang: UiLanguage, zh: string, en: string) {
  return lang === "en" ? en : zh;
}

export function buildPreferenceCookie(name: string, value: string) {
  return `${name}=${value}; Path=/; Max-Age=31536000; SameSite=Lax`;
}

export function buildThemeInitScript(theme: UiTheme | null) {
  const explicitTheme = theme ? JSON.stringify(theme) : "null";
  return `(() => {
    const explicitTheme = ${explicitTheme};
    const resolvedTheme = explicitTheme || (window.matchMedia && window.matchMedia('(prefers-color-scheme: dark)').matches ? 'dark' : 'light');
    document.documentElement.dataset.theme = resolvedTheme;
    document.documentElement.style.colorScheme = resolvedTheme;
  })();`;
}
