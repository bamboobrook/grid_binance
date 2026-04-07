import type { ReactNode } from "react";

import { UserShell } from "../../components/shell/user-shell";
import { DialogFrame } from "../../components/ui/dialog";
import { getUserExpiryNotification, getUserShellSnapshot } from "../../lib/api/server";

export default async function UserAppLayout({ children }: { children: ReactNode }) {
  const [snapshot, expiryNotification] = await Promise.all([
    getUserShellSnapshot(),
    getUserExpiryNotification(),
  ]);

  return (
    <UserShell snapshot={snapshot}>
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
