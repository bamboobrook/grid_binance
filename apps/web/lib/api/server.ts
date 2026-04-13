import "server-only";

import { cookies } from "next/headers";

import { describeMembershipStatus } from "../ui/domain-copy";
import { pickText, resolveUiLanguageFromRoute, UI_LANGUAGE_COOKIE, type UiLanguage } from "../ui/preferences";

import { buildAdminShellSnapshot } from "./admin-product-state";
import {
  adminAddressPoolsSnapshot,
  adminAuditSnapshot,
  adminDashboardSnapshot,
  adminDepositsSnapshot,
  adminMembershipsSnapshot,
  adminStrategiesSnapshot,
  adminSweepsSnapshot,
  adminSystemSnapshot,
  adminTemplatesSnapshot,
  adminUsersSnapshot,
  analyticsSnapshot,
  billingSnapshot,
  buildPublicAuthSnapshot,
  buildPublicShellSnapshot,
  buildUserShellSnapshot,
  exchangeSnapshot,
  helpCenterSnapshot,
  homeSnapshot,
  membershipSnapshot,
  notificationsSnapshot,
  ordersSnapshot,
  securitySnapshot,
  strategiesSnapshot,
  strategyComposerSnapshot,
  strategyDetailSnapshots,
  telegramSnapshot,
  type AdminShellSnapshot,
  type PublicShellSnapshot,
  type UserShellSnapshot,
  userDashboardSnapshot,
} from "./mock-data";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

type ProfileResponse = {
  admin_totp_required?: boolean;
  email?: string;
  email_verified?: boolean;
  totp_enabled?: boolean;
};

type BillingOverview = {
  membership?: {
    active_until?: string | null;
    grace_until?: string | null;
    status?: string;
  };
};

type AnalyticsReport = {
  user?: {
    net_pnl?: string;
  };
};

type StrategyListResponse = {
  items: Array<{
    status: string;
  }>;
};

type NotificationRecord = {
  event: {
    kind?: string;
    message: string;
    title: string;
  };
  show_expiry_popup: boolean;
};

type NotificationInboxResponse = {
  items: NotificationRecord[];
};

function clone<T>(value: T): T {
  return structuredClone(value);
}

export async function getPublicShellSnapshot(routeLocale?: string | null): Promise<PublicShellSnapshot> {
  const lang = await currentUiLanguage(routeLocale);
  return clone(buildPublicShellSnapshot(lang));
}

export async function getHomeSnapshot() {
  return clone(homeSnapshot);
}

export async function getPublicAuthSnapshot(mode: "login" | "register", routeLocale?: string | null) {
  const lang = await currentUiLanguage(routeLocale);
  return clone(buildPublicAuthSnapshot(mode, lang));
}

export async function getUserShellSnapshot(routeLocale?: string | null): Promise<UserShellSnapshot> {
  const lang = await currentUiLanguage(routeLocale);
  const sessionToken = await currentSessionToken();
  const base = clone(buildUserShellSnapshot(lang));
  base.banners = [];

  if (!sessionToken) {
    return base;
  }

  const [profile, billing, analytics, strategies] = await Promise.all([
    fetchProfile(sessionToken),
    fetchBillingOverview(sessionToken),
    fetchAnalytics(sessionToken),
    fetchStrategies(sessionToken),
  ]);

  if (profile?.email) {
    base.identity.name = profile.email;
  }

  const membershipStatus = billing?.membership?.status ?? pickText(lang, "待开通", "Pending");
  const membershipLabel = describeMembershipStatus(lang, billing?.membership?.status);
  base.identity.role = membershipLabel;
  if (billing?.membership?.active_until) {
    const renewalDate = billing.membership.active_until.slice(0, 10);
    const graceDate = billing.membership.grace_until?.slice(0, 10);
    base.identity.context = graceDate
      ? pickText(lang, `会员状态 ${membershipLabel}，下次续费 ${renewalDate}，宽限期截止 ${graceDate}。`, `Membership ${membershipLabel}. Next renewal ${renewalDate}. Grace ends ${graceDate}.`)
      : pickText(lang, `会员状态 ${membershipLabel}，下次续费 ${renewalDate}。`, `Membership ${membershipLabel}. Next renewal ${renewalDate}.`);
  } else if (billing?.membership?.grace_until) {
    const graceDate = billing.membership.grace_until.slice(0, 10);
    base.identity.context = pickText(lang, `会员当前处于 ${membershipLabel}，宽限期截止 ${graceDate}。`, `Membership is ${membershipLabel}. Grace ends ${graceDate}.`);
  } else {
    base.identity.context = pickText(lang, "会员尚未激活；先完成会员开通、交易所连接和通知设置。", "Membership is not active yet. Complete billing, exchange setup, and notifications first.");
  }

  const runningCount = strategies.filter((item) => item.status === "Running").length;
  const errorPausedCount = strategies.filter((item) => item.status === "ErrorPaused").length;
  base.quickStats = [
    { label: pickText(lang, "净收益", "Net PnL"), value: analytics?.user?.net_pnl ?? "-" },
    { label: pickText(lang, "运行中", "Running"), value: pickText(lang, `${runningCount} 个策略`, `${runningCount} strategies`) },
    { label: pickText(lang, "会员状态", "Membership Status"), value: membershipLabel },
    { label: pickText(lang, "异常阻塞", "ErrorPaused"), value: String(errorPausedCount) },
  ];

  return base;
}

