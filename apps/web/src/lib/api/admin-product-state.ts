import "server-only";

import { cookies } from "next/headers";

import type { AdminShellSnapshot } from "./mock-data";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

export type AdminProfile = {
  admin_access_granted: boolean;
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

export type AdminUserRecord = {
  email: string;
  latest_order_status: string | null;
  membership: AdminMembershipRecord;
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
    budget: string;
    exchange_ready: boolean;
    grid_spacing_bps: number;
    id: string;
    membership_ready: boolean;
    name: string;
    symbol: string;
    symbol_ready: boolean;
  }>;
};

export type AdminStrategyList = {
  items: Array<{
    id: string;
    market: string;
    mode: string;
    name: string;
    owner_email: string;
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
  const [profile, memberships, deposits, templates] = await Promise.all([
    getCurrentAdminProfile(),
    getAdminMembershipsData(),
    getAdminDepositsData(),
    getAdminTemplatesData(),
  ]);
  const openDeposits = deposits.abnormal_deposits.filter((item) => item.status === "manual_review_required").length;
  const membershipsNeedingAction = memberships.items.filter((item) => ["Grace", "Frozen", "Revoked"].includes(item.status)).length;

  return {
    banners: [
      {
        action: { href: "/admin/deposits", label: openDeposits > 0 ? "Review queue" : "View deposits" },
        description: profile.admin_access_granted
          ? "TOTP-backed session is active for backend operator workflows."
          : "Admin access is not currently granted.",
        title: profile.admin_access_granted ? "Admin access granted" : "Admin access missing",
        tone: profile.admin_access_granted ? "success" : "warning",
      },
    ],
    brand: "GridBinance Ops",
    description: "Backend-backed admin control plane.",
    identity: {
      context: `TOTP ${profile.totp_enabled ? "enabled" : "disabled"}. Memberships needing action ${membershipsNeedingAction}.`,
      name: profile.email,
      role: profile.admin_access_granted ? "Admin operator" : "User",
    },
    nav: [
      { href: "/admin/dashboard", label: "Dashboard" },
      { href: "/admin/users", label: "Users" },
      { href: "/admin/memberships", label: "Memberships" },
      { href: "/admin/deposits", label: "Deposits", badge: String(openDeposits) },
      { href: "/admin/address-pools", label: "Address pools" },
      { href: "/admin/templates", label: "Templates" },
      { href: "/admin/strategies", label: "Strategies" },
      { href: "/admin/sweeps", label: "Sweeps" },
      { href: "/admin/audit", label: "Audit" },
      { href: "/admin/system", label: "System" },
    ],
    quickStats: [
      { label: "Open deposits", value: String(openDeposits) },
      { label: "Membership risk", value: String(membershipsNeedingAction) },
      { label: "Templates", value: String(templates.items.length) },
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

export function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}
