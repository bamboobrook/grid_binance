import type { ReactNode } from "react";

import { UserShell } from "../../components/shell/user-shell";
import { getUserShellSnapshot } from "../../lib/api/server";

export default async function UserAppLayout({ children }: { children: ReactNode }) {
  const snapshot = await getUserShellSnapshot("/app/dashboard");

  return <UserShell snapshot={snapshot}>{children}</UserShell>;
}
