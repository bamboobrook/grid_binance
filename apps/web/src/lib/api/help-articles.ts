export const VALID_HELP_ARTICLES = ["expiry-reminder"] as const;

export function isValidHelpArticle(article: string): boolean {
  return VALID_HELP_ARTICLES.includes(article as (typeof VALID_HELP_ARTICLES)[number]);
}

export function normalizeHelpArticle(value?: string | string[]): string | null {
  const article = Array.isArray(value) ? value[0] : value;

  if (!article) {
    return null;
  }

  return isValidHelpArticle(article) ? article : null;
}
