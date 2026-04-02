import Link from "next/link";

import { PublicShell } from "../components/shell/public-shell";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../components/ui/card";
import { StatusBanner } from "../components/ui/status-banner";
import { getHomeSnapshot, getPublicShellSnapshot } from "../lib/api/server";

export default async function HomePage() {
  const [shellSnapshot, snapshot] = await Promise.all([getPublicShellSnapshot(), getHomeSnapshot()]);

  return (
    <PublicShell snapshot={shellSnapshot}>
      <StatusBanner description={snapshot.banner.description} title={snapshot.banner.title} tone={snapshot.banner.tone} />
      <Card tone="accent">
        <CardHeader>
          <CardTitle>Commercial recovery shell baseline</CardTitle>
          <CardDescription>Homepage now sits in the same public shell system as login and registration.</CardDescription>
        </CardHeader>
        <CardBody>
          <ul className="text-list">
            {snapshot.links.map((item) => (
              <li key={item.href}>
                <Link href={item.href}>{item.label}</Link>
              </li>
            ))}
          </ul>
        </CardBody>
      </Card>
    </PublicShell>
  );
}
