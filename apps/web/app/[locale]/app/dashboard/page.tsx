import Link from "next/link";
import { cookies } from "next/headers";
import { getTranslations } from "next-intl/server";
import { Activity, AlertTriangle, History, Wallet, Zap } from "lucide-react";

import { Button } from "@/components/ui/form";
import { Card } from "@/components/ui/card";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";
import { DISPLAY_TIME_ZONE, formatTaipeiDate, formatTaipeiDateTime } from "@/lib/ui/time";

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
    captured_at: string;
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
  membership?: {
    active_until?: string | null;
    grace_until?: string | null;
    status?: string | null;
  };
};

export default async function DashboardPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const lang: UiLanguage = locale === "en" ? "en" : "zh";
  const t = await getTranslations({ locale, namespace: "dashboard" });

  const [analytics, strategies, billing] = await Promise.all([fetchAnalytics(), fetchStrategies(), fetchBillingOverview()]);
  const runningCount = strategies.filter((item) => item.status === "Running").length;
  const errorPausedCount = strategies.filter((item) => item.status === "ErrorPaused").length;
  const membershipStatus = billing?.membership?.status ?? pickText(lang, "待开通", "Pending");
  const assetAllocation = buildAssetAllocation(analytics?.wallets ?? []);

  const metrics = [
    { label: t("metrics.realizedPnL"), value: analytics?.user.realized_pnl ?? "0.00", color: "text-emerald-500" },
    { label: t("metrics.unrealizedPnL"), value: analytics?.user.unrealized_pnl ?? "0.00", color: "text-blue-500" },
    { label: t("metrics.netPnL"), value: analytics?.user.net_pnl ?? "0.00", color: "text-amber-500" },
    { label: t("metrics.runningBots"), value: String(runningCount), color: "text-emerald-500" },
    { label: pickText(lang, "手续费", "Fees paid"), value: analytics?.user.fees_paid ?? "0.00", color: "text-foreground" },
    { label: pickText(lang, "资金费", "Funding total"), value: analytics?.user.funding_total ?? "0.00", color: "text-foreground" },
    { label: pickText(lang, "会员状态", "Membership Status"), value: membershipStatus, color: "text-foreground" },
    { label: pickText(lang, "异常阻塞", "ErrorPaused"), value: String(errorPausedCount), color: errorPausedCount > 0 ? "text-red-500" : "text-foreground" },
  ];

  return (
    <div className="flex flex-col space-y-4 max-w-[1600px] mx-auto h-full">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h1 className="text-xl font-bold tracking-tight text-foreground">{t("title")}</h1>
        </div>
        <div className="flex flex-wrap items-center gap-3">
          <Button className="h-8 px-3 text-xs bg-transparent border border-border text-foreground hover:bg-secondary/70">
            <History className="w-3.5 h-3.5 mr-1.5" />
            {pickText(lang, "最近24小时", "Last 24h")}
          </Button>
          <Link href={`/${locale}/app/strategies/new`}>
            <Button className="h-8 px-4 text-xs font-semibold">
              <Zap className="w-3.5 h-3.5 mr-1.5" />
              {pickText(lang, "新建机器人", "New Bot")}
            </Button>
          </Link>
        </div>
      </div>

      <div className="grid grid-cols-2 gap-4 md:grid-cols-4 xl:grid-cols-8">
        {metrics.map((metric) => (
          <div key={metric.label} className="bg-card border border-border/60 rounded-xl p-4 flex flex-col justify-center">
            <span className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider mb-1">{metric.label}</span>
            <span className={`text-xl font-mono font-semibold ${metric.color}`}>{metric.value}</span>
          </div>
        ))}
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-[1.35fr_0.95fr] gap-4">
        <div className="flex flex-col gap-4">
          <Card className="bg-card border-border shadow-none">
            <div className="bg-secondary/30 px-4 py-2.5 border-b border-border flex items-center justify-between">
              <span className="text-xs font-bold text-foreground uppercase tracking-wider flex items-center gap-2">
                <Activity className="w-4 h-4 text-primary" />
                {t("sections.recentFills")}
              </span>
              <Link href={`/${locale}/app/orders`} className="text-[11px] text-primary hover:underline">
                {pickText(lang, "查看历史", "View history")}
              </Link>
            </div>
            <div className="overflow-x-auto">
              <table className="w-full text-left text-sm">
                <thead className="bg-muted text-muted-foreground text-[10px] uppercase tracking-wider">
                  <tr>
                    <th className="px-4 py-2 font-medium">{pickText(lang, "交易对", "Pair")}</th>
                    <th className="px-4 py-2 font-medium text-right">{pickText(lang, "收益", "PnL")}</th>
                    <th className="px-4 py-2 font-medium text-right">{pickText(lang, "状态", "Status")}</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-slate-800/50">
                  {(analytics?.fills ?? []).slice(0, 10).map((fill, index) => {
                    const pnl = Number.parseFloat(fill.net_pnl || fill.realized_pnl || "0");
                    const isPositive = pnl >= 0;
                    return (
                      <tr key={index} className="hover:bg-secondary/30 transition-colors">
                        <td className="px-4 py-2.5 font-mono text-xs text-foreground font-semibold">{fill.symbol}</td>
                        <td className={`px-4 py-2.5 text-right font-mono text-xs font-bold ${isPositive ? "text-emerald-500" : "text-red-500"}`}>
                          {isPositive ? "+" : ""}{pnl.toFixed(4)}
                        </td>
                        <td className="px-4 py-2.5 text-right">
                          <span className={`px-1.5 py-0.5 rounded-sm text-[10px] font-bold ${isPositive ? "bg-emerald-500/10 text-emerald-500" : "bg-red-500/10 text-red-500"}`}>
                            {isPositive ? pickText(lang, "已平仓", "Closed") : pickText(lang, "追踪止盈中", "Trailing")}
                          </span>
                        </td>
                      </tr>
                    );
                  })}
                  {(!analytics?.fills || analytics.fills.length === 0) && (
                    <tr>
                      <td colSpan={3} className="px-4 py-8 text-center text-xs text-muted-foreground">
                        {pickText(lang, "暂时还没有最近成交，先创建机器人开始运行。", "No recent deals yet. Start a bot to see activity.")}
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>
          </Card>

          <Card className="bg-card border-border shadow-none">
            <div className="bg-secondary/30 px-4 py-2.5 border-b border-border flex items-center justify-between">
              <span className="text-xs font-bold text-foreground uppercase tracking-wider flex items-center gap-2">
                <History className="w-4 h-4 text-primary" />
                {pickText(lang, "近期账户活动", "Recent account activity")}
              </span>
            </div>
            <div className="overflow-x-auto">
              <table className="w-full text-left text-sm">
                <thead className="bg-muted text-muted-foreground text-[10px] uppercase tracking-wider">
                  <tr>
                    <th className="px-4 py-2 font-medium">{pickText(lang, "时间", "Captured At")}</th>
                    <th className="px-4 py-2 font-medium">{pickText(lang, "账户", "Account")}</th>
                    <th className="px-4 py-2 font-medium text-right">{pickText(lang, "资金费", "Funding")}</th>
                    <th className="px-4 py-2 font-medium text-right">{pickText(lang, "手续费", "Fees")}</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-slate-800/50">
                  {(analytics?.account_snapshots ?? []).slice(0, 6).map((snapshot, index) => (
                    <tr key={`${snapshot.exchange}-${snapshot.captured_at}-${index}`} className="hover:bg-secondary/30 transition-colors">
                      <td className="px-4 py-2.5 text-xs text-foreground">{formatTaipeiDateTime(snapshot.captured_at, lang)}</td>
                      <td className="px-4 py-2.5 text-xs text-foreground">{snapshot.exchange}</td>
                      <td className="px-4 py-2.5 text-right font-mono text-xs text-foreground">{snapshot.funding_total}</td>
                      <td className="px-4 py-2.5 text-right font-mono text-xs text-foreground">{snapshot.fees_paid}</td>
                    </tr>
                  ))}
                  {(!analytics?.account_snapshots || analytics.account_snapshots.length === 0) && (
                    <tr>
                      <td colSpan={4} className="px-4 py-8 text-center text-xs text-muted-foreground">
                        {pickText(lang, "当前还没有账户活动快照。", "No account activity snapshots yet.")}
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>
          </Card>
        </div>

        <div className="flex flex-col gap-4">
          <Card className="bg-card border-border shadow-none">
            <div className="bg-secondary/30 px-4 py-2.5 border-b border-border">
              <span className="text-xs font-bold text-foreground uppercase tracking-wider flex items-center gap-2">
                <Wallet className="w-4 h-4 text-emerald-500" />
                {pickText(lang, "资产分布", "Asset Allocation")}
              </span>
            </div>
            <div className="p-4 space-y-4">
              {assetAllocation.length > 0 ? (
                <>
                  <div className="rounded-2xl border border-border/60 bg-background/40 p-3" data-asset-allocation-chart="true">
                    <AssetAllocationChart items={assetAllocation.slice(0, 6)} lang={lang} />
                  </div>
                  <div className="space-y-2">
                    {assetAllocation.slice(0, 5).map((item) => (
                      <div key={item.asset} className="flex items-center justify-between gap-3 text-xs">
                        <div className="flex items-center gap-2">
                          <span className="h-2.5 w-2.5 rounded-full" style={{ backgroundColor: item.color }} />
                          <span className="text-foreground font-medium">{item.asset}</span>
                        </div>
                        <div className="text-right">
                          <div className="font-mono text-foreground">{item.balance}</div>
                          <div className="text-[11px] text-muted-foreground">{item.share}%</div>
                        </div>
                      </div>
                    ))}
                  </div>
                  <p className="text-[11px] text-muted-foreground">
                    {pickText(lang, "按最新账户快照中的余额字段汇总展示，时间统一按 ", "This chart summarizes the latest wallet snapshot balances. Time zone: ") + DISPLAY_TIME_ZONE}
                  </p>
                </>
              ) : (
                <p className="text-xs text-muted-foreground text-center py-2">{pickText(lang, "暂无余额数据", "No balance data")}</p>
              )}
            </div>
          </Card>

          <Card className="bg-card border-border shadow-none">
            <div className="bg-secondary/30 px-4 py-2.5 border-b border-border">
              <span className="text-xs font-bold text-foreground uppercase tracking-wider flex items-center gap-2">
                <AlertTriangle className="w-4 h-4 text-blue-500" />
                {pickText(lang, "会员与运行状态", "Membership & Runtime")}
              </span>
            </div>
            <div className="p-4 space-y-3">
              <div className="flex items-center justify-between text-xs">
                <span className="text-muted-foreground">{pickText(lang, "会员状态", "Membership Status")}</span>
                <span className="text-foreground font-semibold">{membershipStatus}</span>
              </div>
              <div className="flex items-center justify-between text-xs">
                <span className="text-muted-foreground">{pickText(lang, "宽限期截止", "Grace Until")}</span>
                <span className="text-foreground font-mono">{formatTaipeiDate(billing?.membership?.grace_until, lang)}</span>
              </div>
              <div className="flex items-center justify-between text-xs">
                <span className="text-muted-foreground">{pickText(lang, "运行中策略", "Running Bots")}</span>
                <span className="text-foreground font-mono">{String(runningCount)}</span>
              </div>
              <div className="flex items-center justify-between text-xs">
                <span className="text-muted-foreground">{pickText(lang, "异常阻塞", "ErrorPaused")}</span>
                <span className={errorPausedCount > 0 ? "text-red-500 font-semibold" : "text-foreground font-semibold"}>{String(errorPausedCount)}</span>
              </div>
              <div className="flex items-center justify-between text-xs">
                <span className="text-muted-foreground">{pickText(lang, "钱包资产数", "Wallet Assets")}</span>
                <span className="text-foreground font-mono">{String(analytics?.user.wallet_asset_count ?? 0)}</span>
              </div>
            </div>
          </Card>
        </div>
      </div>
    </div>
  );
}

async function fetchAnalytics(): Promise<AnalyticsReport | null> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (!sessionToken) return null;
  const response = await fetch(authApiBaseUrl() + "/analytics", {
    method: "GET",
    headers: { authorization: "Bearer " + sessionToken },
    cache: "no-store",
  });
  if (!response.ok) return null;
  return (await response.json()) as AnalyticsReport;
}

async function fetchStrategies(): Promise<StrategyListResponse["items"]> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (!sessionToken) return [];
  const response = await fetch(authApiBaseUrl() + "/strategies", {
    method: "GET",
    headers: { authorization: "Bearer " + sessionToken },
    cache: "no-store",
  });
  if (!response.ok) return [];
  return ((await response.json()) as StrategyListResponse).items;
}

async function fetchBillingOverview(): Promise<BillingOverview | null> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (!sessionToken) return null;
  const response = await fetch(authApiBaseUrl() + "/billing/overview", {
    method: "GET",
    headers: { authorization: "Bearer " + sessionToken },
    cache: "no-store",
  });
  if (!response.ok) return null;
  return (await response.json()) as BillingOverview;
}

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}


