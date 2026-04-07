import type { ReactNode } from "react";
import { cookies } from "next/headers";

import { PublicShell } from "../../../components/shell/public-shell";
import { getPublicShellSnapshot } from "../../../lib/api/server";
import {
  resolveUiLanguage,
  resolveUiTheme,
  UI_LANGUAGE_COOKIE,
  UI_THEME_COOKIE,
} from "../../../lib/ui/preferences";

export default async function PublicLayout({ children }: { children: ReactNode }) {
  const [snapshot, cookieStore] = await Promise.all([getPublicShellSnapshot(), cookies()]);
  const lang = resolveUiLanguage(cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const theme = resolveUiTheme(cookieStore.get(UI_THEME_COOKIE)?.value);

  return (
    <PublicShell lang={lang} snapshot={snapshot} theme={theme}>
      {children}
    </PublicShell>
  );
}
