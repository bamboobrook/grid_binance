import "server-only";

import { cookies } from "next/headers";

import type { AdminShellSnapshot } from "./mock-data";
import { pickText, type UiLanguage } from "../ui/preferences";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

export const SUPPORTED_PAYMENT_CHAINS = ["ETH", "BSC", "SOL"] as const;
export const SUPPORTED_PAYMENT_ASSETS = ["USDT", "USDC"] as const;

export type AdminRole = "super_admin" | "operator_admin";

export type AdminPermissions = {
  can_manage_address_pools: boolean;
  can_manage_memberships: boolean;
  can_manage_plans: boolean;
  can_manage_sweeps: boolean;
  can_manage_system: boolean;
  can_manage_templates: boolean;
};

export type AdminProfile = {
  admin_access_granted: boolean;
  admin_permissions: AdminPermissions | null;
  admin_role: AdminRole | null;
  admin_totp_required: boolean;
  email: string;
  totp_enabled: boolean;
};

export type AdminMembershipRecord = {
  active_until: string | null;
  email: string;
  grace_until: string | null;
  override_status: string | null;
  status: string;
};

export type AdminMembershipList = {
  items: AdminMembershipRecord[];
};

export type AdminMembershipPlan = {
  code: string;
  duration_days: number;
  is_active: boolean;
  name: string;
  prices: Array<{ amount: string; asset: string; chain: string }>;
};

export type AdminMembershipPlans = {
  plans: AdminMembershipPlan[];
};

export type AdminUserRecord = {
  admin_role: string | null;
  email: string;
  email_verified: boolean;
  latest_order_status: string | null;
  membership: AdminMembershipRecord | null;
  registered: boolean;
  totp_enabled: boolean;
};

export type AdminUserList = {
  items: AdminUserRecord[];
};

export type AdminDepositView = {
  address: string;
  amount: string;
  asset: string;
  chain: string;
  matched_order_id: number | null;
  order_id: number | null;
  review_reason: string | null;
  status: string;
  tx_hash: string;
};

export type AdminDepositsResponse = {
  abnormal_deposits: AdminDepositView[];
  orders: Array<{
    address: string | null;
    amount: string;
    asset: string;
    chain: string;
    email: string;
    order_id: number;
    queue_position: number | null;
    status: string;
  }>;
};

export type AdminAddressPoolsResponse = {
  addresses: Array<{
    address: string;
    chain: string;
    is_enabled: boolean;
  }>;
};

export type AdminTemplateList = {
  items: Array<{
    balance_ready: boolean;
    budget: string;
    conflict_ready: boolean;
    exchange_ready: boolean;
    filters_ready: boolean;
    generation: string;
    amount_mode?: "Quote" | "Base";
    futures_margin_mode?: "Isolated" | "Cross" | null;
    leverage?: number | null;
    grid_spacing_bps: number;
    hedge_mode_ready: boolean;
    id: string;
    levels: Array<{
      entry_price: string;
      quantity: string;
      take_profit_bps: number;
      trailing_bps: number | null;
    }>;
    margin_ready: boolean;
    market: string;
    membership_ready: boolean;
    mode: string;
    name: string;
    overall_stop_loss_bps: number | null;
    overall_take_profit_bps: number | null;
    permissions_ready: boolean;
    post_trigger_action: string;
    symbol: string;
    symbol_ready: boolean;
    withdrawals_disabled: boolean;
  }>;
};

export type AdminStrategyList = {
  items: Array<{
    active_revision: {
      revision_id: string;
      version: number;
    } | null;
    draft_revision: {
      revision_id: string;
      version: number;
    };
    id: string;
    market: string;
    owner_email: string;
    mode: string;
    name: string;
    runtime: {
      events: Array<{ created_at: string; detail: string; event_type: string }>;
      fills: Array<{ fee_amount: string | null; fill_id: string; fill_type: string; realized_pnl: string | null }>;
      last_preflight: { ok: boolean; steps: Array<{ status: string; step: string }> } | null;
      orders: Array<{ order_id: string; side: string; status: string }>;
      positions: Array<{ average_entry_price: string; quantity: string }>;
    };
    status: string;
    symbol: string;
  }>;
};