type AssetAllocationSlice = {
  asset: string;
  balance: string;
  color: string;
  share: string;
  value: number;
};

function buildAssetAllocation(wallets: AnalyticsReport["wallets"]): AssetAllocationSlice[] {
  const latest = new Map<string, AnalyticsReport["wallets"][number]>();
  for (const wallet of wallets) {
    const key = `${wallet.exchange}:${wallet.wallet_type}`;
    const current = latest.get(key);
    if (!current || current.captured_at < wallet.captured_at) {
      latest.set(key, wallet);
    }
  }

  const aggregated = new Map<string, number>();
  for (const wallet of latest.values()) {
    for (const [asset, amount] of Object.entries(wallet.balances)) {
      const numeric = Number.parseFloat(amount);
      if (!Number.isFinite(numeric) || numeric <= 0) {
        continue;
      }
      aggregated.set(asset, (aggregated.get(asset) ?? 0) + numeric);
    }
  }

  const total = Array.from(aggregated.values()).reduce((sum, value) => sum + value, 0);
  const palette = ["#22c55e", "#3b82f6", "#f59e0b", "#ef4444", "#8b5cf6", "#14b8a6", "#f97316"];
  return Array.from(aggregated.entries())
    .sort((a, b) => b[1] - a[1])
    .map(([asset, value], index) => ({
      asset,
      balance: value.toFixed(value >= 100 ? 2 : 4),
      color: palette[index % palette.length],
      share: total > 0 ? ((value / total) * 100).toFixed(1) : "0.0",
      value,
    }));
}

