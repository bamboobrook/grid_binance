import Link from "next/link";
import { notFound } from "next/navigation";

import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { StatusBanner } from "../../../components/ui/status-banner";
import { getHelpArticle } from "../../../lib/api/help-articles";

export default async function HelpArticlePage({
  params,
}: {
  params: Promise<{ slug: string }>;
}) {
  const { slug } = await params;
  const article = getHelpArticle(slug);

  if (!article) {
    notFound();
  }

  return (
    <main className="shell shell--public">
      <div className="public-shell__content">
        <StatusBanner description={article.summary} title={article.title} tone="info" />
        <Card tone="accent">
          <CardHeader>
            <CardTitle>{article.title}</CardTitle>
            <CardDescription>{article.body[0]}</CardDescription>
          </CardHeader>
          <CardBody>
            <ul className="text-list">
              {article.body.slice(1).map((paragraph) => (
                <li key={paragraph}>{paragraph}</li>
              ))}
            </ul>
            <div className="button-row">
              <Link className="button button--ghost" href="/app/help">
                Back to Help Center
              </Link>
              <Link className="button" href="/app/billing">
                Open Billing Center
              </Link>
            </div>
          </CardBody>
        </Card>
      </div>
    </main>
  );
}