export type AdminSweepList = {
  jobs: Array<{
    asset: string;
    chain: string;
    requested_by: string;
    status: string;
    sweep_job_id: number;
    transfer_count: number;
    treasury_address: string;
    submitted_at: string | null;
    completed_at: string | null;
    failed_at: string | null;
    last_error: string | null;
    transfers: Array<{
      amount: string;
      from_address: string;
      to_address: string;
      tx_hash: string | null;
      status: string;
      submitted_at: string | null;
      confirmed_at: string | null;
      failed_at: string | null;
      error_message: string | null;
    }>;
  }>;
};

export type AdminAuditList = {
  items: Array<{
    action: string;
    actor_email: string;
    created_at: string;
    payload: Record<string, unknown>;
    target_id: string;
    target_type: string;
  }>;
};

export type AdminSystemConfig = {
  bsc_confirmations: number;
  eth_confirmations: number;
  sol_confirmations: number;
};

const FALLBACK_ADMIN_PROFILE: AdminProfile = {
  admin_access_granted: false,
  admin_permissions: null,
  admin_role: null,
  admin_totp_required: false,
  email: "admin-session@fallback.local",
  totp_enabled: false,
};

const FALLBACK_ADMIN_USERS: AdminUserList = { items: [] };
const FALLBACK_ADMIN_MEMBERSHIPS: AdminMembershipList = { items: [] };
const FALLBACK_ADMIN_MEMBERSHIP_PLANS: AdminMembershipPlans = { plans: [] };
const FALLBACK_ADMIN_DEPOSITS: AdminDepositsResponse = { abnormal_deposits: [], orders: [] };
const FALLBACK_ADMIN_ADDRESS_POOLS: AdminAddressPoolsResponse = { addresses: [] };
const FALLBACK_ADMIN_TEMPLATES: AdminTemplateList = { items: [] };
const FALLBACK_ADMIN_STRATEGIES: AdminStrategyList = { items: [] };
const FALLBACK_ADMIN_SWEEPS: AdminSweepList = { jobs: [] };
const FALLBACK_ADMIN_AUDIT: AdminAuditList = { items: [] };
const FALLBACK_ADMIN_SYSTEM: AdminSystemConfig = {
  bsc_confirmations: 0,
  eth_confirmations: 0,
  sol_confirmations: 0,
};

export async function getCurrentAdminProfile() {
  return fetchAdminJson<AdminProfile>("/profile");
}

export async function getAdminUsersData() {
  return fetchAdminJson<AdminUserList>("/admin/users");
}

export async function getAdminMembershipsData() {
  return fetchAdminJson<AdminMembershipList>("/admin/memberships");
}

export async function getAdminMembershipPlansData() {
  return fetchAdminJson<AdminMembershipPlans>("/admin/memberships/plans");
}

export async function getAdminDepositsData() {
  return fetchAdminJson<AdminDepositsResponse>(`/admin/deposits?at=${encodeURIComponent(new Date().toISOString())}`);
}

export async function getAdminAddressPoolsData() {
  return fetchAdminJson<AdminAddressPoolsResponse>("/admin/address-pools");
}

export async function getAdminTemplatesData() {
  return fetchAdminJson<AdminTemplateList>("/admin/templates");
}

export async function getAdminStrategiesData() {
  return fetchAdminJson<AdminStrategyList>("/admin/strategies");
}

export async function getAdminSweepsData() {
  return fetchAdminJson<AdminSweepList>("/admin/sweeps");
}

export async function getAdminAuditData() {
  return fetchAdminJson<AdminAuditList>("/admin/audit");
}

export async function getAdminSystemData() {
  return fetchAdminJson<AdminSystemConfig>("/admin/system");
}

function describeAdminRole(lang: UiLanguage, role: AdminRole | null) {
  switch (role) {
    case "super_admin":
      return pickText(lang, "超级管理员", "Super Admin");
    case "operator_admin":
      return pickText(lang, "操作员", "Operator Admin");
    default:
      return pickText(lang, "待验证", "Pending Verification");
  }
}