function AssetAllocationChart({ items, lang }: { items: AssetAllocationSlice[]; lang: UiLanguage }) {
  const total = items.reduce((sum, item) => sum + item.value, 0);
  let start = 0;
  const slices = items.map((item) => {
    const ratio = total > 0 ? item.value / total : 0;
    const slice = describeDonutSlice(56, 56, 38, start, start + ratio * Math.PI * 2);
    start += ratio * Math.PI * 2;
    return { ...item, ...slice };
  });

  return (
    <div className="flex items-center gap-4">
      <svg aria-label={pickText(lang, "资产分布饼图", "Asset allocation chart")} className="h-32 w-32 shrink-0" viewBox="0 0 112 112">
        <circle cx="56" cy="56" fill="none" r="38" stroke="currentColor" strokeOpacity="0.08" strokeWidth="22" />
        {slices.map((slice) => (
          <path key={slice.asset} d={slice.path} fill="none" stroke={slice.color} strokeLinecap="round" strokeWidth="22" />
        ))}
        <text className="fill-foreground" fontFamily="monospace" fontSize="12" textAnchor="middle" x="56" y="52">
          {pickText(lang, "资产", "Assets")}
        </text>
        <text className="fill-foreground" fontFamily="monospace" fontSize="14" fontWeight="700" textAnchor="middle" x="56" y="68">
          {String(items.length)}
        </text>
      </svg>
      <div className="space-y-2 text-xs text-muted-foreground">
        <p>{pickText(lang, "显示当前钱包中占比最高的资产。", "Shows the largest assets in the current wallet snapshot.")}</p>
        <p>{pickText(lang, "如果你同时开了现货和合约，这里会自动合并最新快照。", "Spot and futures snapshots are merged automatically when both are available.")}</p>
      </div>
    </div>
  );
}

function describeDonutSlice(cx: number, cy: number, radius: number, start: number, end: number) {
  const startX = cx + radius * Math.cos(start - Math.PI / 2);
  const startY = cy + radius * Math.sin(start - Math.PI / 2);
  const endX = cx + radius * Math.cos(end - Math.PI / 2);
  const endY = cy + radius * Math.sin(end - Math.PI / 2);
  const largeArc = end - start > Math.PI ? 1 : 0;
  return {
    path: `M ${startX} ${startY} A ${radius} ${radius} 0 ${largeArc} 1 ${endX} ${endY}`,
  };
}
