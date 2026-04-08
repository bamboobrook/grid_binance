import type { ReactNode } from "react";
import { cookies } from "next/headers";

import { AdminShell } from "@/components/shell/admin-shell";
import { getAdminShellSnapshot } from "@/lib/api/server";
import {
  resolveUiLanguage,
  resolveUiTheme,
  UI_LANGUAGE_COOKIE,
  UI_THEME_COOKIE,
} from "@/lib/ui/preferences";

export default async function AdminAppLayout({ children }: { children: ReactNode }) {
  const [snapshot, cookieStore] = await Promise.all([getAdminShellSnapshot(), cookies()]);
  const lang = resolveUiLanguage(cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const theme = resolveUiTheme(cookieStore.get(UI_THEME_COOKIE)?.value);

  return (
    <AdminShell lang={lang} snapshot={snapshot} theme={theme}>
      {children}
    </AdminShell>
  );
}
