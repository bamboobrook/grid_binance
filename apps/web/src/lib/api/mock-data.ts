export type NavItem = {
  badge?: string;
  href: string;
  label: string;
};

export type BannerSnapshot = {
  action?: NavItem;
  description: string;
  title: string;
  tone: "info" | "success" | "warning" | "danger";
};

export type QuickStat = {
  label: string;
  value: string;
};

export type PublicShellSnapshot = {
  actions: NavItem[];
  brand: string;
  description: string;
  eyebrow: string;
  highlights: Array<{ description: string; title: string }>;
  subtitle: string;
  supportLinks: NavItem[];
  title: string;
};

export type UserShellSnapshot = {
  activeHref: string;
  banners: BannerSnapshot[];
  brand: string;
  description: string;
  identity: {
    context: string;
    name: string;
    role: string;
  };
  nav: NavItem[];
  quickStats: QuickStat[];
  subtitle: string;
  title: string;
};

export type AdminShellSnapshot = UserShellSnapshot;

export const publicShellSnapshot: PublicShellSnapshot = {
  brand: "GridBinance",
  subtitle: "Binance grid SaaS control plane",
  eyebrow: "Commercial recovery plan",
  title: "Public access shell",
  description: "Shared entry flow for registration and login, with risk framing and help center guidance surfaced beside the form.",
  actions: [
    { href: "/login", label: "Login" },
    { href: "/register", label: "Register" },
    { href: "/help/expiry-reminder", label: "Help" }
  ],
  highlights: [
    {
      title: "Membership first",
      description: "Running strategies is gated by an active membership with a visible grace-period surface."
    },
    {
      title: "Explicit warnings",
      description: "Billing, API permissions, and take-profit modes stay visible instead of being hidden behind automation."
    }
  ],
  supportLinks: [
    { href: "/help/expiry-reminder", label: "Expiry reminder guide" },
    { href: "/app/dashboard", label: "User dashboard preview" },
    { href: "/admin/dashboard", label: "Admin dashboard preview" }
  ]
};

export const publicAuthSnapshots = {
  login: {
    title: "Login",
    description: "Sign in to access your trading workspace, billing lifecycle, and runtime alerts.",
    submitLabel: "Sign in",
    alternateHref: "/register",
    alternateLabel: "Need an account? Register",
    checklist: ["Verified email", "No withdrawal API permission", "2FA ready"],
    notice: {
      tone: "info",
      title: "Security baseline",
      description: "User and admin flows both support TOTP in V1; admin use is mandatory."
    }
  },
  register: {
    title: "Register",
    description: "Create your account now and continue into verification, membership, and exchange setup.",
    submitLabel: "Create account",
    alternateHref: "/login",
    alternateLabel: "Already registered? Login",
    checklist: ["Email verification required", "One Binance account per user", "Membership required before runtime"],
    notice: {
      tone: "warning",
      title: "Before you bind Binance",
      description: "Do not enable withdrawal permission on your API key. The app will only use trading and read scopes."
    }
  }
} as const;

const userNav: NavItem[] = [
  { href: "/app/dashboard", label: "Dashboard" },
  { href: "/app/exchange", label: "Exchange" },
  { href: "/app/strategies", label: "Strategies", badge: "8" },
  { href: "/app/billing", label: "Billing" },
  { href: "/app/analytics", label: "Analytics" },
  { href: "/app/security", label: "Security" },
  { href: "/app/membership", label: "Membership" },
  { href: "/app/notifications", label: "Notifications", badge: "3" }
];

const adminNav: NavItem[] = [
  { href: "/admin/dashboard", label: "Dashboard" },
  { href: "/admin/users", label: "Users", badge: "12" },
  { href: "/admin/address-pools", label: "Address pools" },
  { href: "/admin/templates", label: "Templates" },
  { href: "/admin/billing", label: "Billing", badge: "4" },
  { href: "/admin/audit", label: "Audit" }
];

