import { pickText, type UiLanguage } from "../ui/preferences";

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

export function buildPublicShellSnapshot(lang: UiLanguage): PublicShellSnapshot {
  return {
    brand: "GridBinance",
    subtitle: pickText(lang, "币安网格 SaaS 控制台", "Binance grid SaaS control plane"),
    eyebrow: pickText(lang, "首版商用交付", "Commercial recovery plan"),
    title: pickText(lang, "公共入口", "Public access shell"),
    description: pickText(lang, "首页、登录、注册共用同一套公共外壳。", "Shared entry and preview shell for the homepage, login, and registration flows."),
    actions: [
      { href: "/", label: pickText(lang, "首页", "Home") },
      { href: "/login", label: pickText(lang, "登录", "Login") },
      { href: "/register", label: pickText(lang, "注册", "Register") },
    ],
    highlights: [
      {
        title: pickText(lang, "先开会员", "Membership first"),
        description: pickText(lang, "策略运行受有效会员权限控制，宽限期风险会明确展示。", "Running strategies is gated by an active membership with a visible grace-period surface."),
      },
      {
        title: pickText(lang, "关键风险直说", "Explicit warnings"),
        description: pickText(lang, "计费、API 权限、止盈模式等关键提醒会直接展示，不藏在自动化后面。", "Billing, API permissions, and take-profit modes stay visible instead of being hidden behind automation."),
      },
    ],
    supportLinks: [
      { href: "/app/help", label: pickText(lang, "帮助中心", "In-app help center") },
      { href: "/app/dashboard", label: pickText(lang, "用户控制台预览", "User dashboard preview") },
      { href: "/admin/dashboard", label: pickText(lang, "管理员控制台预览", "Admin dashboard preview") },
    ],
  };
}

export function buildPublicAuthSnapshot(mode: "login" | "register", lang: UiLanguage) {
  if (mode === "login") {
    return {
      title: pickText(lang, "登录", "Login"),
      description: pickText(lang, "登录后进入你的交易控制台、会员计费和运行提醒中心。", "Sign in to access your trading workspace, billing lifecycle, and runtime alerts."),
      submitLabel: pickText(lang, "登录", "Sign in"),
      alternateHref: "/register",
      alternateLabel: pickText(lang, "还没有账号？去注册", "Need an account? Register"),
      checklist: [
        pickText(lang, "邮箱已验证", "Verified email"),
        pickText(lang, "API 不可开启提现权限", "No withdrawal API permission"),
        pickText(lang, "已准备好 2FA", "2FA ready"),
      ],
      notice: {
        tone: "info",
        title: pickText(lang, "安全基线", "Security baseline"),
        description: pickText(lang, "用户和管理员都支持 TOTP，管理员必须启用。", "User and admin flows both support TOTP in V1; admin use is mandatory."),
      },
    };
  }

  return {
    title: pickText(lang, "注册", "Register"),
    description: pickText(lang, "创建账号后可直接登录，再继续会员开通和交易所接入。", "Create your account, sign in immediately, then continue with membership and exchange setup."),
    submitLabel: pickText(lang, "创建账号", "Create account"),
    alternateHref: "/login",
    alternateLabel: pickText(lang, "已经注册？去登录", "Already registered? Login"),
    checklist: [
      pickText(lang, "注册后可直接登录", "Direct sign-in after registration"),
      pickText(lang, "每个用户仅绑定一个币安账号", "One Binance account per user"),
      pickText(lang, "运行策略前必须开通会员", "Membership required before runtime"),
    ],
    notice: {
      tone: "warning",
      title: pickText(lang, "绑定币安前请确认", "Before you bind Binance"),
      description: pickText(lang, "不要给 API 开启提现权限，平台只会使用读取与交易权限。", "Do not enable withdrawal permission on your API key. The app will only use trading and read scopes."),
    },
  };
}

function buildUserNav(lang: UiLanguage): NavItem[] {
  return [
    { href: "/app/dashboard", label: pickText(lang, "总览", "Dashboard") },
    { href: "/app/exchange", label: pickText(lang, "交易所", "Exchange") },
    { href: "/app/notifications", label: pickText(lang, "通知", "Notifications") },
    { href: "/app/strategies", label: pickText(lang, "策略", "Strategies") },
    { href: "/app/backtest", label: pickText(lang, "回测", "Backtest") },
    { href: "/app/martingale-portfolios", label: pickText(lang, "马丁组合", "Martingale Portfolios") },
    { href: "/app/orders", label: pickText(lang, "订单", "Orders") },
    { href: "/app/analytics", label: pickText(lang, "统计", "Analytics") },
    { href: "/app/billing", label: pickText(lang, "会员中心", "Membership Center") },
    { href: "/app/telegram", label: pickText(lang, "Telegram", "Telegram") },
    { href: "/app/security", label: pickText(lang, "安全", "Security") },
    { href: "/app/help", label: pickText(lang, "帮助", "Help") },
  ];
}

