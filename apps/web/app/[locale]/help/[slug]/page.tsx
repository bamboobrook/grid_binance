import Link from "next/link";
import { notFound } from "next/navigation";

import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { StatusBanner } from "@/components/ui/status-banner";
import { getHelpArticle, type HelpArticleBlock } from "@/lib/api/help-articles";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";

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
  params: Promise<{ locale: string; slug: string }>;
}) {
  const { locale, slug } = await params;
  const lang: UiLanguage = locale === "en" ? "en" : "zh";
  const article = getHelpArticle(slug, locale);

  if (!article) {
    notFound();
  }

  return (
    <main className="shell shell--public">
      <div className="public-shell__content py-6">
        <StatusBanner description={article.summary} title={article.title} />
        <Card>
          <CardHeader>
            <CardTitle>{article.title}</CardTitle>
            <CardDescription>
              {pickText(lang, "公开帮助页展示的是同一套用户说明，未登录也能看。", "The public help page shows the same user guides, even before sign-in.")}
            </CardDescription>
          </CardHeader>
          <CardBody>
            <div className="ui-form">{article.blocks.map((block, index) => renderArticleBlock(block, index))}</div>
            <div className="flex flex-wrap items-center gap-2 mt-6">
              <Link className="inline-flex items-center justify-center rounded-sm text-sm font-medium h-9 px-4 py-2 hover:bg-secondary text-foreground transition-colors" href={`/${locale}/app/help?article=${article.slug}`}>
                {pickText(lang, "在应用内打开帮助中心", "Open in App Help Center")}
              </Link>
              <Link className="inline-flex items-center justify-center rounded-sm text-sm font-medium h-9 px-4 py-2 bg-primary hover:bg-primary/90 text-primary-foreground shadow-sm transition-colors" href={`/${locale}/app/billing`}>
                {pickText(lang, "打开计费中心", "Open Billing Center")}
              </Link>
            </div>
          </CardBody>
        </Card>
      </div>
    </main>
  );
}
