import { cookies } from "next/headers";

import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { DataTable } from "../../../components/ui/table";
import { StatusBanner } from "../../../components/ui/status-banner";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

type AnalyticsReport = {
  costs: { fees_paid: string; funding_total: string };
  exchange_trades: Array<{
    exchange: string;
    fee_amount: string | null;
    fee_asset: string | null;
    price: string;
    quantity: string;
    side: string;
    symbol: string;
    trade_id: string;
    traded_at: string;
  }>;
  strategies: Array<{
    current_state: string;
    fees_paid: string;
    funding_total: string;
    net_pnl: string;
    realized_pnl: string;
    strategy_id: string;
    symbol: string;
    unrealized_pnl: string;
  }>;
  user: {
    exchange_trade_count: number;
    fees_paid: string;
    funding_total: string;
    net_pnl: string;
    realized_pnl: string;
    unrealized_pnl: string;
    wallet_asset_count: number;
  };
  wallets: Array<{
    balances: Record<string, string>;
    captured_at: string;
    exchange: string;
    wallet_type: string;
  }>;
};

export default async function AnalyticsPage() {
  const analytics = await fetchAnalytics();

  return (
    <>
      <StatusBanner
        description="Analytics now renders the backend report directly, including strategy totals, wallet snapshots, and recent exchange trades."
        title="Analytics"
        tone="info"
      />
      <AppShellSection
        description="Account-level and strategy-level statistics stay visible in their dedicated workspace."
        eyebrow="Analytics"
        title="Analytics"
        actions={<div className="button-row"><a className="button button--ghost" href="/api/user/exports/strategy-stats">Download strategy stats CSV</a><a className="button button--ghost" href="/api/user/exports/payments">Download payments CSV</a></div>}
      >
        <div className="content-grid content-grid--metrics">
          {[
            ["Realized PnL", analytics?.user.realized_pnl ?? "-"],
            ["Unrealized PnL", analytics?.user.unrealized_pnl ?? "-"],
            ["Fees paid", analytics?.user.fees_paid ?? "-"],
            ["Funding total", analytics?.user.funding_total ?? "-"],
            ["Net PnL", analytics?.user.net_pnl ?? "-"],
            ["Exchange trades", String(analytics?.user.exchange_trade_count ?? 0)],
          ].map(([label, value]) => (
            <Card key={label}>
              <CardHeader>
                <CardTitle>{value}</CardTitle>
                <CardDescription>{label}</CardDescription>
              </CardHeader>
            </Card>
          ))}
        </div>
      </AppShellSection>
      <div className="content-grid content-grid--split">
        <Card>
          <CardHeader>
            <CardTitle>Strategy statistics</CardTitle>
            <CardDescription>Independent strategy-level realized, unrealized, fee, funding, and net totals.</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "strategy", label: "Strategy" },
                { key: "symbol", label: "Symbol" },
                { key: "detail", label: "Detail" },
                { key: "net", label: "Net", align: "right" },
              ]}
              rows={(analytics?.strategies ?? []).map((row) => ({
                id: row.strategy_id,
                strategy: row.strategy_id,
                symbol: row.symbol,
                detail: `${row.current_state} · Realized ${row.realized_pnl} · Fees ${row.fees_paid} · Funding ${row.funding_total}`,
                net: row.net_pnl,
              }))}
            />
          </CardBody>
        </Card>
        <Card tone="subtle">
          <CardHeader>
            <CardTitle>Wallet snapshots</CardTitle>
            <CardDescription>Captured wallet state for platform analytics and reconciliation.</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "exchange", label: "Exchange" },
                { key: "wallet", label: "Wallet" },
                { key: "balances", label: "Balances", align: "right" },
              ]}
              rows={(analytics?.wallets ?? []).map((row, index) => ({
                id: `${row.exchange}-${index}`,
                exchange: row.exchange,
                wallet: row.wallet_type,
                balances: Object.entries(row.balances).map(([asset, amount]) => `${asset} ${amount}`).join(" | "),
              }))}
            />
          </CardBody>
        </Card>
      </div>
      <div className="content-grid content-grid--split">
        <Card>
          <CardHeader>
            <CardTitle>Recent exchange trades</CardTitle>
            <CardDescription>Recent Binance-side trades stay visible here so users can reconcile fills, fees, and strategy behavior.</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "at", label: "Timestamp" },
                { key: "symbol", label: "Symbol" },
                { key: "detail", label: "Detail" },
                { key: "fee", label: "Fee", align: "right" },
              ]}
              rows={(analytics?.exchange_trades ?? []).map((row) => ({
                id: row.trade_id,
                at: row.traded_at.replace("T", " ").slice(0, 16),
                symbol: row.symbol,
                detail: `${row.exchange} · ${row.side} · ${row.quantity} @ ${row.price}`,
                fee: row.fee_amount ? `${row.fee_amount} ${row.fee_asset ?? ""}`.trim() : "-",
              }))}
            />
          </CardBody>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>Cost summary</CardTitle>
            <CardDescription>Fees and funding are preserved as separate cost lines.</CardDescription>
          </CardHeader>
          <CardBody>
            <ul className="text-list">
              <li>Fees paid: {analytics?.costs.fees_paid ?? "-"}</li>
              <li>Funding total: {analytics?.costs.funding_total ?? "-"}</li>
              <li>Wallet asset count: {String(analytics?.user.wallet_asset_count ?? 0)}</li>
            </ul>
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

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}
