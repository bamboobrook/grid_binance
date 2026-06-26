import Link from "next/link";
import { cookies } from "next/headers";
import type { ReactNode } from "react";
import { ArrowRight, Bot, CreditCard, TrendingUp, WalletCards } from "lucide-react";

import { AppShellSection } from "@/components/shell/app-shell-section";
import { PnlTrendChart } from "@/components/ui/pnl-trend-chart";
import { StrategyHealthCards } from "@/components/ui/strategy-health-cards";
import { formatPnl, formatPercent } from "@/lib/ui/format";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";
import { formatTaipeiDateTime } from "@/lib/ui/time";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";
const TREND_RANGES = [7, 30, 90] as const;
type TrendRange = (typeof TREND_RANGES)[number];

type AnalyticsResponse = {
  total_pnl: string;
  total_pnl_pct: string;
  fee_total: string;
  funding_total: string;
  strategy_health: { running: number; paused: number; error_paused: number; stopped: number; draft: number };
  pnl_trend: Array<{ date: string; pnl: number }>;
};

function firstValue(value?: string | string[]) {
  return Array.isArray(value) ? value[0] : value;
}

export default async function DashboardPage({
  params,
  searchParams,
}: {
  params: Promise<{ locale: string }>;
  searchParams?: Promise<{ range?: string | string[] }>;
}) {
  const { locale } = await params;
  const query = (await searchParams) ?? {};
  const lang = (locale === "zh" ? "zh" : "en") as UiLanguage;
  const trendRange = resolveTrendRange(firstValue(query.range));
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  const previewMode = process.env.NEXT_PUBLIC_UI_PREVIEW === "1";
  const analytics = sessionToken ? await fetchAnalytics(sessionToken) : previewMode ? previewAnalytics() : null;
  const membership = sessionToken ? await fetchMembership(sessionToken) : previewMode ? previewMembership() : null;
  const accountSnapshots = sessionToken ? await fetchAccountSnapshots(sessionToken) : previewMode ? previewActivities(lang) : null;
  const assetAllocation = sessionToken ? await fetchAssetAllocation(sessionToken) : previewMode ? previewAssets() : null;
  const health = analytics
    ? { running: analytics.strategy_health.running, paused: analytics.strategy_health.paused, errorPaused: analytics.strategy_health.error_paused, stopped: analytics.strategy_health.stopped, draft: analytics.strategy_health.draft }
    : { running: 0, paused: 0, errorPaused: 0, stopped: 0, draft: 0 };
  const hasStrategies = health.running + health.paused + health.errorPaused + health.stopped + health.draft > 0;
  const pnlTrend = analytics?.pnl_trend ?? [];
  const visiblePnlTrend = filterPnlTrend(pnlTrend, trendRange);
  const totalPnl = analytics ? Number(analytics.total_pnl) : null;
  const totalPnlPct = analytics ? Number(analytics.total_pnl_pct) : null;
  const nextActionHref = hasStrategies ? `/${locale}/app/orders` : `/${locale}/app/exchange`;
  const nextActionLabel = hasStrategies
    ? pickText(lang, "查看订单和成交", "Review orders and fills")
    : pickText(lang, "连接币安 API", "Connect Binance API");
  const riskActionHref = health.errorPaused > 0 ? `/${locale}/app/strategies?status=ErrorPaused&view=table` : "";
  const overviewCards = [
    {
      href: `/${locale}/app/exchange`,
      icon: <WalletCards className="h-4 w-4" />,
      label: pickText(lang, "交易所", "Exchange"),
      value: previewMode || sessionToken ? pickText(lang, "待确认连接", "Connection pending") : pickText(lang, "未登录", "Signed out"),
      detail: pickText(lang, "API 和余额状态", "API and balance status"),
    },
    {
      href: `/${locale}/app/strategies`,
      icon: <Bot className="h-4 w-4" />,
      label: pickText(lang, "机器人", "Bots"),
      value: pickText(lang, `${health.running} 个运行中`, `${health.running} running`),
      detail: health.errorPaused > 0 ? pickText(lang, "有异常需要处理", "Blocked bots need attention") : pickText(lang, "暂无异常阻塞", "No blocked bots"),
    },
    {
      href: `/${locale}/app/billing`,
      icon: <CreditCard className="h-4 w-4" />,
      label: pickText(lang, "会员", "Membership"),
      value: membership ? membership.plan : pickText(lang, "暂无会员信息", "No membership info"),
      detail: membership?.expires_at ? pickText(lang, `到期 ${membership.expires_at.slice(0, 10)}`, `Expires ${membership.expires_at.slice(0, 10)}`) : pickText(lang, "查看会员状态", "Check membership"),
    },
    {
      href: `/${locale}/app/analytics`,
      icon: <TrendingUp className="h-4 w-4" />,
      label: pickText(lang, "累计收益", "Total PnL"),
      value: totalPnl == null ? "-" : formatPnl(totalPnl),
      detail: totalPnlPct == null ? pickText(lang, "暂无收益率", "No return yet") : formatPercent(totalPnlPct),
    },
  ];
  const focusItems = [
    {
      href: `/${locale}/app/orders`,
      actionLabel: hasStrategies ? pickText(lang, "查看", "Review") : "",
      label: pickText(lang, "订单检查", "Order check"),
      value: hasStrategies ? "" : pickText(lang, "待启动", "Pending"),
    },
    {
      actionLabel: health.errorPaused > 0 ? pickText(lang, "处理", "Fix") : "",
      href: riskActionHref,
      label: pickText(lang, "风险提醒", "Risk alerts"),
      value: String(health.errorPaused),
    },
    {
      detail: totalPnl == null ? pickText(lang, "累计 -", "Total -") : pickText(lang, `累计 ${formatPnl(totalPnl)}`, `Total ${formatPnl(totalPnl)}`),
      label: pickText(lang, "收益变化", "PnL change"),
      value: totalPnlPct == null ? "-" : formatPercent(totalPnlPct),
    },
  ];
  const trendRangeOptions = [
    { label: pickText(lang, "近 7 日", "7D"), value: 7 },
    { label: pickText(lang, "近 30 日", "30D"), value: 30 },
    { label: pickText(lang, "近 90 日", "90D"), value: 90 },
  ];

  return (
    <AppShellSection
      actions={
        <Link className="inline-flex h-9 items-center rounded-md bg-primary px-3 text-sm font-bold text-primary-foreground hover:bg-primary/90" href={nextActionHref}>
          {nextActionLabel}
          <ArrowRight className="ml-2 h-4 w-4" />
        </Link>
      }
      eyebrow={pickText(lang, "用户首页", "User home")}
      title={pickText(lang, "我的交易机器人", "My trading bots")}
    >
      <div className="grid gap-3 md:grid-cols-2 xl:grid-cols-4">
        {overviewCards.map((card) => (
          <StatusCard
            detail={card.detail}
            href={card.href}
            icon={card.icon}
            key={card.label}
            label={card.label}
            value={card.value}
          />
        ))}
      </div>

      <section className="rounded-md border border-border bg-card p-4">
        <div className="mb-3 flex flex-col justify-between gap-2 sm:flex-row sm:items-center">
          <div>
            <p className="text-xs font-bold uppercase text-muted-foreground">{pickText(lang, "机器人状态", "Bot status")}</p>
            <h2 className="text-lg font-bold">{pickText(lang, "运行概览", "Runtime overview")}</h2>
          </div>
        </div>
        <StrategyHealthCards health={health} lang={lang} />
      </section>

      <div className="grid gap-4 xl:grid-cols-[minmax(0,1fr)_22rem]">
        <section className="rounded-md border border-border bg-card p-4">
          <div className="mb-4 flex items-center justify-between gap-3">
            <div>
              <p className="text-xs font-bold uppercase text-muted-foreground">{pickText(lang, "收益走势", "PnL trend")}</p>
              <h2 className="text-lg font-bold">{describeTrendRange(lang, trendRange)}</h2>
            </div>
            <div className="flex flex-col items-end gap-2">
              <div className="flex items-center gap-1 rounded-sm border border-border bg-background p-1">
                {trendRangeOptions.map((option) => (
                  <Link
                    className={`inline-flex h-7 items-center rounded-sm px-2 text-[11px] font-bold transition-colors ${
                      option.value === trendRange
                        ? "bg-primary text-primary-foreground"
                        : "text-muted-foreground hover:bg-secondary hover:text-foreground"
                    }`}
                    href={`/${locale}/app/dashboard?range=${option.value}`}
                    key={option.value}
                  >
                    {option.label}
                  </Link>
                ))}
              </div>
              <div className="text-right">
                <p className="text-xl font-black">{totalPnl == null ? "-" : formatPnl(totalPnl)}</p>
                <p className="text-xs text-muted-foreground">{totalPnlPct == null ? "-" : formatPercent(totalPnlPct)}</p>
              </div>
            </div>
          </div>
          {visiblePnlTrend.length > 0 ? (
            <PnlTrendChart data={visiblePnlTrend} height={190} lang={lang} />
          ) : (
            <div className="flex h-40 items-center justify-center rounded-md border border-dashed border-border text-sm text-muted-foreground">
              {pickText(lang, "启动机器人后会显示收益趋势", "PnL trend appears after a bot starts")}
            </div>
          )}
        </section>

        <section className="flex rounded-md border border-border bg-card p-4">
          <div className="flex w-full flex-col">
            <p className="text-xs font-bold uppercase text-muted-foreground">{pickText(lang, "今日重点", "Today")}</p>
            <div className="mt-3 grid flex-1 gap-2 sm:grid-cols-3 xl:grid-cols-1 xl:grid-rows-3">
              {focusItems.map((item) => (
                <div className="flex min-h-16 items-center justify-between gap-3 rounded-md border border-border bg-background p-3" key={item.label}>
                  <div className="min-w-0">
                    <p className="text-sm font-bold">{item.label}</p>
                    {"detail" in item && item.detail ? (
                      <p className="mt-1 truncate text-xs text-muted-foreground">{item.detail}</p>
                    ) : null}
                  </div>
                  {"href" in item && item.href && "actionLabel" in item && item.actionLabel ? (
                    <div className="flex shrink-0 items-center gap-2">
                      {item.value ? <strong className="text-lg font-black">{item.value}</strong> : null}
                      <Link className="inline-flex h-7 items-center rounded-sm bg-secondary px-2 text-xs font-bold text-foreground hover:bg-secondary/80" href={item.href}>
                        {item.actionLabel}
                      </Link>
                    </div>
                  ) : item.value ? (
                    <strong className="shrink-0 text-lg font-black">{item.value}</strong>
                  ) : null}
                </div>
              ))}
            </div>
          </div>
        </section>
      </div>

      <div className="grid gap-4 lg:grid-cols-2">
        <section className="rounded-md border border-border bg-card p-4">
          <h2 className="text-sm font-bold">{pickText(lang, "最近活动", "Recent activity")}</h2>
          {accountSnapshots && accountSnapshots.length > 0 ? (
            <ul className="mt-3 space-y-3 text-sm">
              {accountSnapshots.slice(0, 5).map((snap) => (
                <li className="flex items-center justify-between gap-4 border-b border-border pb-2 last:border-b-0 last:pb-0" key={snap.id}>
                  <span>{snap.description}</span>
                  <span className="shrink-0 text-xs text-muted-foreground">{formatTaipeiDateTime(snap.created_at, lang)}</span>
                </li>
              ))}
            </ul>
          ) : (
            <p className="mt-3 text-sm text-muted-foreground">{pickText(lang, "暂无活动记录", "No recent activity")}</p>
          )}
        </section>

        <section className="rounded-md border border-border bg-card p-4" data-asset-allocation-chart>
          <h2 className="text-sm font-bold">{pickText(lang, "资产分布", "Asset allocation")}</h2>
          {assetAllocation && assetAllocation.length > 0 ? (
            <ul className="mt-3 space-y-3 text-sm">
              {assetAllocation.map((item) => (
                <li className="grid grid-cols-[5rem_1fr_auto] items-center gap-3" key={item.symbol}>
                  <span className="font-mono font-bold">{item.symbol}</span>
                  <span className="h-2 overflow-hidden rounded-full bg-secondary">
                    <span className="block h-full rounded-full bg-primary" style={{ width: `${Math.min(Number(item.pct), 100)}%` }} />
                  </span>
                  <span className="text-xs text-muted-foreground">{item.pct}%</span>
                </li>
              ))}
            </ul>
          ) : (
            <p className="mt-3 text-sm text-muted-foreground">{pickText(lang, "暂无资产数据", "No asset data")}</p>
          )}
        </section>
      </div>
    </AppShellSection>
  );
}

