import { existsSync, readFileSync, readdirSync } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

export type HelpArticle = {
  body: string[];
  slug: string;
  summary: string;
  title: string;
};

function docsDirectory() {
  const starts = [
    process.cwd(),
    path.dirname(fileURLToPath(import.meta.url)),
  ];

  for (const start of starts) {
    let current = start;
    while (true) {
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

function parseArticle(slug: string): HelpArticle {
  const content = readFileSync(path.join(docsDirectory(), `${slug}.md`), "utf8");
  const lines = content
    .split(/\r?\n/)
    .map((line) => line.trim())
    .filter(Boolean);

  const titleLine = lines.find((line) => line.startsWith("# ")) ?? "# Untitled";
  const title = titleLine.replace(/^#\s+/, "");
  const paragraphs = lines.filter((line) => !line.startsWith("# "));
  const summary = paragraphs[0] ?? "Repository-backed user help article.";

  return {
    slug,
    title,
    summary,
    body: paragraphs.slice(1),
  };
}

export function listHelpArticles(): HelpArticle[] {
  return readdirSync(docsDirectory())
    .filter((entry) => entry.endsWith(".md"))
    .map((entry) => entry.replace(/\.md$/, ""))
    .sort()
    .map((slug) => parseArticle(slug));
}

export const HELP_ARTICLES = listHelpArticles();
export const VALID_HELP_ARTICLES = HELP_ARTICLES.map((article) => article.slug) as [string, ...string[]];

export function getHelpArticle(article: string): HelpArticle | null {
  return HELP_ARTICLES.find((item) => item.slug === article) ?? null;
}

export function isValidHelpArticle(article: string): boolean {
  return getHelpArticle(article) !== null;
}

export function normalizeHelpArticle(value?: string | string[]): string | null {
  const article = Array.isArray(value) ? value[0] : value;

  if (!article) {
    return null;
  }

  return isValidHelpArticle(article) ? article : null;
}