export async function getUserExpiryNotification(): Promise<NotificationRecord | null> {
  const sessionToken = await currentSessionToken();
  if (!sessionToken) {
    return null;
  }

  const profile = await fetchProfile(sessionToken);
  if (!profile?.email) {
    return null;
  }

  const inbox = await fetchNotifications(sessionToken, profile.email);
  return inbox?.items.find((item) => item.show_expiry_popup) ?? null;
}

export async function getAdminShellSnapshot(routeLocale?: string | null): Promise<AdminShellSnapshot> {
  const lang = await currentUiLanguage(routeLocale);
  try {
    return await buildAdminShellSnapshot(lang);
  } catch {
    return buildFallbackAdminShellSnapshot(lang);
  }
}

export async function getUserDashboardSnapshot() {
  return clone(userDashboardSnapshot);
}

export async function getExchangeSnapshot() {
  return clone(exchangeSnapshot);
}

export async function getStrategiesSnapshot() {
  return clone(strategiesSnapshot);
}

export async function getStrategyComposerSnapshot() {
  return clone(strategyComposerSnapshot);
}

export async function getStrategyDetailSnapshot(id: string) {
  return clone(strategyDetailSnapshots[id as keyof typeof strategyDetailSnapshots] ?? null);
}

export async function getOrdersSnapshot() {
  return clone(ordersSnapshot);
}

export async function getBillingSnapshot() {
  return clone(billingSnapshot);
}

export async function getAnalyticsSnapshot() {
  return clone(analyticsSnapshot);
}

export async function getTelegramSnapshot() {
  return clone(telegramSnapshot);
}

export async function getSecuritySnapshot() {
  return clone(securitySnapshot);
}

export async function getHelpCenterSnapshot() {
  return clone(helpCenterSnapshot);
}

export async function getMembershipSnapshot() {
  return clone(membershipSnapshot);
}

export async function getNotificationsSnapshot() {
  return clone(notificationsSnapshot);
}

export async function getAdminDashboardSnapshot() {
  return clone(adminDashboardSnapshot);
}

export async function getAdminUsersSnapshot() {
  return clone(adminUsersSnapshot);
}

export async function getAdminMembershipsSnapshot() {
  return clone(adminMembershipsSnapshot);
}

export async function getAdminDepositsSnapshot() {
  return clone(adminDepositsSnapshot);
}

export async function getAdminAddressPoolsSnapshot() {
  return clone(adminAddressPoolsSnapshot);
}

export async function getAdminTemplatesSnapshot() {
  return clone(adminTemplatesSnapshot);
}

export async function getAdminStrategiesSnapshot() {
  return clone(adminStrategiesSnapshot);
}

export async function getAdminSweepsSnapshot() {
  return clone(adminSweepsSnapshot);
}

export async function getAdminAuditSnapshot() {
  return clone(adminAuditSnapshot);
}

export async function getAdminSystemSnapshot() {
  return clone(adminSystemSnapshot);
}

