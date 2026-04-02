import Link from "next/link";

import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { StatusBanner } from "../../../components/ui/status-banner";
import { getHelpCenterSnapshot } from "../../../lib/api/server";

export default async function HelpPage() {
  const snapshot = await getHelpCenterSnapshot();

  return (
    <>
      <StatusBanner description={snapshot.banner.description} title={snapshot.banner.title} tone={snapshot.banner.tone} />
      <AppShellSection
        description="This documented in-app help route is the shell-level entry point; article depth can continue to use repository-backed content."
        eyebrow="Help center"
        title="Help"
      >
        <Card>
          <CardHeader>
            <CardTitle>Guides</CardTitle>
            <CardDescription>Shared shell entry into user documentation.</CardDescription>
          </CardHeader>
          <CardBody>
            <ul className="text-list">
              {snapshot.guides.map((guide) => (
                <li key={guide.href}>
                  <Link href={guide.href}>{guide.label}</Link>
                </li>
              ))}
            </ul>
          </CardBody>
        </Card>
      </AppShellSection>
    </>
  );
}
