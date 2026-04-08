import type { ReactNode } from "react";
import { cookies } from "next/headers";

import { UserShell } from "@/components/shell/user-shell";
import { getUserShellSnapshot } from "@/lib/api/server";
import {
  resolveUiLanguageFromRoute,
  resolveUiTheme,
  UI_LANGUAGE_COOKIE,
  UI_THEME_COOKIE,
} from "@/lib/ui/preferences";

export default async function UserAppLayout({
  children,
  params,
}: {
  children: ReactNode;
  params: Promise<{ locale: string }>;
}) {
  const { locale } = await params;
  const cookieStore = await cookies();
  const lang = resolveUiLanguageFromRoute(locale, cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const theme = resolveUiTheme(cookieStore.get(UI_THEME_COOKIE)?.value);
  const snapshot = await getUserShellSnapshot(locale);

  return (
    <UserShell lang={lang} locale={locale} snapshot={snapshot} theme={theme}>
      {children}
    </UserShell>
  );
}
