import "server-only";

import { cookies } from "next/headers";

import type { AdminShellSnapshot } from "./mock-data";

export type AdminFlashState = {
  addressPools: { description: string; title: string; tone: "info" | "success" | "warning" | "danger" } | null;
  deposits: { description: string; title: string; tone: "info" | "success" | "warning" | "danger" } | null;
  memberships: { description: string; title: string; tone: "info" | "success" | "warning" | "danger" } | null;
  system: { description: string; title: string; tone: "info" | "success" | "warning" | "danger" } | null;
  templates: { description: string; title: string; tone: "info" | "success" | "warning" | "danger" } | null;
  users: { description: string; title: string; tone: "info" | "success" | "warning" | "danger" } | null;
};

export type MembershipRecord = {
  email: string;
  expiresAt: string;
  graceEndsAt: string | null;
  id: string;
  note: string;
  plan: string;
  status: "Active" | "Grace" | "Frozen" | "Revoked";
};

export type DepositCaseRecord = {
  amount: string;
  chain: string;
  id: string;
  issue: string;
  note: string;
  order: string;
  state: "open" | "refunded" | "credited" | "held";
  token: string;
  user: string;
};

export type AddressPoolRecord = {
  chain: "ethereum" | "bsc" | "solana";
  id: string;
  locked: number;
  queue: number;
  total: number;
};

export type TemplateRecord = {
  copies: number;
  id: string;
  market: string;
  mode: string;
  name: string;
  status: "draft" | "published";
  updatedAt: string;
};

export type StrategyRecord = {
  id: string;
  incident: string;
  market: string;
  name: string;
  state: "running" | "paused" | "error_paused" | "draft";
  symbol: string;
  user: string;
};

export type SweepJobRecord = {
  amount: string;
  asset: string;
  id: string;
  requestedAt: string;
  state: "Queued" | "Running" | "Completed";
  wallet: string;
};

export type AuditRecord = {
  action: string;
  actor: string;
  domain: "membership" | "deposit" | "pool" | "template" | "system";
  id: string;
  summary: string;
  target: string;
  timestamp: string;
};

export type SystemConfigState = {
  billing: {
    bscConfirmations: string;
    ethConfirmations: string;
    solanaConfirmations: string;
  };
  treasury: {
    bscWallet: string;
    solanaWallet: string;
    usdtPriceBsc: string;
    usdcPriceSolana: string;
  };
};

export type AdminProductState = {
  addressPools: AddressPoolRecord[];
  audit: AuditRecord[];
  counters: {
    audit: number;
    template: number;
  };
  deposits: DepositCaseRecord[];
  flash: AdminFlashState;
  memberships: MembershipRecord[];
  strategies: StrategyRecord[];
  sweeps: SweepJobRecord[];
  system: SystemConfigState;
  templates: TemplateRecord[];
};

type Store = Map<string, AdminProductState>;

const globalStore = globalThis as typeof globalThis & {
  __gridBinanceAdminProductState?: Store;
};

const store = globalStore.__gridBinanceAdminProductState ?? new Map<string, AdminProductState>();
globalStore.__gridBinanceAdminProductState = store;

export async function getCurrentAdminProductState() {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? null;
  return getAdminProductState(sessionToken);
}

export function getAdminProductState(sessionToken: string | null | undefined): AdminProductState {
  const key = sessionKey(sessionToken);
  if (!store.has(key)) {
    store.set(key, createDefaultState());
  }

  return structuredClone(store.get(key)!);
}

export function updateAdminProductState(
  sessionToken: string | null | undefined,
  updater: (state: AdminProductState) => void,
): AdminProductState {
  const next = getAdminProductState(sessionToken);
  updater(next);
  store.set(sessionKey(sessionToken), next);
  return structuredClone(next);
}