function resolveTrendRange(range?: string): TrendRange {
  const parsed = Number(range);
  return TREND_RANGES.includes(parsed as TrendRange) ? (parsed as TrendRange) : 30;
}

function describeTrendRange(lang: UiLanguage, range: TrendRange) {
  switch (range) {
    case 7:
      return pickText(lang, "近 7 日", "Last 7 days");
    case 90:
      return pickText(lang, "近 90 日", "Last 90 days");
    default:
      return pickText(lang, "近 30 日", "Last 30 days");
  }
}

function filterPnlTrend(data: Array<{ date: string; pnl: number }>, range: TrendRange) {
  return data.slice(-range);
}

function StatusCard({
  detail,
  href,
  icon,
  label,
  value,
}: {
  detail: string;
  href: string;
  icon: ReactNode;
  label: string;
  value: string;
}) {
  return (
    <Link className="rounded-md border border-border bg-card p-4 transition-colors hover:bg-secondary" href={href}>
      <div className="flex items-center gap-2 text-muted-foreground">
        {icon}
        <p className="text-xs font-bold uppercase">{label}</p>
      </div>
      <p className="mt-2 text-sm font-bold text-foreground">{value}</p>
      <p className="mt-1 text-xs text-muted-foreground">{detail}</p>
    </Link>
  );
}