async function currentUiLanguage(routeLocale?: string | null): Promise<UiLanguage> {
  const cookieStore = await cookies();
  return resolveUiLanguageFromRoute(routeLocale, cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
}

async function currentSessionToken() {
  const cookieStore = await cookies();
  return cookieStore.get("session_token")?.value ?? null;
}

function buildFallbackAdminShellSnapshot(lang: UiLanguage): AdminShellSnapshot {
  return clone({
    banners: [
      {
        description: pickText(
          lang,
          "后台基础资料暂时不可用，管理工作台已切换到最小回退壳体。",
          "Admin baseline data is temporarily unavailable. The shell switched to a minimal fallback surface.",
        ),
        title: pickText(lang, "管理工作台回退中", "Admin shell fallback"),
        tone: "warning",
      },
    ],
    brand: "GridBinance Ops",
    description: pickText(lang, "后台数据异常时的最小管理壳体。", "Minimal admin shell shown when backend data is unavailable."),
    identity: {
      context: pickText(lang, "等待后台恢复后再刷新页面。", "Refresh after the admin backend recovers."),
      name: pickText(lang, "管理员会话", "Admin session"),
      role: pickText(lang, "回退模式", "Fallback mode"),
    },
    nav: [
      { href: "/admin/dashboard", label: pickText(lang, "总览", "Dashboard") },
      { href: "/admin/users", label: pickText(lang, "用户", "Users") },
      { href: "/admin/memberships", label: pickText(lang, "会员", "Memberships") },
      { href: "/admin/deposits", label: pickText(lang, "充值单", "Deposits") },
      { href: "/admin/system", label: pickText(lang, "系统", "System") },
    ],
    quickStats: [
      { label: pickText(lang, "待处理充值", "Open deposits"), value: "0" },
      { label: pickText(lang, "会员风险", "Membership risk"), value: "0" },
      { label: pickText(lang, "数据状态", "Data state"), value: pickText(lang, "回退中", "Fallback") },
    ],
    subtitle: pickText(lang, "管理员控制台", "Admin control plane"),
    title: pickText(lang, "管理工作台", "Administration shell"),
  });
}

async function fetchProfile(sessionToken: string): Promise<ProfileResponse | null> {
  const response = await fetch(`${authApiBaseUrl()}/profile`, {
    method: "GET",
    headers: { authorization: `Bearer ${sessionToken}` },
    cache: "no-store",
  });
  if (!response.ok) {
    return null;
  }
  return (await response.json()) as ProfileResponse;
}

async function fetchBillingOverview(sessionToken: string): Promise<BillingOverview | null> {
  const response = await fetch(`${authApiBaseUrl()}/billing/overview`, {
    method: "GET",
    headers: { authorization: `Bearer ${sessionToken}` },
    cache: "no-store",
  });
  if (!response.ok) {
    return null;
  }
  return (await response.json()) as BillingOverview;
}

async function fetchAnalytics(sessionToken: string): Promise<AnalyticsReport | null> {
  const response = await fetch(`${authApiBaseUrl()}/analytics`, {
    method: "GET",
    headers: { authorization: `Bearer ${sessionToken}` },
    cache: "no-store",
  });
  if (!response.ok) {
    return null;
  }
  return (await response.json()) as AnalyticsReport;
}

async function fetchStrategies(sessionToken: string): Promise<StrategyListResponse["items"]> {
  const response = await fetch(`${authApiBaseUrl()}/strategies`, {
    method: "GET",
    headers: { authorization: `Bearer ${sessionToken}` },
    cache: "no-store",
  });
  if (!response.ok) {
    return [];
  }
  return ((await response.json()) as StrategyListResponse).items;
}

async function fetchNotifications(
  sessionToken: string,
  email: string,
): Promise<NotificationInboxResponse | null> {
  const response = await fetch(`${authApiBaseUrl()}/notifications?email=${encodeURIComponent(email)}`, {
    method: "GET",
    headers: { authorization: `Bearer ${sessionToken}` },
    cache: "no-store",
  });
  if (!response.ok) {
    return null;
  }
  return (await response.json()) as NotificationInboxResponse;
}

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}