export function buildUserShellSnapshot(activeHref: string): UserShellSnapshot {
  return {
    activeHref,
    brand: "GridBinance",
    subtitle: "User operating cockpit",
    title: "Trading workspace shell",
    description: "Shared navigation, membership visibility, and runtime warning surfaces across all user pages.",
    identity: {
      name: "Luna Chen",
      role: "Verified member",
      context: "Plan renews in 13 days. Binance futures account remains in hedge mode."
    },
    nav: userNav,
    quickStats: [
      { label: "Net PnL", value: "+1,284.20 USDT" },
      { label: "Running", value: "5 strategies" },
      { label: "Grace", value: "Inactive" }
    ],
    banners: [
      {
        tone: "warning",
        title: "Renewal window approaching",
        description: "Membership enters a 48-hour grace period after expiry. Existing running strategies may continue only during that window.",
        action: { href: "/app/billing", label: "Open billing" }
      }
    ]
  };
}

export function buildAdminShellSnapshot(activeHref: string): AdminShellSnapshot {
  return {
    activeHref,
    brand: "GridBinance Ops",
    subtitle: "Admin control plane",
    title: "Administration shell",
    description: "Shared operations navigation for pricing, address pools, templates, and audit review.",
    identity: {
      name: "Operator Nova",
      role: "super_admin",
      context: "TOTP is enabled. Four abnormal billing orders require review today."
    },
    nav: adminNav,
    quickStats: [
      { label: "Queued orders", value: "4" },
      { label: "Pool utilization", value: "78%" },
      { label: "Templates", value: "11 active" }
    ],
    banners: [
      {
        tone: "danger",
        title: "Abnormal payment queue",
        description: "Overpayment, wrong-token, and underpayment cases stay blocked until an operator resolves them.",
        action: { href: "/admin/billing", label: "Review queue" }
      }
    ]
  };
}

export const userDashboardSnapshot = {
  banner: {
    tone: "success",
    title: "Shell baseline active",
    description: "Dashboard widgets now live inside the shared user shell instead of route-local bare markup."
  },
  tabs: [
    { href: "/app/dashboard", label: "Overview" },
    { href: "/app/analytics", label: "Analytics" },
    { href: "/app/strategies", label: "Strategies", badge: "8" }
  ],
  metrics: [
    { label: "Wallet balance", value: "18,420 USDT", detail: "Spot + futures + locked billing reserves" },
    { label: "Net profit", value: "+1,284.20 USDT", detail: "Fees and funding included" },
    { label: "Error-paused", value: "1 strategy", detail: "Runtime warning surfaced in inbox" }
  ],
  fills: [
    { id: "fill-1", symbol: "BTCUSDT", side: "Buy", pnl: "+82.10", state: "Settled" },
    { id: "fill-2", symbol: "ETHUSDT", side: "Sell", pnl: "+24.87", state: "Settled" },
    { id: "fill-3", symbol: "SOLUSDT", side: "Buy", pnl: "-6.24", state: "Trailing TP" }
  ],
  notes: [
    "Recent fills include per-fill profit reporting for Telegram notifications.",
    "Membership status remains visible beside runtime metrics.",
    "Exchange trade history and activity views are reserved for subsequent task content."
  ]
};

export const exchangeSnapshot = {
  banner: {
    tone: "info",
    title: "Exchange credential workspace",
    description: "One user can bind only one Binance account, and secrets stay masked after save."
  },
  tabs: [
    { href: "/app/exchange", label: "Credentials" },
    { href: "/app/security", label: "Security" },
    { href: "/app/strategies", label: "Symbols" }
  ],
  metadata: [
    { label: "Symbol sync", value: "Every 1 hour" },
    { label: "Supported scopes", value: "Spot, USDⓈ-M, COIN-M" },
    { label: "Futures mode", value: "Hedge mode required" }
  ]
};

export const strategiesSnapshot = {
  banner: {
    tone: "warning",
    title: "Lifecycle guardrails",
    description: "Strategy edits require pause first, save before restart, and no hot-modify while running."
  },
  tabs: [
    { href: "/app/strategies", label: "All" },
    { href: "/app/strategies/grid-btc", label: "Workspace" },
    { href: "/app/analytics", label: "PnL" }
  ],
  rows: [
    { id: "grid-btc", name: "BTC mean re-entry", market: "Spot", state: "Running", exposure: "5,000 USDT" },
    { id: "grid-eth", name: "ETH short ladder", market: "USDⓈ-M", state: "Draft", exposure: "2,400 USDT" },
    { id: "grid-sol", name: "SOL neutral swing", market: "COIN-M", state: "Paused", exposure: "1,800 USD" }
  ],
  summaries: [
    { label: "Drafts", value: "2" },
    { label: "Running", value: "5" },
    { label: "Paused", value: "1" }
  ]
};

