import Link from "next/link";

import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Chip } from "../../../components/ui/chip";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { getCurrentUserProductState } from "../../../lib/api/user-product-state";

export default async function DashboardPage() {
  const state = await getCurrentUserProductState();
  const runningCount = state.strategies.filter((item) => item.status === "running").length;
  const errorPausedCount = state.strategies.filter((item) => item.status === "error_paused").length;

  const metrics = [
    { label: "Wallet balance", value: "18,420 USDT", detail: "Spot, futures, and pending billing reserves." },
    { label: "Total realized PnL", value: "+1,632.44 USDT", detail: "Closed cycles across all strategies." },
    { label: "Total unrealized PnL", value: "+192.51 USDT", detail: "Open inventory and futures mark-to-market." },
    { label: "Total fees", value: "-231.08 USDT", detail: "Maker, taker, and settlement fees combined." },
    { label: "Total funding fees", value: "-18.42 USDT", detail: "Applies only to active futures exposure." },
    { label: "Net profit", value: "+1,284.20 USDT", detail: "Realized plus unrealized minus fees and funding." },
    { label: "Running strategies", value: String(runningCount), detail: "Eligible to keep operating during grace only while entitlement is valid." },
    { label: "Error-paused strategies", value: String(errorPausedCount), detail: "Require remediation before restart is allowed." },
    { label: "Membership status", value: state.billing.membershipStatus, detail: state.billing.membershipStatus === "Unknown" ? "Entitlement truth is temporarily unavailable; starts remain fail-closed." : `Next renewal ${state.billing.nextRenewalAt}, grace ends ${state.billing.graceEndsAt}.` },
  ];

  const actionQueue = [
    {
      title: state.exchange.connectionStatus === "passed" ? "Exchange connection healthy" : "Complete exchange connection test",
      description: state.exchange.connectionStatus === "passed"
        ? "Spot, USDⓈ-M, and COIN-M permissions are already verified."
        : "Save masked Binance credentials, then verify spot and futures permissions.",
      href: "/app/exchange",
      action: "Review exchange setup",
    },
    {
      title: errorPausedCount > 0 ? "Resolve error-paused strategy" : "Review running strategy statistics",
      description: errorPausedCount > 0
        ? "Investigate the blocked strategy, then re-run pre-flight before restart."
        : "Open a strategy workspace to review independent PnL, fees, and cost basis.",
      href: `/app/strategies/${state.strategies.find((item) => item.status === "error_paused")?.id ?? state.strategies[0]?.id ?? ""}`,
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
              <li>Membership status: {state.billing.membershipStatus}</li>
              <li>Next renewal: {state.billing.nextRenewalAt}</li>
              <li>Grace period ends: {state.billing.graceEndsAt}</li>
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
                { key: "side", label: "Side" },
                { key: "pnl", label: "PnL", align: "right" },
                { key: "state", label: "State", align: "right" },
              ]}
              rows={state.recentFills.map((fill) => ({
                ...fill,
                state: <Chip tone={fill.state === "Trailing TP" ? "warning" : "success"}>{fill.state}</Chip>,
              }))}
            />
          </CardBody>
        </Card>
        <Card tone="subtle">
          <CardHeader>
            <CardTitle>Exchange account activity</CardTitle>
            <CardDescription>Account-level history supports analytics, reconciliation, and user trust.</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "at", label: "Timestamp" },
                { key: "activity", label: "Activity" },
                { key: "detail", label: "Detail", align: "right" },
              ]}
              rows={state.tradeHistory}
            />
          </CardBody>
        </Card>
      </div>
    </>
  );
}
