import Link from "next/link";
import { cookies } from "next/headers";

import { AppShellSection } from "../../../components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "../../../components/ui/card";
import { Chip } from "../../../components/ui/chip";
import { StatusBanner } from "../../../components/ui/status-banner";
import { DataTable } from "../../../components/ui/table";
import { UI_LANGUAGE_COOKIE, pickText, resolveUiLanguage, type UiLanguage } from "../../../lib/ui/preferences";

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
  const cookieStore = await cookies();
  const lang = resolveUiLanguage(cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const results = await Promise.all([fetchAnalytics(), fetchStrategies(), fetchBillingOverview()]);
  const analytics = results[0];
  const strategies = results[1];
  const billing = results[2];

  const membership = billing?.membership ?? null;
  const runningCount = strategies.filter((item) => item.status === "Running").length;
  const pausedCount = strategies.filter((item) => item.status === "Paused").length;
  const blockedCount = strategies.filter((item) => item.status === "ErrorPaused").length;
  const latestWallet = analytics?.wallets[0] ?? null;
  const walletSummary = latestWallet
    ? Object.entries(latestWallet.balances)
        .slice(0, 4)
        .map(([asset, amount]) => asset + " " + amount)
        .join(" | ")
    : pickText(lang, "等待账户同步", "Awaiting account sync");
  const blockedStrategy = strategies.find((item) => item.status === "ErrorPaused") ?? null;

  const metrics = [
    {
      label: pickText(lang, "已实现收益", "Realized PnL"),
      value: analytics?.user.realized_pnl ?? "-",
      detail: pickText(lang, "已平仓网格累计结果", "Closed grid cycles settled"),
    },
    {
      label: pickText(lang, "未实现收益", "Unrealized PnL"),
      value: analytics?.user.unrealized_pnl ?? "-",
      detail: pickText(lang, "当前持仓与挂单浮动", "Open inventory and mark-to-market"),
    },
    {
      label: pickText(lang, "净收益", "Net PnL"),
      value: analytics?.user.net_pnl ?? "-",
      detail: pickText(lang, "已扣除手续费与资金费", "After fees and funding"),
    },
    {
      label: pickText(lang, "运行中策略", "Running strategies"),
      value: String(runningCount),
      detail: pickText(lang, "仍在执行的账户实例", "Actively managing orders now"),
    },
    {
      label: pickText(lang, "暂停 / 异常", "Paused / blocked"),
      value: String(pausedCount) + " / " + String(blockedCount),
      detail: pickText(lang, "异常暂停需要人工处理", "Blocked strategies need operator action"),
    },
    {
      label: pickText(lang, "会员状态", "Membership status"),
      value: describeMembershipStatus(lang, membership?.status),
      detail: membership?.active_until
        ? pickText(lang, "到期 " + membership.active_until.slice(0, 10) + "，宽限至 " + safeDate(membership.grace_until), "Renews " + membership.active_until.slice(0, 10) + ", grace until " + safeDate(membership.grace_until))
        : pickText(lang, "当前无法确认权益，启动保持 fail-closed", "Entitlement truth unavailable, starts remain fail-closed"),
    },
  ];

  const actionQueue = [
    {
      title: pickText(lang, "检查交易所连接", "Review exchange connection"),
      description: pickText(lang, "先确认 API 已绑定且校验通过，再允许新策略启动。", "Confirm API is bound and validated before starting new strategies."),
      href: "/app/exchange",
      action: pickText(lang, "打开交易所设置", "Open exchange settings"),
      tone: "warning" as const,
    },
    {
      title: blockedStrategy ? pickText(lang, "处理异常暂停策略", "Resolve blocked strategy") : pickText(lang, "查看主力策略", "Inspect lead strategy"),
      description: blockedStrategy
        ? pickText(lang, "异常策略 " + blockedStrategy.symbol + " 需要重新预检后再恢复。", "Blocked strategy " + blockedStrategy.symbol + " needs another pre-flight before restart.")
        : pickText(lang, "进入策略工作台检查收益、成本和仓位。", "Open a strategy workspace to inspect PnL, cost, and holdings."),
      href: "/app/strategies/" + (blockedStrategy?.id ?? strategies[0]?.id ?? ""),
      action: pickText(lang, "打开策略工作台", "Open strategy workspace"),
      tone: blockedStrategy ? "danger" : "info",
    },
    {
      title: pickText(lang, "确认续费订单", "Review renewal order"),
      description: pickText(lang, "链、币种、金额必须完全匹配，错付会进入人工队列。", "Chain, token, and amount must match exactly or the payment goes to manual review."),
      href: "/app/billing",
      action: pickText(lang, "查看计费中心", "Open billing center"),
      tone: "warning" as const,
    },
  ];

  return (
    <>
      <StatusBanner
        description={membership?.grace_until
          ? pickText(lang, "宽限期最晚到 " + membership.grace_until.slice(0, 10) + "，过期后新启动会被阻止。", "Grace window lasts until " + membership.grace_until.slice(0, 10) + ". New starts are blocked after that.")
          : pickText(lang, "风险条优先展示会员与运行状态，避免在启动时才发现阻塞。", "Risk strip keeps membership and runtime blockers visible before launch attempts.")}
        title={pickText(lang, "顶部风险条", "Top risk strip")}
        tone="warning"
      />
      <AppShellSection
        description={pickText(lang, "用户总览聚焦风险、操作与真实账户数据。", "User overview focuses on risk, actions, and real account state.")}
        eyebrow={pickText(lang, "用户总览", "User overview")}
        title={pickText(lang, "交易驾驶舱", "Trading cockpit")}
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
            <CardTitle>{pickText(lang, "今日动作", "Next action")}</CardTitle>
            <CardDescription>{pickText(lang, "先处理阻塞项，再进入具体工作台执行。", "Clear blockers first, then jump into the exact workspace.")}</CardDescription>
          </CardHeader>
          <CardBody>
            <ul className="text-list">
              {actionQueue.map((item) => (
                <li key={item.title}>
                  <Chip tone={item.tone}>{item.title}</Chip>
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
            <CardTitle>{pickText(lang, "账户看板", "Account watch")}</CardTitle>
            <CardDescription>{pickText(lang, "把会员、钱包和提醒放在同一侧栏，减少来回切页。", "Keep entitlement, wallet, and reminders in one side panel.")}</CardDescription>
          </CardHeader>
          <CardBody>
            <ul className="text-list">
              <li>{pickText(lang, "会员状态", "Membership status")}: {describeMembershipStatus(lang, membership?.status)}</li>
              <li>{pickText(lang, "下次续费", "Next renewal")}: {safeDate(membership?.active_until)}</li>
              <li>{pickText(lang, "宽限截止", "Grace ends")}: {safeDate(membership?.grace_until)}</li>
              <li>{pickText(lang, "钱包摘要", "Wallet snapshot")}: {walletSummary}</li>
              <li>{pickText(lang, "Telegram 提醒已覆盖会员与运行事故", "Telegram reminders cover entitlement and runtime incidents")}</li>
            </ul>
          </CardBody>
        </Card>
      </div>
      <div className="content-grid content-grid--split">
        <Card>
          <CardHeader>
            <CardTitle>{pickText(lang, "最近成交", "Recent fills")}</CardTitle>
            <CardDescription>{pickText(lang, "逐笔盈亏与 Telegram 通知保持一致。", "Per-fill PnL stays aligned with Telegram notifications.")}</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "symbol", label: pickText(lang, "交易对", "Symbol") },
                { key: "pnl", label: pickText(lang, "收益", "PnL"), align: "right" },
                { key: "state", label: pickText(lang, "状态", "State"), align: "right" },
              ]}
              rows={(analytics?.fills ?? []).map((fill, index) => {
                const pnl = fill.net_pnl || fill.realized_pnl;
                return {
                  id: fill.strategy_id + "-" + index,
                  symbol: fill.symbol,
                  pnl,
                  state: <Chip tone={pnl.startsWith("-") ? "warning" : "success"}>{describeFillState(lang, pnl)}</Chip>,
                };
              })}
            />
          </CardBody>
        </Card>
        <Card tone="subtle">
          <CardHeader>
            <CardTitle>{pickText(lang, "交易所活动", "Exchange activity")}</CardTitle>
            <CardDescription>{pickText(lang, "账户快照直接来自后端分析接口。", "Account snapshots come straight from backend analytics.")}</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "capturedAt", label: pickText(lang, "采集时间", "Captured") },
                { key: "exchange", label: pickText(lang, "交易所", "Exchange") },
                { key: "detail", label: pickText(lang, "明细", "Detail"), align: "right" },
              ]}
              rows={(analytics?.account_snapshots ?? []).map((item, index) => ({
                id: item.exchange + "-" + index,
                capturedAt: item.captured_at.replace("T", " ").slice(0, 16),
                exchange: item.exchange,
                detail: pickText(lang, "手续费 " + item.fees_paid + " | 资金费 " + item.funding_total + " | 浮盈亏 " + item.unrealized_pnl, "Fees " + item.fees_paid + " | Funding " + item.funding_total + " | Unrealized " + item.unrealized_pnl),
              }))}
            />
          </CardBody>
        </Card>
      </div>
    </>
  );
}

