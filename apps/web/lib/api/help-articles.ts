import { existsSync, readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

export type HelpArticleBlock =
  | { items: string[]; kind: "ordered-list" | "unordered-list" }
  | { kind: "heading"; level: number; text: string }
  | { kind: "paragraph"; text: string };

export type HelpArticle = {
  blocks: HelpArticleBlock[];
  body: string[];
  slug: string;
  summary: string;
  title: string;
};

function docsDirectory(locale?: string) {
  const starts = [process.cwd(), path.dirname(fileURLToPath(import.meta.url))];

  for (const start of starts) {
    let current = start;
    while (true) {
      const localizedCandidate = locale ? path.join(current, "docs", "user-guide", locale) : null;
      if (localizedCandidate && existsSync(localizedCandidate)) {
        return localizedCandidate;
      }

      const candidate = path.join(current, "docs", "user-guide");
      if (existsSync(candidate)) {
        return candidate;
      }

      const parent = path.dirname(current);
      if (parent === current) {
        break;
      }
      current = parent;
    }
  }

  throw new Error("docs/user-guide directory not found");
}

function stripMarkdown(line: string): string {
  return line
    .replace(/^#{1,6}\s+/, "")
    .replace(/^[-*]\s+/, "")
    .replace(/^\d+\.\s+/, "")
    .replace(/`([^`]+)`/g, "$1")
    .replace(/\s+/g, " ")
    .trim();
}

function toBlocks(lines: string[], locale?: string): HelpArticleBlock[] {
  const blocks: HelpArticleBlock[] = [];

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];

    if (/^#{2,6}\s+/.test(line)) {
      const heading = line.match(/^(#{2,6})\s+(.*)$/);
      if (heading) {
        blocks.push({ kind: "heading", level: heading[1].length, text: humanizeHelpText(heading[2].trim(), locale) });
      }
      continue;
    }

    if (/^[-*]\s+/.test(line)) {
      const items: string[] = [];
      while (index < lines.length && /^[-*]\s+/.test(lines[index])) {
        const item = humanizeHelpText(stripMarkdown(lines[index]), locale);
        if (item) {
          items.push(item);
        }
        index += 1;
      }
      index -= 1;
      if (items.length > 0) {
        blocks.push({ items, kind: "unordered-list" });
      }
      continue;
    }

    if (/^\d+\.\s+/.test(line)) {
      const items: string[] = [];
      while (index < lines.length && /^\d+\.\s+/.test(lines[index])) {
        const item = humanizeHelpText(stripMarkdown(lines[index]), locale);
        if (item) {
          items.push(item);
        }
        index += 1;
      }
      index -= 1;
      if (items.length > 0) {
        blocks.push({ items, kind: "ordered-list" });
      }
      continue;
    }

    const text = humanizeHelpText(line, locale);
    if (text) {
      blocks.push({ kind: "paragraph", text });
    }
  }

  return blocks;
}

function parseArticle(slug: string, locale?: string): HelpArticle {
  const content = readFileSync(path.join(docsDirectory(locale), `${slug}.md`), "utf8");
  const lines = content.split(/\r?\n/).map((line) => line.trim());

  const titleIndex = lines.findIndex((line) => line.startsWith("# "));
  const titleLine = titleIndex >= 0 ? lines[titleIndex] : "# Untitled";
  const title = humanizeHelpText(titleLine.replace(/^#\s+/, ""), locale);
  const rawBody = lines.slice(titleIndex >= 0 ? titleIndex + 1 : 0).filter(Boolean);
  const body = sanitizeArticleBody(rawBody, locale);
  const summaryLine = body.find((line) => !/^#{1,6}\s+/.test(line) && !/^[-*]\s+/.test(line) && !/^\d+\.\s+/.test(line));
  const summary = humanizeHelpText(stripMarkdown(summaryLine ?? body[0] ?? defaultSummary(locale)), locale);

  return {
    blocks: toBlocks(body, locale),
    body,
    slug,
    summary,
    title,
  };
}

export function listHelpArticles(locale?: string): HelpArticle[] {
  return readdirSync(docsDirectory(locale))
    .filter((entry) => entry.endsWith(".md"))
    .map((entry) => entry.replace(/\.md$/, ""))
    .sort()
    .map((slug) => parseArticle(slug, locale));
}

export const HELP_ARTICLES = listHelpArticles();
export const VALID_HELP_ARTICLES = HELP_ARTICLES.map((article) => article.slug) as [string, ...string[]];

export function getHelpArticle(article: string, locale?: string): HelpArticle | null {
  return listHelpArticles(locale).find((item) => item.slug === article) ?? null;
}

export function isValidHelpArticle(article: string, locale?: string): boolean {
  return getHelpArticle(article, locale) !== null;
}

export function normalizeHelpArticle(value?: string | string[], locale?: string): string | null {
  const article = Array.isArray(value) ? value[0] : value;
  if (!article) {
    return null;
  }
  return isValidHelpArticle(article, locale) ? article : null;
}

function sanitizeArticleBody(lines: string[], locale?: string) {
  const output: string[] = [];
  let skipUntilNextSection = false;

  for (const line of lines) {
    if (/^##\s+/.test(line)) {
      const heading = stripMarkdown(line);
      if (isDeveloperOnlySection(heading, locale)) {
        skipUntilNextSection = true;
        continue;
      }
      skipUntilNextSection = false;
    }

    if (skipUntilNextSection) {
      continue;
    }

    output.push(line);
  }

  return output;
}

function isDeveloperOnlySection(heading: string, locale?: string) {
  const normalized = heading.trim().toLowerCase();
  return normalized === "local stack" || heading.trim() === "本地环境";
}

function humanizeHelpText(line: string, locale?: string) {
  const trimmed = stripMarkdown(line);
  if (!trimmed) {
    return "";
  }

  let text = trimmed;
  if (isChinese(locale)) {
    text = text
      .replace(/^登录后可以通过 .*? 查看本指南；登录前也可以通过公开路由 .*? 查看。$/, "登录后可在帮助中心直接查看这篇说明，未登录也能在公开帮助页查看。")
      .replace(/^Use the in-app help route .*$/, "")
      .replace(/\/app\/dashboard/g, "总览页")
      .replace(/\/app\/exchange/g, "交易所页面")
      .replace(/\/app\/strategies\/new/g, "创建策略页面")
      .replace(/\/app\/strategies/g, "策略页面")
      .replace(/\/app\/orders/g, "订单页面")
      .replace(/\/app\/billing/g, "会员中心")
      .replace(/\/app\/telegram/g, "Telegram 页面")
      .replace(/\/app\/security/g, "安全中心")
      .replace(/\/admin-bootstrap/g, "管理员首次安全设置页")
      .replace(/docs\/user-guide/g, "帮助资料")
      .replace(/levels_json/g, "内部参数格式")
      .replace(/<slug>/g, "文章名")
      .replace(/JSON/g, "内部参数格式");
  } else {
    text = text
      .replace(/^Use the in-app help route .*$/, "Open this guide from the Help Center after sign-in, or read it from the public help page before login.")
      .replace(/^登录后可以通过 .*$/, "")
      .replace(/\/app\/dashboard/g, "the Dashboard")
      .replace(/\/app\/exchange/g, "the Exchange page")
      .replace(/\/app\/strategies\/new/g, "the Create Strategy page")
      .replace(/\/app\/strategies/g, "the Strategies page")
      .replace(/\/app\/orders/g, "the Orders page")
      .replace(/\/app\/billing/g, "the Membership Center")
      .replace(/\/app\/telegram/g, "the Telegram page")
      .replace(/\/app\/security/g, "the Security Center")
      .replace(/\/admin-bootstrap/g, "the first admin security setup page")
      .replace(/docs\/user-guide/g, "the Help Center")
      .replace(/levels_json/g, "the internal strategy format")
      .replace(/<slug>/g, "an article name")
      .replace(/JSON/g, "the internal strategy format");
  }

  return text.replace(/\s+/g, " ").trim();
}

function defaultSummary(locale?: string) {
  return isChinese(locale) ? "这里会告诉你这个功能怎么用。" : "This guide explains the feature in plain language.";
}

function isChinese(locale?: string) {
  return typeof locale === "string" && locale.toLowerCase().startsWith("zh");
}
