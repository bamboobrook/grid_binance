import Link from "next/link";
import { notFound } from "next/navigation";

import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { StatusBanner } from "@/components/ui/status-banner";
import { getHelpArticle, type HelpArticleBlock } from "@/lib/api/help-articles";

function renderArticleBlock(block: HelpArticleBlock, index: number) {
  if (block.kind === "heading") {
    const HeadingTag = block.level <= 2 ? "h2" : "h3";
    return <HeadingTag key={`${block.kind}-${index}`}>{block.text}</HeadingTag>;
  }

  if (block.kind === "unordered-list") {
    return (
      <ul key={`${block.kind}-${index}`} className="text-list">
        {block.items.map((item) => (
          <li key={item}>{item}</li>
        ))}
      </ul>
    );
  }

  if (block.kind === "ordered-list") {
    return (
      <ol key={`${block.kind}-${index}`} className="text-list">
        {block.items.map((item) => (
          <li key={item}>{item}</li>
        ))}
      </ol>
    );
  }

  if (block.kind === "paragraph") {
    return <p key={`${block.kind}-${index}`}>{block.text}</p>;
  }

  return null;
}

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
        <StatusBanner description={article.summary} title={article.title} />
        <Card>
          <CardHeader>
            <CardTitle>{article.title}</CardTitle>
            <CardDescription>Public help route showing the same repository-backed content rendered in /app/help.</CardDescription>
          </CardHeader>
          <CardBody>
            {article.blocks.map((block, index) => renderArticleBlock(block, index))}
            <div className="button-row">
              <Link className="button button--ghost" href={`/app/help?article=${article.slug}`}>
                Open in App Help Center
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
