import type { ReactNode } from "react";

import { AdminShell } from "../../components/shell/admin-shell";
import { getAdminShellSnapshot } from "../../lib/api/server";

export default async function AdminAppLayout({ children }: { children: ReactNode }) {
  const snapshot = await getAdminShellSnapshot();

  return <AdminShell snapshot={snapshot}>{children}</AdminShell>;
}
