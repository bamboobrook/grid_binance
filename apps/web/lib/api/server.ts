import "server-only";

import { cookies } from "next/headers";

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

  const membershipStatus = billing?.membership?.status ?? pickText(lang, "未知", "Unknown");
  base.identity.role = membershipStatus;
  if (billing?.membership?.active_until) {
    const renewalDate = billing.membership.active_until.slice(0, 10);
    const graceDate = billing.membership.grace_until?.slice(0, 10);
    base.identity.context = graceDate
      ? pickText(lang, `会员状态 ${membershipStatus}，下次续费 ${renewalDate}，宽限期截止 ${graceDate}。`, `Membership ${membershipStatus}. Next renewal ${renewalDate}. Grace ends ${graceDate}.`)
      : pickText(lang, `会员状态 ${membershipStatus}，下次续费 ${renewalDate}。`, `Membership ${membershipStatus}. Next renewal ${renewalDate}.`);
  }

  const runningCount = strategies.filter((item) => item.status === "Running").length;
  base.quickStats = [
    { label: pickText(lang, "净收益", "Net PnL"), value: analytics?.user?.net_pnl ?? "-" },
    { label: pickText(lang, "运行中", "Running"), value: pickText(lang, `${runningCount} 个策略`, `${runningCount} strategies`) },
    { label: pickText(lang, "会员状态", "Grace"), value: membershipStatus },
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
  return buildAdminShellSnapshot(lang);
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
