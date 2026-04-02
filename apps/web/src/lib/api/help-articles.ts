export type HelpArticle = {
  body: string[];
  slug: string;
  summary: string;
  title: string;
};

export const HELP_ARTICLES: HelpArticle[] = [
  {
    slug: "expiry-reminder",
    title: "Expiry Reminder",
    summary: "Understand the 48-hour grace period and what happens when membership renewal is delayed.",
    body: [
      "Expiry And Grace Period",
      "Existing running strategies may continue for 48 hours after membership expiry, but new starts stay blocked until renewal is confirmed.",
      "After the grace period ends, the platform auto-pauses running strategies and shows recovery guidance in the dashboard, billing center, and Telegram alerts.",
      "Renewal stacking is allowed, but each payment order must be completed with the exact chain, token, and amount shown on the billing page.",
    ],
  },
  {
    slug: "create-grid-strategy",
    title: "Create Grid Strategy",
    summary: "Review draft creation, pre-flight validation, and start requirements before your first launch.",
    body: [
      "Drafts can be edited freely until you run pre-flight and start the strategy.",
      "Running strategy parameters cannot be hot-modified. Pause, save edits, and re-run pre-flight before restart.",
      "Trailing take profit uses taker execution and may increase fees compared with maker-style take-profit orders.",
    ],
  },
  {
    slug: "security-center",
    title: "Security Center",
    summary: "Manage passwords, TOTP, and session review without losing visibility into account posture.",
    body: [
      "Use a unique password, enable TOTP, and revoke stale sessions whenever device trust changes.",
      "Binance API secrets stay encrypted and masked after save. Withdrawal permission must remain disabled.",
      "Telegram notifications complement web alerts for membership, API, and strategy incidents.",
    ],
  },
];

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
