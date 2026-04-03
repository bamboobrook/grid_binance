import Link from "next/link";
import { notFound } from "next/navigation";

import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { StatusBanner } from "../../../components/ui/status-banner";
import { getHelpArticle, HELP_ARTICLES, normalizeHelpArticle, type HelpArticleBlock } from "../../../lib/api/help-articles";

type HelpPageProps = {
  searchParams?: Promise<{
    article?: string | string[];
  }>;
};

function renderArticleBlock(block: HelpArticleBlock, index: number) {
  if (block.kind === "heading") {
    const HeadingTag = block.level <= 2 ? "h3" : "h4";
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

  return <p key={`${block.kind}-${index}`}>{block.text}</p>;
}

export default async function HelpPage({ searchParams }: HelpPageProps) {
  const requestedArticle = (await searchParams)?.article;
  const articleSlug = normalizeHelpArticle(requestedArticle);

  if (requestedArticle && !articleSlug) {
    notFound();
  }

  const article = articleSlug ? getHelpArticle(articleSlug) : null;
  const selectedArticle = article ?? HELP_ARTICLES[0];

  return (
    <>
      <StatusBanner
        description="The in-app help center now renders the same repository-backed guides that live under docs/user-guide/*.md."
        title="Help center"
        tone="success"
      />
      <AppShellSection
        description="Open a guide on the left and read the full repository document without leaving the app shell."
        eyebrow="Help center"
        title="Help Center"
      >
        <div className="content-grid content-grid--split">
          <Card>
            <CardHeader>
              <CardTitle>Guides</CardTitle>
              <CardDescription>Every entry below is loaded from the matching file in docs/user-guide.</CardDescription>
            </CardHeader>
            <CardBody>
              <ul className="text-list">
                {HELP_ARTICLES.map((item) => (
                  <li key={item.slug}>
                    <Link href={`/app/help?article=${item.slug}`}>
                      {item.slug === "expiry-reminder" ? "Expiry reminder guide" : item.title}
                    </Link>
                    <br />
                    <span>{item.summary}</span>
                  </li>
                ))}
              </ul>
            </CardBody>
          </Card>
          <Card tone="subtle">
            <CardHeader>
              <CardTitle>{selectedArticle.title}</CardTitle>
              <CardDescription>
                {article
                  ? selectedArticle.summary
                  : "Showing the default repository guide until you choose a specific article."}
              </CardDescription>
            </CardHeader>
            <CardBody>{selectedArticle.blocks.map((block, index) => renderArticleBlock(block, index))}</CardBody>
          </Card>
        </div>
      </AppShellSection>
    </>
  );
}