export const strategyDetailSnapshots = {
  "grid-btc": {
    title: "BTC mean re-entry",
    description: "Review pre-check state, grid ladders, trailing take profit, and stop semantics before runtime wiring lands.",
    tabs: [
      { href: "/app/strategies/grid-btc", label: "Workspace" },
      { href: "/app/analytics", label: "Analytics" },
      { href: "/help/expiry-reminder", label: "Help" }
    ],
    stats: [
      { label: "Mode", value: "Classic two-way spot" },
      { label: "Grid count", value: "12 levels" },
      { label: "Cycle policy", value: "Rebuild and continue" }
    ],
    rows: [
      { id: "level-1", level: "L1", range: "86,000 - 86,750", allocation: "0.008 BTC", tp: "1.2%" },
      { id: "level-2", level: "L2", range: "86,750 - 87,500", allocation: "0.007 BTC", tp: "1.1%" },
      { id: "level-3", level: "L3", range: "87,500 - 88,250", allocation: "0.006 BTC", tp: "0.9%" }
    ]
  }
} as const;

export const billingSnapshot = {
  banner: {
    tone: "warning",
    title: "Grace-period reminder enabled",
    description: "After expiry, existing strategies may continue only for 48 hours before auto-pause blocks new starts."
  },
  tabs: [
    { href: "/app/billing", label: "Renewal" },
    { href: "/app/membership", label: "Entitlement" },
    { href: "/help/expiry-reminder", label: "Help" }
  ],
  plans: [
    { label: "Monthly", value: "20 USD eq." },
    { label: "Quarterly", value: "18 USD eq./mo" },
    { label: "Yearly", value: "15 USD eq./mo" }
  ],
  rows: [
    { id: "order-1", order: "ORD-4201", chain: "BSC / USDT", amount: "20.00", state: "Awaiting exact transfer" },
    { id: "order-2", order: "ORD-4138", chain: "Solana / USDC", amount: "60.00", state: "Confirmed" }
  ]
};

export const analyticsSnapshot = {
  banner: {
    tone: "info",
    title: "Reporting surfaces",
    description: "Exports are shared UI primitives here first; full reporting endpoints arrive in later tasks."
  },
  tabs: [
    { href: "/app/analytics", label: "Summary" },
    { href: "/app/strategies", label: "Strategies" },
    { href: "/app/notifications", label: "Alerts" }
  ],
  metrics: [
    { label: "Realized PnL", value: "+1,632.44" },
    { label: "Unrealized PnL", value: "+192.51" },
    { label: "Fees", value: "-231.08" }
  ],
  rows: [
    { id: "export-1", export: "Fill records", cadence: "On demand CSV", scope: "Account + strategy" },
    { id: "export-2", export: "Order records", cadence: "On demand CSV", scope: "User account" },
    { id: "export-3", export: "Strategy statistics", cadence: "On demand CSV", scope: "Per strategy" }
  ]
};

export const securitySnapshot = {
  banner: {
    tone: "info",
    title: "Security center shell",
    description: "Password reset, email verification, and TOTP live behind the shared form system."
  },
  checkpoints: [
    { label: "Email", value: "Verified" },
    { label: "TOTP", value: "Enabled" },
    { label: "Session review", value: "2 active devices" }
  ]
};

export const membershipSnapshot = {
  banner: {
    tone: "success",
    title: "Membership overview",
    description: "Renewal stacking, freeze state, and grace reminders are grouped in one shell-aware view."
  },
  rows: [
    { id: "timeline-1", event: "Current plan", at: "2026-04-15", note: "Monthly plan ends" },
    { id: "timeline-2", event: "Grace period", at: "2026-04-17", note: "Auto-pause begins after this date" },
    { id: "timeline-3", event: "Stacked renewal", at: "Queued", note: "Awaiting exact on-chain payment" }
  ]
};

export const notificationsSnapshot = {
  banner: {
    tone: "warning",
    title: "Notification routing",
    description: "Deposit success, membership reminders, API invalidation, and grid fills stay visible in web and Telegram."
  },
  channels: [
    { label: "Telegram", value: "Bound" },
    { label: "Web inbox", value: "Active" },
    { label: "Critical alerts", value: "Instant" }
  ],
  rows: [
    { id: "notice-1", event: "Membership expiring", channel: "Telegram + web", state: "Queued" },
    { id: "notice-2", event: "Runtime failure", channel: "Telegram + web", state: "Delivered" },
    { id: "notice-3", event: "Deposit confirmed", channel: "Telegram + web", state: "Delivered" }
  ]
};