export function buildAdminShellSnapshotFromState(state: AdminProductState): AdminShellSnapshot {
  const openDeposits = state.deposits.filter((item) => item.state === "open").length;
  const totalAddresses = state.addressPools.reduce((sum, pool) => sum + pool.total, 0);
  const lockedAddresses = state.addressPools.reduce((sum, pool) => sum + pool.locked, 0);
  const poolUtilization = totalAddresses === 0 ? 0 : Math.round((lockedAddresses / totalAddresses) * 100);
  const publishedTemplates = state.templates.filter((item) => item.status === "published").length;

  return {
    banners: [
      openDeposits > 0
        ? {
            action: { href: "/admin/deposits", label: "Review queue" },
            description: "Overpayment, wrong-token, and underpayment cases stay blocked until an operator resolves them.",
            title: "Abnormal payment queue",
            tone: "danger",
          }
        : {
            action: { href: "/admin/audit", label: "Review audit" },
            description: "Deposit exceptions are clear. Continue reviewing pool changes, template releases, and system configuration writes.",
            title: "Operator queue clear",
            tone: "success",
          },
    ],
    brand: "GridBinance Ops",
    description: "Shared operations navigation for pricing, address pools, deposits, and audit review.",
    identity: {
      context: `${openDeposits} abnormal deposits open. Latest audit ${state.audit[0]?.action ?? "none"}.`,
      name: "Operator Nova",
      role: "super_admin",
    },
    nav: [
      { href: "/admin/dashboard", label: "Dashboard" },
      { href: "/admin/users", label: "Users", badge: String(state.memberships.length) },
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
      { label: "Queued orders", value: String(openDeposits) },
      { label: "Pool utilization", value: `${poolUtilization}%` },
      { label: "Templates", value: `${publishedTemplates} active` },
    ],
    subtitle: "Admin control plane",
    title: "Administration shell",
  };
}

export function appendAuditRecord(
  state: AdminProductState,
  entry: Omit<AuditRecord, "id" | "timestamp">,
) {
  state.counters.audit += 1;
  const minute = String(10 + state.counters.audit).padStart(2, "0");
  state.audit.unshift({
    ...entry,
    id: `audit-${state.counters.audit}`,
    timestamp: `2026-04-02 10:${minute}`,
  });
}

