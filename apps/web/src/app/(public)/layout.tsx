import type { ReactNode } from "react";

import { PublicShell } from "../../components/shell/public-shell";
import { getPublicShellSnapshot } from "../../lib/api/server";

export default async function PublicLayout({ children }: { children: ReactNode }) {
  const snapshot = await getPublicShellSnapshot();

  return <PublicShell snapshot={snapshot}>{children}</PublicShell>;
}
