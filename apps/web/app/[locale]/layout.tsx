import "@/styles/globals.css";
import type { ReactNode } from "react";
import { cookies } from "next/headers";
import { NextIntlClientProvider } from "next-intl";
import { ThemeProvider } from "@/components/providers";
import {
  buildThemeInitScript,
  resolveUiTheme,
  UI_THEME_COOKIE,
} from "@/lib/ui/preferences";

export default async function LocaleLayout({
  children,
  params,
}: {
  children: ReactNode;
  params: Promise<{ locale: string }>;
}) {
  const [{ locale }, cookieStore] = await Promise.all([params, cookies()]);
  const currentLocale = locale === "en" ? "en" : "zh";
  const messages = (await import(`@/messages/${currentLocale}.json`)).default;
  const theme = resolveUiTheme(cookieStore.get(UI_THEME_COOKIE)?.value);

  return (
    <html data-theme={theme ?? undefined} lang={currentLocale} suppressHydrationWarning>
      <body className="app-body min-h-screen bg-background text-foreground font-sans antialiased">
        <script dangerouslySetInnerHTML={{ __html: buildThemeInitScript(theme) }} id="theme-init" />
        <NextIntlClientProvider messages={messages} locale={currentLocale}>
          <ThemeProvider attribute="data-theme" defaultTheme={theme ?? "system"} disableTransitionOnChange enableSystem>
            {children}
          </ThemeProvider>
        </NextIntlClientProvider>
      </body>
    </html>
  );
}
