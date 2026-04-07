import "server-only";

import { cookies } from "next/headers";

import type { AdminShellSnapshot } from "./mock-data";

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

export async function buildAdminShellSnapshot(): Promise<AdminShellSnapshot> {
  const profile = await getCurrentAdminProfile();
  const [memberships, deposits, templates] = await Promise.all([
    getAdminMembershipsData(),
    getAdminDepositsData(),
    profile.admin_permissions?.can_manage_templates ? getAdminTemplatesData() : Promise.resolve<AdminTemplateList | null>(null),
  ]);
  const openDeposits = deposits.abnormal_deposits.filter((item) => item.status === "manual_review_required").length;
  const membershipsNeedingAction = memberships.items.filter((item) => ["Grace", "Frozen", "Revoked"].includes(item.status)).length;
  const role = profile.admin_access_granted ? profile.admin_role : null;
  const restricted = role !== "super_admin";
  const roleLabel = role ?? "admin_access_pending";

  const nav = [
    { href: "/admin/dashboard", label: "Dashboard" },
    { href: "/admin/users", label: "Users" },
    { href: "/admin/memberships", label: "Memberships" },
    { href: "/admin/deposits", label: "Deposits", badge: String(openDeposits) },
    { href: "/admin/address-pools", label: "Address pools" },
    ...(profile.admin_permissions?.can_manage_templates ? [{ href: "/admin/templates", label: "Templates" }] : []),
    { href: "/admin/strategies", label: "Strategies" },
    { href: "/admin/sweeps", label: "Sweeps" },
    ...(role === "super_admin" ? [{ href: "/admin/audit", label: "Audit" }] : []),
    { href: "/admin/system", label: "System" },
  ];

  return {
    banners: [
      {
        action: { href: "/admin/deposits", label: openDeposits > 0 ? "Review queue" : "View deposits" },
        description: !profile.admin_access_granted
          ? "Admin identity is recognized, but this bearer session has not cleared the TOTP gate yet."
          : restricted
            ? "Operator boundary is active. Pricing, templates, sweeps, and system changes require super_admin."
            : "Super admin session is active for pricing, treasury, and template operations.",
        title: profile.admin_access_granted ? "Admin access granted" : "Admin access missing",
        tone: profile.admin_access_granted ? "success" : "warning",
      },
    ],
    brand: "GridBinance Ops",
    description: "Backend-backed admin control plane.",
    identity: {
      context: profile.admin_access_granted
        ? `TOTP ${profile.totp_enabled ? "enabled" : "disabled"}. Operator boundary ${restricted ? "active" : "lifted"}.`
        : `TOTP ${profile.totp_enabled ? "enabled" : "disabled"}. Admin access is pending fresh bearer-session verification.`,
      name: profile.email,
      role: roleLabel,
    },
    nav,
    quickStats: [
      { label: "Open deposits", value: String(openDeposits) },
      { label: "Membership risk", value: String(membershipsNeedingAction) },
      ...(profile.admin_permissions?.can_manage_templates
        ? [{ label: "Templates", value: String(templates?.items.length ?? 0) }]
        : []),
    ],
    subtitle: "Admin control plane",
    title: "Administration shell",
  };
}

export async function fetchAdminJson<T>(path: string, init?: RequestInit): Promise<T> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  return fetchAdminJsonWithToken<T>(sessionToken, path, init);
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
  try {
    return await fetchAdminJson<T>(path, init);
  } catch {
    return null;
  }
}

export function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}
