import type { ReactNode } from "react";
import { cookies } from "next/headers";
import { redirect } from "next/navigation";

import { UserShell } from "@/components/shell/user-shell";
import {
  getCurrentProfile,
  getUserExpiryNotification,
  getUserShellSnapshot,
} from "@/lib/api/server";
import { localizeNotificationMessage, localizeNotificationTitle } from "@/lib/ui/domain-copy";
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
  const [{ locale }, cookieStore] = await Promise.all([params, cookies()]);
  const lang = resolveUiLanguageFromRoute(locale, cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const theme = resolveUiTheme(cookieStore.get(UI_THEME_COOKIE)?.value);
  const [profile, snapshot, expiryNotice] = await Promise.all([
    getCurrentProfile(),
    getUserShellSnapshot(locale),
    getUserExpiryNotification(),
  ]);
  if (profile?.admin_totp_required) {
    redirect(`/${locale}/admin/login?email=${encodeURIComponent(profile.email ?? "")}`);
  }
  const expiryReminder = expiryNotice
    ? {
        description: localizeNotificationMessage(lang, expiryNotice.event.kind, expiryNotice.event.message),
        title: localizeNotificationTitle(lang, expiryNotice.event.kind, expiryNotice.event.title),
      }
    : null;

  return (
    <UserShell expiryReminder={expiryReminder} lang={lang} locale={locale} snapshot={snapshot} theme={theme}>
      {children}
    </UserShell>
  );
}