export async function buildAdminShellSnapshot(lang: UiLanguage): Promise<AdminShellSnapshot> {
  const profile = (await tryFetchAdminJson<AdminProfile>("/profile")) ?? {
    admin_access_granted: false,
    admin_permissions: null,
    admin_role: null,
    admin_totp_required: false,
    email: pickText(lang, "管理员会话", "Admin session"),
    totp_enabled: false,
  };
  const canManageTemplates = profile.admin_permissions?.can_manage_templates === true;
  const [membershipsResult, depositsResult, templatesResult] = await Promise.all([
    tryFetchAdminJson<AdminMembershipList>("/admin/memberships"),
    tryFetchAdminJson<AdminDepositsResponse>(`/admin/deposits?at=${encodeURIComponent(new Date().toISOString())}`),
    canManageTemplates ? tryFetchAdminJson<AdminTemplateList>("/admin/templates") : Promise.resolve<AdminTemplateList | null>(null),
  ]);
  const memberships = membershipsResult ?? { items: [] };
  const deposits = depositsResult ?? { abnormal_deposits: [], orders: [] };
  const templates = templatesResult ?? { items: [] };
  const shellDegraded =
    membershipsResult === null ||
    depositsResult === null ||
    (canManageTemplates && templatesResult === null);
  const openDeposits = deposits.abnormal_deposits.filter((item) => item.status === "manual_review_required").length;
  const membershipsNeedingAction = memberships.items.filter((item) => ["Grace", "Frozen", "Revoked"].includes(item.status)).length;
  const role = profile.admin_access_granted ? profile.admin_role : null;
  const restricted = role !== "super_admin";
  const roleLabel = describeAdminRole(lang, role);

  const nav = [
    { href: "/admin/dashboard", label: pickText(lang, "总览", "Dashboard") },
    { href: "/admin/users", label: pickText(lang, "用户", "Users") },
    { href: "/admin/memberships", label: pickText(lang, "会员", "Memberships") },
    { href: "/admin/deposits", label: pickText(lang, "充值单", "Deposits"), badge: String(openDeposits) },
    { href: "/admin/address-pools", label: pickText(lang, "地址池", "Address pools") },
    ...(profile.admin_permissions?.can_manage_templates ? [{ href: "/admin/templates", label: pickText(lang, "模板", "Templates") }] : []),
    { href: "/admin/strategies", label: pickText(lang, "策略", "Strategies") },
    { href: "/admin/sweeps", label: pickText(lang, "归集", "Sweeps") },
    ...(role === "super_admin" ? [{ href: "/admin/audit", label: pickText(lang, "审计", "Audit") }] : []),
    { href: "/admin/system", label: pickText(lang, "系统", "System") },
  ];

  const banners: AdminShellSnapshot["banners"] = [
    {
      action: { href: "/admin/deposits", label: openDeposits > 0 ? pickText(lang, "处理队列", "Review queue") : pickText(lang, "查看充值", "View deposits") },
      description: !profile.admin_access_granted
        ? pickText(lang, "管理员身份已识别，但当前 bearer 会话尚未通过 TOTP 门禁。", "Admin identity is recognized, but this bearer session has not cleared the TOTP gate yet.")
        : restricted
          ? pickText(lang, "当前为操作员权限边界，价格、模板、归集和系统变更需要超级管理员会话。", "Operator boundary is active. Pricing, templates, sweeps, and system changes require a Super Admin session.")
          : pickText(lang, "当前为超级管理员会话，可执行定价、金库和模板操作。", "Super admin session is active for pricing, treasury, and template operations."),
      title: profile.admin_access_granted ? pickText(lang, "管理员权限已生效", "Admin access granted") : pickText(lang, "管理员权限未生效", "Admin access missing"),
      tone: profile.admin_access_granted ? "success" : "warning",
    },
  ];

  if (shellDegraded) {
    banners.push({
      description: pickText(
        lang,
        "部分 supporting endpoint 请求失败，工作台已降级为回退数据继续渲染；请稍后刷新或检查后台服务。",
        "One or more supporting endpoints failed. The shell stayed up in a degraded fallback mode; refresh later or inspect the backend services.",
      ),
      title: pickText(lang, "管理工作台已降级", "Admin shell degraded"),
      tone: "warning",
    });
  }

  return {
    banners,
    brand: "GridBinance Ops",
    description: pickText(lang, "基于后端真实数据的管理控制台。", "Backend-backed admin control plane."),
    identity: {
      context: profile.admin_access_granted
        ? pickText(lang, `TOTP ${profile.totp_enabled ? "已启用" : "未启用"}，操作员边界${restricted ? "生效中" : "已解除"}。`, `TOTP ${profile.totp_enabled ? "enabled" : "disabled"}. Operator boundary ${restricted ? "active" : "lifted"}.`)
        : pickText(lang, `TOTP ${profile.totp_enabled ? "已启用" : "未启用"}，管理员权限仍待新的 bearer 会话验证。`, `TOTP ${profile.totp_enabled ? "enabled" : "disabled"}. Admin access is pending fresh bearer-session verification.`),
      name: profile.email,
      role: roleLabel,
    },
    nav,
    quickStats: [
      { label: pickText(lang, "待处理充值", "Open deposits"), value: String(openDeposits) },
      { label: pickText(lang, "会员风险", "Membership risk"), value: String(membershipsNeedingAction) },
      ...(canManageTemplates
        ? [{ label: pickText(lang, "模板", "Templates"), value: String(templates.items.length) }]
        : []),
    ],
    subtitle: pickText(lang, "管理员控制台", "Admin control plane"),
    title: pickText(lang, "管理工作台", "Administration shell"),
  };
}

