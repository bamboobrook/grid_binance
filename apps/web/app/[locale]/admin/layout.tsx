import type { ReactNode } from "react";
import { cookies } from "next/headers";
import { redirect } from "next/navigation";

import { AdminShell } from "@/components/shell/admin-shell";
import { getAdminShellSnapshot } from "@/lib/api/server";
import {
  resolveUiLanguageFromRoute,
  resolveUiTheme,
  UI_LANGUAGE_COOKIE,
  UI_THEME_COOKIE,
} from "@/lib/ui/preferences";

export default async function AdminAppLayout({
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

  let snapshot;
  try {
    snapshot = await getAdminShellSnapshot(locale);
  } catch {
    redirect(`/${locale}/login?error=session+expired`);
  }

  return (
    <AdminShell lang={lang} locale={locale} snapshot={snapshot} theme={theme}>
      {children}
    </AdminShell>
  );
}
