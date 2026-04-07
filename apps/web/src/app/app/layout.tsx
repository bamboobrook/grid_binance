import type { ReactNode } from "react";
import { cookies } from "next/headers";

import { UserShell } from "../../components/shell/user-shell";
import { DialogFrame } from "../../components/ui/dialog";
import { getUserExpiryNotification, getUserShellSnapshot } from "../../lib/api/server";
import {
  resolveUiLanguage,
  resolveUiTheme,
  UI_LANGUAGE_COOKIE,
  UI_THEME_COOKIE,
} from "../../lib/ui/preferences";

export default async function UserAppLayout({ children }: { children: ReactNode }) {
  const [snapshot, expiryNotification, cookieStore] = await Promise.all([
    getUserShellSnapshot(),
    getUserExpiryNotification(),
    cookies(),
  ]);
  const lang = resolveUiLanguage(cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const theme = resolveUiTheme(cookieStore.get(UI_THEME_COOKIE)?.value);

  return (
    <UserShell lang={lang} snapshot={snapshot} theme={theme}>
      {expiryNotification ? (
        <DialogFrame
          title={expiryNotification.event.title}
          description={expiryNotification.event.message}
          tone="warning"
          modal
        />
      ) : null}
      {children}
    </UserShell>
  );
}
