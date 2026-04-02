import Link from "next/link";

import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../components/ui/card";
import { StatusBanner } from "../../components/ui/status-banner";
import { getHomeSnapshot } from "../../lib/api/server";

export default async function HomePage() {
  const snapshot = await getHomeSnapshot();

  return (
    <>
      <StatusBanner description={snapshot.banner.description} title={snapshot.banner.title} tone={snapshot.banner.tone} />
      <Card tone="accent">
        <CardHeader>
          <CardTitle>Commercial recovery shell baseline</CardTitle>
          <CardDescription>Homepage now lives inside the shared public layout path with login and registration.</CardDescription>
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
    </>
  );
}
