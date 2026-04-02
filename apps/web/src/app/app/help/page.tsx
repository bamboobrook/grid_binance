import Link from "next/link";
import { notFound } from "next/navigation";

import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { StatusBanner } from "../../../components/ui/status-banner";
import { getHelpArticle, HELP_ARTICLES, normalizeHelpArticle } from "../../../lib/api/help-articles";

type HelpPageProps = {
  searchParams?: Promise<{
    article?: string | string[];
  }>;
};

export default async function HelpPage({ searchParams }: HelpPageProps) {
  const requestedArticle = (await searchParams)?.article;
  const articleSlug = normalizeHelpArticle(requestedArticle);

  if (requestedArticle && !articleSlug) {
    notFound();
  }

  const article = articleSlug ? getHelpArticle(articleSlug) : null;

  return (
    <>
      <StatusBanner
        description="The in-app help center mirrors repository-backed user guidance for billing, strategy, and security flows."
        title="Help center"
        tone="success"
      />
      <AppShellSection
        description="Use the help center to move from concept questions into the exact route where the action happens."
        eyebrow="Help center"
        title="Help Center"
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>Guides</CardTitle>
              <CardDescription>User-facing documentation shared between the app shell and the standalone article route.</CardDescription>
            </CardHeader>
            <CardBody>
              <ul className="text-list">
                {HELP_ARTICLES.map((item) => (
                  <li key={item.slug}>
                    <Link href={`/help/${item.slug}`}>{item.slug === "expiry-reminder" ? "Expiry reminder guide" : item.title}</Link>
                    <br />
                    <span>{item.summary}</span>
                  </li>
                ))}
              </ul>
            </CardBody>
          </Card>
          <Card tone="subtle">
            <CardHeader>
              <CardTitle>{article ? article.title : "Recommended next steps"}</CardTitle>
              <CardDescription>{article ? article.summary : "Pick the guide that matches the blocker in your current workflow."}</CardDescription>
            </CardHeader>
            <CardBody>
              {article ? (
                <ul className="text-list">
                  {article.body.map((paragraph) => (
                    <li key={paragraph}>{paragraph}</li>
                  ))}
                </ul>
              ) : (
                <ul className="text-list">
                  <li>
                    <Link href="/app/billing">Open Billing Center</Link>
                  </li>
                  <li>
                    <Link href="/app/security">Open Security Center</Link>
                  </li>
                  <li>
                    <Link href="/app/strategies/new">Create a new strategy draft</Link>
                  </li>
                </ul>
              )}
            </CardBody>
          </Card>
        </div>
      </AppShellSection>
    </>
  );
}
