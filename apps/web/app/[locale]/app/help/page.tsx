import Link from "next/link";
import { notFound } from "next/navigation";
import { cookies } from "next/headers";

import { AppShellSection } from "@/components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { StatusBanner } from "@/components/ui/status-banner";
import { getHelpArticle, listHelpArticles, normalizeHelpArticle, type HelpArticleBlock } from "@/lib/api/help-articles";
import { UI_LANGUAGE_COOKIE, pickText, resolveUiLanguageFromRoute } from "@/lib/ui/preferences";

type HelpPageProps = {
  params: Promise<{ locale: string }>;
  searchParams?: Promise<{
    article?: string | string[];
  }>;
};

function renderArticleBlock(block: HelpArticleBlock, index: number) {
  if (block.kind === "heading") {
    const HeadingTag = block.level <= 2 ? "h3" : "h4";
    return <HeadingTag key={block.kind + "-" + index}>{block.text}</HeadingTag>;
  }

  if (block.kind === "unordered-list") {
    return (
      <ul key={block.kind + "-" + index} className="text-list">
        {block.items.map((item) => (
          <li key={item}>{item}</li>
        ))}
      </ul>
    );
  }

  if (block.kind === "ordered-list") {
    return (
      <ol key={block.kind + "-" + index} className="text-list">
        {block.items.map((item) => (
          <li key={item}>{item}</li>
        ))}
      </ol>
    );
  }

  if (block.kind === "paragraph") {
    return <p key={block.kind + "-" + index}>{block.text}</p>;
  }

  return null;
}

export default async function HelpPage({ params, searchParams }: HelpPageProps) {
  const { locale } = await params;
  const cookieStore = await cookies();
  const lang = resolveUiLanguageFromRoute(locale, cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const friendlyArticles = listHelpArticles(locale);
  const articles = friendlyArticles;
  const requestedArticle = (await searchParams)?.article;
  const articleSlug = normalizeHelpArticle(requestedArticle, locale);

  if (requestedArticle && articleSlug === null) {
    notFound();
  }

  const article = articleSlug ? getHelpArticle(articleSlug, locale) : null;
  const selectedArticle = article ?? articles[0];

  return (
    <>
      <StatusBanner
        description={pickText(lang, "这里展示的是给用户看的操作说明，重点是怎么做，不是开发文档。", "These guides focus on what to do next in plain language instead of developer-facing notes.")}
        title={pickText(lang, "帮助中心", "Help center")}
       
      />
      <AppShellSection
        description={pickText(lang, "左侧选问题，右侧直接看做法。尽量少用生硬术语。", "Pick a topic on the left and read the action steps on the right.")}
        eyebrow={pickText(lang, "帮助中心", "Help center")}
        title={pickText(lang, "帮助中心", "Help Center")}
      >
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "指南列表", "Guides")}</CardTitle>
              <CardDescription>{pickText(lang, "每一条都是可直接照着做的使用说明。", "Each guide is written as a direct usage walkthrough.")}</CardDescription>
            </CardHeader>
            <CardBody>
              <ul className="text-list">
                {articles.map((item) => (
                  <li key={item.slug}>
                    <Link href={`/${locale}/app/help?article=${item.slug}`}>
                      {item.slug === "expiry-reminder" ? pickText(lang, "到期提醒指南", "Expiry reminder guide") : item.title}
                    </Link>
                    <br />
                    <span>{item.summary}</span>
                  </li>
                ))}
              </ul>
            </CardBody>
          </Card>
          <Card>
            <CardHeader>
              <CardTitle>{selectedArticle.title}</CardTitle>
              <CardDescription>
                {article
                  ? selectedArticle.summary
                  : pickText(lang, "当前先展示默认指南，左侧点一下就能切换到其它说明。", "The default guide is shown first. Pick any topic on the left to switch.")}
              </CardDescription>
            </CardHeader>
            <CardBody>{selectedArticle.blocks.map((block, index) => renderArticleBlock(block, index))}</CardBody>
          </Card>
        </div>
      </AppShellSection>
    </>
  );
}
