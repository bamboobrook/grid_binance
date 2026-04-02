import Link from "next/link";

import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Chip } from "../../../components/ui/chip";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";

const metrics = [
  { label: "Wallet balance", value: "18,420 USDT", detail: "Spot, futures, and pending billing reserves." },
  { label: "Net profit", value: "+1,284.20 USDT", detail: "Fees and funding are already included." },
  { label: "Running strategies", value: "5", detail: "One additional strategy is paused for review." },
  { label: "Membership", value: "13 days left", detail: "Grace period starts after expiry and lasts 48 hours." },
];

const actionQueue = [
  {
    title: "Complete exchange connection test",
    description: "Save masked Binance credentials, then verify spot and futures permissions.",
    href: "/app/exchange",
    action: "Review exchange setup",
  },
  {
    title: "Resolve error-paused strategy",
    description: "Review pre-flight blockers before restarting the ETH short ladder.",
    href: "/app/strategies/grid-btc?draft=1&edited=1&preflight=1",
    action: "Inspect runtime blocker",
  },
  {
    title: "Review exact-amount billing warning",
    description: "Membership renewal orders require the exact chain, token, and amount shown.",
    href: "/app/billing",
    action: "Review renewal order",
  },
];

const recentFills = [
  { id: "fill-1", symbol: "BTCUSDT", side: "Buy", pnl: "+82.10", state: "Settled" },
  { id: "fill-2", symbol: "ETHUSDT", side: "Sell", pnl: "+24.87", state: "Settled" },
  { id: "fill-3", symbol: "SOLUSDT", side: "Buy", pnl: "-6.24", state: "Trailing TP" },
];

const shortcuts = [
  { href: "/app/security", label: "Security Center" },
  { href: "/app/billing", label: "Billing Center" },
  { href: "/app/strategies/grid-btc", label: "Strategy Workspace" },
  { href: "/app/analytics", label: "Analytics" },
  { href: "/help/expiry-reminder", label: "Help Center" },
];

export default function DashboardPage() {
  return (
    <>
      <StatusBanner
        description="Grace-period timing stays visible on the dashboard so users see membership risk before start actions fail."
        title="Expiry reminder flow"
        tone="warning"
      />
      <AppShellSection
        description="Wallet, membership, runtime, and recovery actions stay visible in one place for the user operating cockpit."
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
            <CardDescription>Commercial recovery work stays focused on explicit user actions instead of placeholder boxes.</CardDescription>
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
            <CardTitle>Workspace shortcuts</CardTitle>
            <CardDescription>These links preserve the documented page map and the older browser smoke expectations.</CardDescription>
          </CardHeader>
          <CardBody>
            <ul className="text-list">
              {shortcuts.map((item) => (
                <li key={item.href}>
                  <Link href={item.href}>{item.label}</Link>
                </li>
              ))}
            </ul>
          </CardBody>
        </Card>
      </div>
      <div className="content-grid content-grid--split">
        <Card>
          <CardHeader>
            <CardTitle>Recent fills</CardTitle>
            <CardDescription>Per-fill PnL stays visible for web and Telegram notification parity.</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "symbol", label: "Symbol" },
                { key: "side", label: "Side" },
                { key: "pnl", label: "PnL", align: "right" },
                { key: "state", label: "State", align: "right" },
              ]}
              rows={recentFills.map((fill) => ({
                ...fill,
                state: <Chip tone={fill.state === "Trailing TP" ? "warning" : "success"}>{fill.state}</Chip>,
              }))}
            />
          </CardBody>
        </Card>
        <Card tone="subtle">
          <CardHeader>
            <CardTitle>Membership posture</CardTitle>
            <CardDescription>Next renewal and grace-period consequences remain actionable.</CardDescription>
          </CardHeader>
          <CardBody>
            <ul className="text-list">
              <li>Next renewal: 2026-04-15</li>
              <li>Grace period ends: 2026-04-17</li>
              <li>New starts are blocked when grace expires.</li>
              <li>Telegram reminders are active for membership and runtime incidents.</li>
            </ul>
          </CardBody>
        </Card>
      </div>
    </>
  );
}