const adminNav: NavItem[] = [
  { href: "/admin/dashboard", label: "Dashboard" },
  { href: "/admin/users", label: "Users", badge: "12" },
  { href: "/admin/memberships", label: "Memberships" },
  { href: "/admin/deposits", label: "Deposits", badge: "4" },
  { href: "/admin/address-pools", label: "Address pools" },
  { href: "/admin/templates", label: "Templates" },
  { href: "/admin/strategies", label: "Strategies" },
  { href: "/admin/sweeps", label: "Sweeps" },
  { href: "/admin/audit", label: "Audit" },
  { href: "/admin/system", label: "System" }
];

export function buildUserShellSnapshot(lang: UiLanguage): UserShellSnapshot {
  return {
    brand: "GridBinance",
    subtitle: pickText(lang, "用户操作台", "User operating cockpit"),
    title: pickText(lang, "交易工作台", "Trading workspace shell"),
    description: pickText(lang, "所有用户页面共享导航、会员可见性和运行告警入口。", "Shared navigation, membership visibility, and runtime warning surfaces across all user pages."),
    identity: {
      name: pickText(lang, "账户会话", "Account session"),
      role: pickText(lang, "待同步", "Pending sync"),
      context: pickText(lang, "会员、交易所和通知状态会在登录后按真实数据加载。", "Membership, exchange, and notification status load from live account data after sign-in."),
    },
    nav: buildUserNav(lang),
    quickStats: [
      { label: pickText(lang, "净收益", "Net PnL"), value: "-" },
      { label: pickText(lang, "运行中", "Running"), value: pickText(lang, "等待加载", "Loading") },
      { label: pickText(lang, "会员状态", "Membership Status"), value: pickText(lang, "待同步", "Pending sync") },
    ],
    banners: [],
  };
}

export function buildAdminShellSnapshot(): AdminShellSnapshot {
  return {
    brand: "GridBinance Ops",
    subtitle: "Admin control plane",
    title: "Administration shell",
    description: "Shared operations navigation for pricing, address pools, deposits, and audit review.",
    identity: {
      name: "Operator Nova",
      role: "super_admin",
      context: "TOTP is enabled. Four abnormal deposit orders require review today."
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
        action: { href: "/admin/deposits", label: "Review queue" }
      }
    ]
  };
}

export const homeSnapshot = {
  banner: {
    tone: "info",
    title: "Shared public shell active",
    description: "Homepage, login, and registration now share the same public shell rather than route-local bare markup."
  },
  links: [
    { href: "/register", label: "Registration entry" },
    { href: "/app/dashboard", label: "Open user dashboard" },
    { href: "/app/help", label: "Help center" }
  ]
};

