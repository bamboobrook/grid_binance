import type { ReactNode } from "react";
import { ModernShell } from "../../components/layout/modern-shell";

export default async function UserAppLayout({ children }: { children: ReactNode }) {
  return (
    <ModernShell>
      {children}
    </ModernShell>
  );
}