async function fetchAnalytics(sessionToken: string): Promise<AnalyticsResponse | null> {
  try {
    const response = await fetch(authApiBaseUrl() + "/analytics/dashboard", {
      method: "GET",
      headers: { authorization: "Bearer " + sessionToken },
      cache: "no-store",
    });
    if (!response.ok) return null;
    return (await response.json()) as AnalyticsResponse;
  } catch {
    return null;
  }
}

async function fetchMembership(sessionToken: string): Promise<{ plan: string; expires_at: string | null } | null> {
  try {
    const response = await fetch(authApiBaseUrl() + "/billing/membership", {
      method: "GET",
      headers: { authorization: "Bearer " + sessionToken },
      cache: "no-store",
    });
    if (!response.ok) return null;
    return (await response.json()) as { plan: string; expires_at: string | null };
  } catch {
    return null;
  }
}

async function fetchAccountSnapshots(sessionToken: string): Promise<Array<{ id: string; description: string; created_at: string }> | null> {
  try {
    const response = await fetch(authApiBaseUrl() + "/analytics/account_snapshots", {
      method: "GET",
      headers: { authorization: "Bearer " + sessionToken },
      cache: "no-store",
    });
    if (!response.ok) return null;
    return (await response.json()) as Array<{ id: string; description: string; created_at: string }>;
  } catch {
    return null;
  }
}

