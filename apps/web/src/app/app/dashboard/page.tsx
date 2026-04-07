import Link from "next/link";
import { cookies } from "next/headers";

import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Chip } from "../../../components/ui/chip";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

type AnalyticsReport = {
  account_snapshots: Array<{
    captured_at: string;
    exchange: string;
    fees_paid: string;
    funding_total: string;
    unrealized_pnl: string;
  }>;
  fills: Array<{
    net_pnl: string;
    realized_pnl: string;
    strategy_id: string;
    symbol: string;
  }>;
  user: {
    fees_paid: string;
    funding_total: string;
    net_pnl: string;
    realized_pnl: string;
    unrealized_pnl: string;
    wallet_asset_count: number;
  };
  wallets: Array<{
    balances: Record<string, string>;
    exchange: string;
    wallet_type: string;
  }>;
};

type StrategyListResponse = {
  items: Array<{
    id: string;
    status: string;
    symbol: string;
  }>;
};

type BillingOverview = {
  membership: {
    active_until?: string | null;
    grace_until?: string | null;
    status: string;
  };
};

export default async function DashboardPage() {
  const [analytics, strategies, billing] = await Promise.all([
    fetchAnalytics(),
    fetchStrategies(),
    fetchBillingOverview(),
  ]);
  const runningCount = strategies.filter((item) => item.status === "Running").length;
  const errorPausedCount = strategies.filter((item) => item.status === "ErrorPaused").length;
  const membership = billing?.membership;
  const latestWallet = analytics?.wallets[0] ?? null;
  const walletSummary = latestWallet
    ? Object.entries(latestWallet.balances)
        .slice(0, 4)
        .map(([asset, amount]) => `${asset} ${amount}`)
        .join(" | ")
    : "Unavailable";

  const metrics = [
    { label: "Total realized PnL", value: analytics?.user.realized_pnl ?? "-", detail: "Closed cycles across all strategies." },
    { label: "Total unrealized PnL", value: analytics?.user.unrealized_pnl ?? "-", detail: "Open inventory and futures mark-to-market." },
    { label: "Total fees", value: analytics?.user.fees_paid ?? "-", detail: "Maker, taker, and settlement fees combined." },
    { label: "Total funding fees", value: analytics?.user.funding_total ?? "-", detail: "Applies only to active futures exposure." },
    { label: "Net profit", value: analytics?.user.net_pnl ?? "-", detail: "Realized plus unrealized minus fees and funding." },
    { label: "Running strategies", value: String(runningCount), detail: "Eligible to keep operating during grace only while entitlement is valid." },
    { label: "Error-paused strategies", value: String(errorPausedCount), detail: "Require remediation before restart is allowed." },
    { label: "Membership status", value: membership?.status ?? "Unknown", detail: membership?.active_until ? `Next renewal ${membership.active_until.slice(0, 10)}, grace ends ${membership.grace_until?.slice(0, 10) ?? "-"}.` : "Entitlement truth is temporarily unavailable; starts remain fail-closed." },
    { label: "Wallet assets", value: String(analytics?.user.wallet_asset_count ?? 0), detail: walletSummary },
  ];

  const actionQueue = [
    {
      title: "Complete exchange connection test",
      description: "Save masked Binance credentials, then verify spot and futures permissions.",
      href: "/app/exchange",
      action: "Review exchange setup",
    },
    {
      title: errorPausedCount > 0 ? "Resolve error-paused strategy" : "Review running strategy statistics",
      description: errorPausedCount > 0
        ? "Investigate the blocked strategy, then re-run pre-flight before restart."
        : "Open a strategy workspace to review independent PnL, fees, and cost basis.",
      href: `/app/strategies/${strategies.find((item) => item.status === "ErrorPaused")?.id ?? strategies[0]?.id ?? ""}`,
      action: "Open strategy workspace",
    },
    {
      title: "Review exact-amount billing warning",
      description: "Membership renewal orders require the exact chain, token, and amount shown.",
      href: "/app/billing",
      action: "Review renewal order",
    },
  ];

  return (
    <>
      <StatusBanner
        description="Grace-period timing stays visible on the dashboard so users see membership risk before start actions fail."
        title="Expiry reminder flow"
        tone="warning"
      />
      <AppShellSection
        description="Wallet, reporting, exchange activity, and lifecycle recovery actions stay visible in one operating cockpit."
        eyebrow="User overview"
        title="User Dashboard"
      >
        <div className="content-grid content-grid--metrics">
          {metrics.map((metric) => (
            <Card key={metric.label}>
              <CardHeader>
                <CardTitle>{metric.value}</CardTitle>
                <CardDescription>{metric.label}</CardDescription>
              </CardHeader>
              <CardBody>{metric.detail}</CardBody>
            </Card>
          ))}
        </div>
      </AppShellSection>
      <div className="content-grid content-grid--split">
        <Card tone="accent">
          <CardHeader>
            <CardTitle>Action queue</CardTitle>
            <CardDescription>Operational blockers and next actions stay explicit instead of being hidden behind background automation.</CardDescription>
          </CardHeader>
          <CardBody>
            <ul className="text-list">
              {actionQueue.map((item) => (
                <li key={item.title}>
                  <strong>{item.title}</strong>
                  <br />
                  <span>{item.description}</span>
                  <br />
                  <Link href={item.href}>{item.action}</Link>
                </li>
              ))}
            </ul>
          </CardBody>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>Membership posture</CardTitle>
            <CardDescription>Entitlement, grace behavior, and reminders remain visible beside runtime data.</CardDescription>
          </CardHeader>
          <CardBody>
            <ul className="text-list">
              <li>Membership status: {membership?.status ?? "Unknown"}</li>
              <li>Next renewal: {membership?.active_until?.slice(0, 10) ?? "Unavailable"}</li>
              <li>Grace period ends: {membership?.grace_until?.slice(0, 10) ?? "Unavailable"}</li>
              <li>New starts are blocked when grace expires.</li>
              <li>Telegram reminders are active for membership and runtime incidents.</li>
            </ul>
          </CardBody>
        </Card>
      </div>
      <div className="content-grid content-grid--split">
        <Card>
          <CardHeader>
            <CardTitle>Recent fills</CardTitle>
            <CardDescription>Per-fill profit remains visible for web and Telegram notification parity.</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "symbol", label: "Symbol" },
                { key: "pnl", label: "PnL", align: "right" },
                { key: "state", label: "State", align: "right" },
              ]}
              rows={(analytics?.fills ?? []).map((fill, index) => ({
                id: `${fill.strategy_id}-${index}`,
                symbol: fill.symbol,
                pnl: fill.net_pnl || fill.realized_pnl,
                state: <Chip tone={(fill.net_pnl || fill.realized_pnl).startsWith("-") ? "warning" : "success"}>{(fill.net_pnl || fill.realized_pnl).startsWith("-") ? "Trailing TP" : "Settled"}</Chip>,
              }))}
            />
          </CardBody>
        </Card>
        <Card tone="subtle">
          <CardHeader>
            <CardTitle>Exchange account activity</CardTitle>
            <CardDescription>Latest account snapshots come from backend analytics instead of strategy placeholders.</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "capturedAt", label: "Captured" },
                { key: "exchange", label: "Exchange" },
                { key: "detail", label: "Detail", align: "right" },
              ]}
              rows={(analytics?.account_snapshots ?? []).map((item, index) => ({
                id: `${item.exchange}-${index}`,
                capturedAt: item.captured_at,
                exchange: item.exchange,
                detail: `Fees ${item.fees_paid} | Funding ${item.funding_total} | Unrealized ${item.unrealized_pnl}`,
              }))}
            />
          </CardBody>
        </Card>
      </div>
    </>
  );
}

async function fetchAnalytics(): Promise<AnalyticsReport | null> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (!sessionToken) {
    return null;
  }
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

async function fetchStrategies(): Promise<StrategyListResponse["items"]> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (!sessionToken) {
    return [];
  }
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

async function fetchBillingOverview(): Promise<BillingOverview | null> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (!sessionToken) {
    return null;
  }
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

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}
