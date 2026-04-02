import Link from "next/link";

import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { StatusBanner } from "../../../components/ui/status-banner";
import { getHelpCenterSnapshot } from "../../../lib/api/server";

type HelpPageProps = {
  searchParams?: Promise<{
    article?: string | string[];
  }>;
};

function firstValue(value?: string | string[]) {
  return Array.isArray(value) ? value[0] : value;
}

export default async function HelpPage({ searchParams }: HelpPageProps) {
  const snapshot = await getHelpCenterSnapshot();
  const article = firstValue((await searchParams)?.article);

  return (
    <>
      <StatusBanner description={snapshot.banner.description} title={snapshot.banner.title} tone={snapshot.banner.tone} />
      <AppShellSection
        description="This documented in-app help route is the shell-level entry point; legacy /help articles now consolidate here."
        eyebrow="Help center"
        title="Help"
      >
        <div className="content-grid content-grid--split">
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
          {article ? (
            <Card tone="accent">
              <CardHeader>
                <CardTitle>Selected article</CardTitle>
                <CardDescription>{article.replaceAll("-", " ")}</CardDescription>
              </CardHeader>
              <CardBody>
                Legacy `/help/{'{'}slug{'}'}` routes now land inside the documented `/app/help` shell surface.
              </CardBody>
            </Card>
          ) : null}
        </div>
      </AppShellSection>
    </>
  );
}