async function fetchAssetAllocation(sessionToken: string): Promise<Array<{ symbol: string; value: string; pct: string }> | null> {
  try {
    const response = await fetch(authApiBaseUrl() + "/analytics/asset_allocation", {
      method: "GET",
      headers: { authorization: "Bearer " + sessionToken },
      cache: "no-store",
    });
    if (!response.ok) return null;
    return (await response.json()) as Array<{ symbol: string; value: string; pct: string }>;
  } catch {
    return null;
  }
}

function previewAnalytics(): AnalyticsResponse {
  return {
    total_pnl: "128.64",
    total_pnl_pct: "3.21",
    fee_total: "-4.80",
    funding_total: "1.24",
    strategy_health: { running: 1, paused: 0, error_paused: 0, stopped: 0, draft: 1 },
    pnl_trend: buildPreviewPnlTrend(),
  };
}

function buildPreviewPnlTrend() {
  return Array.from({ length: 90 }, (_, index) => {
    const date = new Date(Date.UTC(2026, 2, 19 + index));
    const month = String(date.getUTCMonth() + 1).padStart(2, "0");
    const day = String(date.getUTCDate()).padStart(2, "0");
    const wave = Math.sin(index / 6) * 4;
    const pullback = index > 52 && index < 62 ? -10 + (index - 52) * 1.2 : 0;
    const pnl = Math.max(0, Math.round(index * 1.28 + wave + pullback));
    return { date: `${month}-${day}`, pnl };
  });
}

function previewMembership() {
  return { plan: "Starter", expires_at: "2026-07-11T00:00:00Z" };
}

function previewActivities(lang: UiLanguage) {
  return [
    {
      id: "preview-1",
      description: pickText(lang, "BTCUSDT 普通网格完成一笔卖出", "BTCUSDT spot grid completed one sell fill"),
      created_at: "2026-06-11T01:20:00Z",
    },
    {
      id: "preview-2",
      description: pickText(lang, "系统提醒：API 权限保持安全", "System reminder: API permissions remain safe"),
      created_at: "2026-06-11T00:40:00Z",
    },
    {
      id: "preview-3",
      description: pickText(lang, "马丁组合回测任务已生成候选", "DCA portfolio backtest generated candidates"),
      created_at: "2026-06-10T18:10:00Z",
    },
  ];
}

function previewAssets() {
  return [
    { symbol: "USDT", value: "1,820", pct: "66" },
    { symbol: "BTC", value: "540", pct: "20" },
    { symbol: "ETH", value: "380", pct: "14" },
  ];
}

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}
