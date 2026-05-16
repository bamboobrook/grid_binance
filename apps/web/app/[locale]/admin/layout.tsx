import type { ReactNode } from "react";
import { cookies } from "next/headers";
import { redirect } from "next/navigation";

import { AdminShell } from "@/components/shell/admin-shell";
import { getCurrentAdminProfile } from "@/lib/api/admin-product-state";
import { getAdminShellSnapshot } from "@/lib/api/server";
import {
  pickText,
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
  const [{ locale }, cookieStore] = await Promise.all([params, cookies()]);
  const lang = resolveUiLanguageFromRoute(locale, cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const theme = resolveUiTheme(cookieStore.get(UI_THEME_COOKIE)?.value);

  const profile = await getCurrentAdminProfile().catch(() => null);
  if (!profile?.admin_access_granted) {
    const error = encodeURIComponent(
      pickText(lang, "请先使用管理员账号完成登录与 TOTP 验证。", "Sign in with an admin account and complete the TOTP challenge first."),
    );
    redirect("/" + locale + "/admin/login?error=" + error);
  }

  const snapshot = await getAdminShellSnapshot(locale);

  return (
    <AdminShell lang={lang} locale={locale} snapshot={snapshot} theme={theme}>
      {children}
    </AdminShell>
  );
}