export async function fetchAdminJson<T>(path: string, init?: RequestInit): Promise<T> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  try {
    return await fetchAdminJsonWithToken<T>(sessionToken, path, init);
  } catch {
    return fallbackAdminData(path) as T;
  }
}

export async function fetchAdminJsonWithToken<T>(sessionToken: string, path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(`${authApiBaseUrl()}${path}`, {
    ...init,
    headers: {
      authorization: `Bearer ${sessionToken}`,
      ...(init?.headers ?? {}),
    },
    cache: "no-store",
  });

  if (!response.ok) {
    throw new Error(`admin backend request failed ${response.status} ${path}`);
  }

  return (await response.json()) as T;
}

async function tryFetchAdminJson<T>(path: string, init?: RequestInit): Promise<T | null> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  try {
    return await fetchAdminJsonWithToken<T>(sessionToken, path, init);
  } catch {
    return null;
  }
}

function fallbackAdminData(path: string) {
  switch (normalizeAdminPath(path)) {
    case "/profile":
      return FALLBACK_ADMIN_PROFILE;
    case "/admin/users":
      return FALLBACK_ADMIN_USERS;
    case "/admin/memberships":
      return FALLBACK_ADMIN_MEMBERSHIPS;
    case "/admin/memberships/plans":
      return FALLBACK_ADMIN_MEMBERSHIP_PLANS;
    case "/admin/deposits":
      return FALLBACK_ADMIN_DEPOSITS;
    case "/admin/address-pools":
      return FALLBACK_ADMIN_ADDRESS_POOLS;
    case "/admin/templates":
      return FALLBACK_ADMIN_TEMPLATES;
    case "/admin/strategies":
      return FALLBACK_ADMIN_STRATEGIES;
    case "/admin/sweeps":
      return FALLBACK_ADMIN_SWEEPS;
    case "/admin/audit":
      return FALLBACK_ADMIN_AUDIT;
    case "/admin/system":
      return FALLBACK_ADMIN_SYSTEM;
    default:
      throw new Error(`admin fallback missing for ${path}`);
  }
}

function normalizeAdminPath(path: string) {
  const [basePath] = path.split("?", 1);
  return basePath;
}

export function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}