function createDefaultState(): AdminProductState {
  return {
    addressPools: [
      { chain: "ethereum", id: "pool-eth", locked: 5, queue: 1, total: 8 },
      { chain: "bsc", id: "pool-bsc", locked: 6, queue: 2, total: 7 },
      { chain: "solana", id: "pool-sol", locked: 3, queue: 0, total: 5 },
    ],
    audit: [
      {
        action: "deposit.flagged",
        actor: "Operator Mira",
        domain: "deposit",
        id: "audit-3",
        summary: "Marked ORD-4206 as abnormal transfer for manual review.",
        target: "ORD-4206",
        timestamp: "2026-04-02 10:13",
      },
      {
        action: "membership.freeze",
        actor: "Operator Nova",
        domain: "membership",
        id: "audit-2",
        summary: "Froze ava@example.com after chargeback review.",
        target: "ava@example.com",
        timestamp: "2026-04-02 10:12",
      },
      {
        action: "pool.lock",
        actor: "System",
        domain: "pool",
        id: "audit-1",
        summary: "Locked BSC address for ORD-4201 during the one-hour reservation window.",
        target: "ORD-4201",
        timestamp: "2026-04-02 10:11",
      },
    ],
    counters: {
      audit: 3,
      template: 11,
    },
    deposits: [
      {
        amount: "20.00",
        chain: "Ethereum",
        id: "dep-4195",
        issue: "Wrong token",
        note: "User sent USDC instead of USDT. Hold for manual response.",
        order: "ORD-4195",
        state: "open",
        token: "USDC",
        user: "luna@example.com",
      },
      {
        amount: "19.50",
        chain: "BSC",
        id: "dep-4201",
        issue: "Underpayment",
        note: "User short-paid by 0.50 USDT. Awaiting operator decision.",
        order: "ORD-4201",
        state: "open",
        token: "USDT",
        user: "miles@example.com",
      },
      {
        amount: "20.75",
        chain: "BSC",
        id: "dep-4204",
        issue: "Overpayment",
        note: "Excess amount held in treasury pending support reply.",
        order: "ORD-4204",
        state: "open",
        token: "USDT",
        user: "ava@example.com",
      },
      {
        amount: "20.00",
        chain: "Solana",
        id: "dep-4206",
        issue: "Abnormal transfer",
        note: "Confirmation pattern mismatched the assigned order window.",
        order: "ORD-4206",
        state: "open",
        token: "USDC",
        user: "kai@example.com",
      },
    ],
    flash: {
      addressPools: null,
      deposits: null,
      memberships: null,
      system: null,
      templates: null,
      users: null,
    },
    memberships: [
      {
        email: "luna@example.com",
        expiresAt: "2026-04-15",
        graceEndsAt: null,
        id: "mem-luna",
        note: "Healthy member with TOTP enabled.",
        plan: "Monthly",
        status: "Active",
      },
      {
        email: "miles@example.com",
        expiresAt: "2026-04-17",
        graceEndsAt: "2026-04-19",
        id: "mem-miles",
        note: "Grace-period customer waiting on renewal confirmation.",
        plan: "Quarterly",
        status: "Grace",
      },
      {
        email: "ava@example.com",
        expiresAt: "Suspended",
        graceEndsAt: null,
        id: "mem-ava",
        note: "Frozen after a manual override.",
        plan: "Yearly",
        status: "Frozen",
      },
    ],
    strategies: [
      {
        id: "strat-btc",
        incident: "Healthy runtime; no intervention needed.",
        market: "Spot",
        name: "BTC Recovery Ladder",
        state: "running",
        symbol: "BTCUSDT",
        user: "luna@example.com",
      },
      {
        id: "strat-eth",
        incident: "Hedge mode must be enabled before futures strategy restart.",
        market: "USDⓈ-M",
        name: "ETH Short Ladder",
        state: "error_paused",
        symbol: "ETHUSDT",
        user: "miles@example.com",
      },
      {
        id: "strat-sol",
        incident: "Paused during grace-period review.",
        market: "COIN-M",
        name: "SOL Neutral Range",
        state: "paused",
        symbol: "SOLUSD_PERP",
        user: "ava@example.com",
      },
    ],
    sweeps: [
      {
        amount: "420",
        asset: "USDT",
        id: "sweep-1",
        requestedAt: "2026-04-02 09:52",
        state: "Queued",
        wallet: "bsc_pool_02",
      },
      {
        amount: "380",
        asset: "USDC",
        id: "sweep-2",
        requestedAt: "2026-04-02 08:18",
        state: "Completed",
        wallet: "sol_pool_03",
      },
    ],
    system: {
      billing: {
        bscConfirmations: "12",
        ethConfirmations: "12",
        solanaConfirmations: "32",
      },
      treasury: {
        bscWallet: "bsc_treasury_main",
        solanaWallet: "sol_treasury_main",
        usdcPriceSolana: "20.00",
        usdtPriceBsc: "20.00",
      },
    },
    templates: [
      { copies: 34, id: "tpl-1", market: "Spot", mode: "classic", name: "BTC Recovery Ladder", status: "published", updatedAt: "2026-04-01" },
      { copies: 17, id: "tpl-2", market: "USDⓈ-M", mode: "short", name: "ETH Short Mean Reversion", status: "published", updatedAt: "2026-04-01" },
      { copies: 8, id: "tpl-3", market: "COIN-M", mode: "neutral", name: "SOL Neutral Range", status: "published", updatedAt: "2026-03-31" },
      { copies: 12, id: "tpl-4", market: "Spot", mode: "buy-only", name: "BNB Trend Catcher", status: "published", updatedAt: "2026-03-31" },
      { copies: 9, id: "tpl-5", market: "Spot", mode: "sell-only", name: "XRP Exit Ladder", status: "published", updatedAt: "2026-03-30" },
      { copies: 6, id: "tpl-6", market: "USDⓈ-M", mode: "long", name: "DOGE Momentum Grid", status: "published", updatedAt: "2026-03-30" },
      { copies: 5, id: "tpl-7", market: "Spot", mode: "classic", name: "ARB Balance Rebuilder", status: "published", updatedAt: "2026-03-29" },
      { copies: 11, id: "tpl-8", market: "Spot", mode: "buy-only", name: "LINK Dip Buyer", status: "published", updatedAt: "2026-03-29" },
      { copies: 4, id: "tpl-9", market: "USDⓈ-M", mode: "short", name: "ENA Breakdown Short", status: "published", updatedAt: "2026-03-28" },
      { copies: 7, id: "tpl-10", market: "COIN-M", mode: "neutral", name: "TRX Delivery Range", status: "published", updatedAt: "2026-03-28" },
      { copies: 3, id: "tpl-11", market: "Spot", mode: "classic", name: "APT Compression Grid", status: "published", updatedAt: "2026-03-27" },
    ],
  };
}

function sessionKey(sessionToken: string | null | undefined) {
  return sessionToken?.trim() || "anonymous";
}
