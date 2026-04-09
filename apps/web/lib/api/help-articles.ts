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
    .trim();
}

function toBlocks(lines: string[]): HelpArticleBlock[] {
  const blocks: HelpArticleBlock[] = [];

  for (let index = 0; index < lines.length; index += 1) {
    const line = lines[index];

    if (/^#{2,6}\s+/.test(line)) {
      const heading = line.match(/^(#{2,6})\s+(.*)$/);
      if (heading) {
        blocks.push({ kind: "heading", level: heading[1].length, text: heading[2].trim() });
      }
      continue;
    }

    if (/^[-*]\s+/.test(line)) {
      const items: string[] = [];
      while (index < lines.length && /^[-*]\s+/.test(lines[index])) {
        items.push(stripMarkdown(lines[index]));
        index += 1;
      }
      index -= 1;
      blocks.push({ items, kind: "unordered-list" });
      continue;
    }

    if (/^\d+\.\s+/.test(line)) {
      const items: string[] = [];
      while (index < lines.length && /^\d+\.\s+/.test(lines[index])) {
        items.push(stripMarkdown(lines[index]));
        index += 1;
      }
      index -= 1;
      blocks.push({ items, kind: "ordered-list" });
      continue;
    }

    blocks.push({ kind: "paragraph", text: line });
  }

  return blocks;
}

function parseArticle(slug: string, locale?: string): HelpArticle {
  const content = readFileSync(path.join(docsDirectory(locale), `${slug}.md`), "utf8");
  const lines = content.split(/\r?\n/).map((line) => line.trim());

  const titleIndex = lines.findIndex((line) => line.startsWith("# "));
  const titleLine = titleIndex >= 0 ? lines[titleIndex] : "# Untitled";
  const title = titleLine.replace(/^#\s+/, "");
  const body = lines.slice(titleIndex >= 0 ? titleIndex + 1 : 0).filter(Boolean);
  const summaryLine = body.find((line) => !/^#{1,6}\s+/.test(line) && !/^[-*]\s+/.test(line) && !/^\d+\.\s+/.test(line));
  const summary = stripMarkdown(summaryLine ?? body[0] ?? "Repository-backed user help article.");

  return {
    blocks: toBlocks(body),
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