function safeDate(value?: string | null) {
  return value?.slice(0, 10) ?? "-";
}

function describeMembershipStatus(lang: UiLanguage, status?: string | null) {
  switch (status) {
    case "Active":
      return pickText(lang, "有效", "Active");
    case "Grace":
      return pickText(lang, "宽限期", "Grace");
    case "Frozen":
      return pickText(lang, "冻结", "Frozen");
    case "Revoked":
      return pickText(lang, "已撤销", "Revoked");
    default:
      return pickText(lang, "待确认", "Pending");
  }
}

function describeFillState(lang: UiLanguage, pnl: string) {
  return pnl.startsWith("-") ? pickText(lang, "回撤止盈", "Trailing TP") : pickText(lang, "已结算", "Settled");
}

async function fetchAnalytics(): Promise<AnalyticsReport | null> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (!sessionToken) {
    return null;
  }
  const response = await fetch(authApiBaseUrl() + "/analytics", {
    method: "GET",
    headers: { authorization: "Bearer " + sessionToken },
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
  const response = await fetch(authApiBaseUrl() + "/strategies", {
    method: "GET",
    headers: { authorization: "Bearer " + sessionToken },
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
  const response = await fetch(authApiBaseUrl() + "/billing/overview", {
    method: "GET",
    headers: { authorization: "Bearer " + sessionToken },
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