export const userDashboardSnapshot = {
  banner: {
    tone: "success",
    title: "Shell baseline active",
    description: "Dashboard widgets now live inside the shared user shell instead of route-local bare markup."
  },
  tabs: [
    { href: "/app/dashboard", label: "Overview" },
    { href: "/app/orders", label: "Orders" },
    { href: "/app/strategies", label: "Strategies" }
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
    { href: "/app/strategies/new", label: "New strategy" },
    { href: "/app/orders", label: "Orders" }
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

export const strategyComposerSnapshot = {
  banner: {
    tone: "info",
    title: "Draft composer shell",
    description: "Task 7 only establishes the shared shell and form surface for strategy creation."
  },
  modes: [
    { label: "Spot", value: "Classic / buy-only / sell-only" },
    { label: "Futures", value: "Long / short / neutral" },
    { label: "Generation", value: "Arithmetic / geometric / custom" }
  ]
};

export const strategyDetailSnapshots = {
  "grid-btc": {
    title: "BTC mean re-entry",
    description: "Review pre-check state, grid ladders, trailing take profit, and stop semantics before runtime wiring lands.",
    tabs: [
      { href: "/app/strategies/grid-btc", label: "Workspace" },
      { href: "/app/orders", label: "Orders" },
      { href: "/app/help", label: "Help" }
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

export const ordersSnapshot = {
  banner: {
    tone: "info",
    title: "Order history shell",
    description: "User order tables and export surfaces now map to the documented /app/orders route."
  },
  rows: [
    { id: "ord-1", order: "ORD-8801", symbol: "BTCUSDT", side: "Buy", state: "Filled" },
    { id: "ord-2", order: "ORD-8802", symbol: "ETHUSDT", side: "Sell", state: "Working" },
    { id: "ord-3", order: "ORD-8803", symbol: "SOLUSDT", side: "Buy", state: "Cancelled" }
  ]
};

export const billingSnapshot = {
  banner: {
    tone: "warning",
    title: "Grace-period reminder enabled",
    description: "After expiry, existing strategies may continue only for 48 hours before auto-pause blocks new starts."
  },
  tabs: [
    { href: "/app/billing", label: "Renewal" },
    { href: "/app/help", label: "Help" },
    { href: "/app/telegram", label: "Telegram" }
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
    title: "Legacy analytics surface",
    description: "This route remains as a non-shell-critical legacy page during route-map alignment."
  },
  tabs: [
    { href: "/app/orders", label: "Orders" },
    { href: "/app/strategies", label: "Strategies" },
    { href: "/app/telegram", label: "Alerts" }
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

export const telegramSnapshot = {
  banner: {
    tone: "warning",
    title: "Telegram notification routing",
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

export const helpCenterSnapshot = {
  banner: {
    tone: "success",
    title: "In-app help center",
    description: "The documented user help route now exists inside the shared user shell."
  },
  guides: [
    { href: "/app/help?article=expiry-reminder", label: "Expiry reminder guide" },
    { href: "/app/billing", label: "Billing center" },
    { href: "/app/security", label: "Security center" }
  ]
};

export const membershipSnapshot = {
  banner: {
    tone: "success",
    title: "Membership overview",
    description: "Legacy membership route retained while the shell route map shifts to documented help and billing pages."
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
    title: "Legacy notification routing",
    description: "Legacy route retained while Telegram becomes the documented page-map destination."
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
    description: "Deposit exceptions and address pool pressure are surfaced at shell level for every admin route."
  },
  tabs: [
    { href: "/admin/dashboard", label: "Overview" },
    { href: "/admin/deposits", label: "Deposits", badge: "4" },
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

export const adminMembershipsSnapshot = {
  banner: {
    tone: "info",
    title: "Membership operations",
    description: "Open, extend, freeze, unfreeze, and revoke controls map to the documented /admin/memberships route."
  },
  rows: [
    { id: "member-1", user: "luna@example.com", plan: "Monthly", state: "Active", action: "Extend" },
    { id: "member-2", user: "miles@example.com", plan: "Quarterly", state: "Grace", action: "Open renewal" }
  ]
};

export const adminDepositsSnapshot = {
  banner: {
    tone: "danger",
    title: "Abnormal deposit triage",
    description: "Overpayment, underpayment, wrong token, and abnormal transfers are held for manual handling."
  },
  rows: [
    { id: "abn-1", order: "ORD-4195", issue: "Wrong token", amount: "20.00", action: "Manual review" },
    { id: "abn-2", order: "ORD-4201", issue: "Underpayment", amount: "19.50", action: "Pending contact" },
    { id: "abn-3", order: "ORD-4204", issue: "Overpayment", amount: "20.75", action: "Treasury hold" }
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

export const adminStrategiesSnapshot = {
  banner: {
    tone: "warning",
    title: "Strategy operations overview",
    description: "Runtime strategy supervision now maps to the documented /admin/strategies route."
  },
  rows: [
    { id: "adm-strat-1", user: "luna@example.com", strategy: "BTC mean re-entry", state: "Running" },
    { id: "adm-strat-2", user: "miles@example.com", strategy: "ETH short ladder", state: "Paused" }
  ]
};

export const adminSweepsSnapshot = {
  banner: {
    tone: "warning",
    title: "Treasury sweep queue",
    description: "Wallet sweep actions are a documented admin route and must remain audited."
  },
  rows: [
    { id: "sweep-pending", wallet: "address-pool-job", amount: "Pending backend job", state: "Pending" }
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

export const adminSystemSnapshot = {
  banner: {
    tone: "info",
    title: "System controls",
    description: "Confirmation counts, pricing config, and operator-facing system toggles map to /admin/system."
  },
  rows: [
    { id: "cfg-1", key: "eth_confirmations", value: "12", scope: "Billing" },
    { id: "cfg-2", key: "symbol_sync_interval", value: "1 hour", scope: "Exchange" },
    { id: "cfg-3", key: "membership_grace_window", value: "48 hours", scope: "Entitlement" }
  ]
};
