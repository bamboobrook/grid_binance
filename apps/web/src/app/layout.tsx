import "../styles/globals.css";
import type { ReactNode } from "react";
import { cookies } from "next/headers";

import {
  buildThemeInitScript,
  resolveUiLanguage,
  resolveUiTheme,
  UI_LANGUAGE_COOKIE,
  UI_THEME_COOKIE,
} from "../lib/ui/preferences";

export default async function RootLayout({ children }: { children: ReactNode }) {
  const cookieStore = await cookies();
  const lang = resolveUiLanguage(cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const theme = resolveUiTheme(cookieStore.get(UI_THEME_COOKIE)?.value);

  return (
    <html data-theme={theme ?? undefined} lang={lang} suppressHydrationWarning>
      <body className="app-body">
        <script dangerouslySetInnerHTML={{ __html: buildThemeInitScript(theme) }} />
        {children}
      </body>
    </html>
  );
}
