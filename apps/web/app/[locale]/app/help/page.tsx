import Link from "next/link";
import { notFound } from "next/navigation";
import { cookies } from "next/headers";

import { AppShellSection } from "@/components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { StatusBanner } from "@/components/ui/status-banner";
import { getHelpArticle, HELP_ARTICLES, normalizeHelpArticle, type HelpArticleBlock } from "@/lib/api/help-articles";
import { UI_LANGUAGE_COOKIE, pickText, resolveUiLanguage } from "@/lib/ui/preferences";

type HelpPageProps = {
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

export default async function HelpPage({ searchParams }: HelpPageProps) {
  const cookieStore = await cookies();
  const lang = resolveUiLanguage(cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const requestedArticle = (await searchParams)?.article;
  const articleSlug = normalizeHelpArticle(requestedArticle);

  if (requestedArticle && articleSlug === null) {
    notFound();
  }

  const article = articleSlug ? getHelpArticle(articleSlug) : null;
  const selectedArticle = article ?? HELP_ARTICLES[0];

  return (
    <>
      <StatusBanner
        description={pickText(lang, "应用内帮助中心直接渲染仓库里的 docs/user-guide 文档。", "The in-app help center renders repository-backed guides from docs/user-guide.")}
        title={pickText(lang, "帮助中心状态条", "Help center status strip")}
       
      />
      <AppShellSection
        description={pickText(lang, "左侧选指南，右侧直接阅读文档正文，不离开应用壳层。", "Choose a guide on the left and read the full repository document on the right without leaving the app shell.")}
        eyebrow={pickText(lang, "帮助中心", "Help center")}
        title={pickText(lang, "帮助中心", "Help Center")}
      >
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-4">
          <Card>
            <CardHeader>
              <CardTitle>{pickText(lang, "指南列表", "Guides")}</CardTitle>
              <CardDescription>{pickText(lang, "每个条目都来自 docs/user-guide 下的对应文件。", "Every entry below is loaded from the matching file in docs/user-guide.")}</CardDescription>
            </CardHeader>
            <CardBody>
              <ul className="text-list">
                {HELP_ARTICLES.map((item) => (
                  <li key={item.slug}>
                    <Link href={"/app/help?article=" + item.slug}>
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
                  : pickText(lang, "当前展示默认指南，选中左侧条目后会切换具体文章。", "Showing the default repository guide until you choose a specific article.")}
              </CardDescription>
            </CardHeader>
            <CardBody>{selectedArticle.blocks.map((block, index) => renderArticleBlock(block, index))}</CardBody>
          </Card>
        </div>
      </AppShellSection>
    </>
  );
}