export const adminDashboardSnapshot = {
  banner: {
    tone: "danger",
    title: "Operator queue requires action",
    description: "Billing exceptions and address pool pressure are surfaced at shell level for every admin route."
  },
  tabs: [
    { href: "/admin/dashboard", label: "Overview" },
    { href: "/admin/billing", label: "Billing", badge: "4" },
    { href: "/admin/audit", label: "Audit" }
  ],
  metrics: [
    { label: "Abnormal orders", value: "4", detail: "Exact-amount mismatch or wrong token" },
    { label: "Pool free slots", value: "11", detail: "Across Ethereum, BSC, Solana" },
    { label: "Pending overrides", value: "2", detail: "Membership extension requests" }
  ],
  rows: [
    { id: "audit-1", actor: "Operator Nova", action: "Extended membership", target: "user_102" },
    { id: "audit-2", actor: "System", action: "Locked BSC address", target: "ORD-4201" },
    { id: "audit-3", actor: "Operator Mira", action: "Marked order abnormal", target: "ORD-4195" }
  ]
};

export const adminUsersSnapshot = {
  banner: {
    tone: "info",
    title: "Membership controls",
    description: "Freeze, unfreeze, open, extend, and revoke actions stay audit-backed and explicit."
  },
  rows: [
    { id: "user-1", email: "luna@example.com", membership: "Active", grace: "No", note: "TOTP enabled" },
    { id: "user-2", email: "miles@example.com", membership: "Grace", grace: "26h left", note: "Awaiting renewal" },
    { id: "user-3", email: "ava@example.com", membership: "Frozen", grace: "N/A", note: "Manual override" }
  ]
};

export const adminAddressPoolsSnapshot = {
  banner: {
    tone: "warning",
    title: "Address rotation",
    description: "Each order gets one address for one hour; overflow enters a queue until a slot frees up."
  },
  rows: [
    { id: "pool-eth", chain: "Ethereum", total: "8", locked: "5", queue: "1" },
    { id: "pool-bsc", chain: "BSC", total: "7", locked: "6", queue: "2" },
    { id: "pool-sol", chain: "Solana", total: "5", locked: "3", queue: "0" }
  ]
};

export const adminTemplatesSnapshot = {
  banner: {
    tone: "info",
    title: "Template library",
    description: "Admin templates are copied into user strategies; later edits do not mutate already-applied drafts."
  },
  rows: [
    { id: "tpl-1", template: "BTC recovery ladder", market: "Spot", usage: "34 copies" },
    { id: "tpl-2", template: "ETH short mean reversion", market: "USDⓈ-M", usage: "17 copies" },
    { id: "tpl-3", template: "SOL neutral range", market: "COIN-M", usage: "8 copies" }
  ]
};

export const adminBillingSnapshot = {
  banner: {
    tone: "danger",
    title: "Abnormal billing triage",
    description: "Overpayment, underpayment, wrong token, and abnormal transfers are held for manual handling."
  },
  rows: [
    { id: "abn-1", order: "ORD-4195", issue: "Wrong token", amount: "20.00", action: "Manual review" },
    { id: "abn-2", order: "ORD-4201", issue: "Underpayment", amount: "19.50", action: "Pending contact" },
    { id: "abn-3", order: "ORD-4204", issue: "Overpayment", amount: "20.75", action: "Treasury hold" }
  ]
};

export const adminAuditSnapshot = {
  banner: {
    tone: "info",
    title: "Audit retention",
    description: "Critical admin actions keep actor, timestamp, target entity, and before/after context."
  },
  rows: [
    { id: "log-1", actor: "Operator Nova", timestamp: "2026-04-02 09:12", action: "Pool size update", target: "BSC" },
    { id: "log-2", actor: "Operator Mira", timestamp: "2026-04-02 08:45", action: "Membership freeze", target: "user_311" },
    { id: "log-3", actor: "System", timestamp: "2026-04-02 08:15", action: "Sweep queued", target: "sol_pool_03" }
  ]
};
