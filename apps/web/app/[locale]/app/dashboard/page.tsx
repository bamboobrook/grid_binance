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
    <div className="mx-auto flex h-full max-w-[1600px] flex-col space-y-6 text-foreground">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h1 className="text-2xl font-black tracking-tight text-foreground">{t("title")}</h1>
        </div>
        <div className="flex flex-wrap items-center gap-3">
          <Button className="h-9 rounded-lg border border-border bg-secondary px-4 text-xs font-semibold text-foreground transition-colors hover:bg-accent/60">
            <History className="mr-2 h-4 w-4 text-muted-foreground" />
            {pickText(lang, "最近24小时", "Last 24h")}
          </Button>
          <Link href={`/${locale}/app/strategies/new`}>
            <Button className="h-9 px-5 text-sm font-bold bg-primary hover:bg-primary/90 text-primary-foreground shadow-lg shadow-primary/20 rounded-lg transition-all">
              <Zap className="w-4 h-4 mr-2" />
              {pickText(lang, "新建机器人", "New Bot")}
            </Button>
          </Link>
        </div>
      </div>

      {/* Bento Grid: Metrics Overview */}
      <div className="grid grid-cols-2 md:grid-cols-4 lg:grid-cols-5 gap-4">
        <div className="group relative col-span-2 flex flex-col justify-between overflow-hidden rounded-2xl border border-border bg-gradient-to-br from-card to-secondary p-5">
          <div className="absolute -right-6 -top-6 w-32 h-32 bg-primary/10 rounded-full blur-3xl group-hover:bg-primary/20 transition-colors"></div>
          <span className="relative z-10 mb-2 text-xs font-bold uppercase tracking-wider text-muted-foreground">{t("metrics.netPnL")}</span>
          <span className={`text-4xl font-mono font-black relative z-10 ${Number.parseFloat(analytics?.user.net_pnl || "0") >= 0 ? "text-emerald-400" : "text-red-400"}`}>
            {Number.parseFloat(analytics?.user.net_pnl || "0") >= 0 ? "+" : ""}{analytics?.user.net_pnl ?? "0.00"}
          </span>
          <div className="relative z-10 mt-4 flex items-center gap-4 text-xs font-medium text-muted-foreground">
            <div className="flex items-center gap-1"><span className="w-2 h-2 rounded-full bg-emerald-500"></span> {t("metrics.runningBots")}: {runningCount}</div>
            <div className="flex items-center gap-1"><span className="w-2 h-2 rounded-full bg-red-500"></span> Error: {errorPausedCount}</div>
          </div>
        </div>

        <div className="flex flex-col justify-center rounded-2xl border border-border bg-card p-5 transition-colors hover:border-primary/30">
          <span className="mb-1 text-[11px] font-bold uppercase tracking-wider text-muted-foreground">{t("metrics.realizedPnL")}</span>
          <span className="text-xl font-mono font-bold text-emerald-400">+{analytics?.user.realized_pnl ?? "0.00"}</span>
        </div>
        <div className="flex flex-col justify-center rounded-2xl border border-border bg-card p-5 transition-colors hover:border-primary/30">
          <span className="mb-1 text-[11px] font-bold uppercase tracking-wider text-muted-foreground">{t("metrics.unrealizedPnL")}</span>
          <span className={`text-xl font-mono font-bold ${Number.parseFloat(analytics?.user.unrealized_pnl || "0") >= 0 ? "text-blue-400" : "text-amber-400"}`}>
            {Number.parseFloat(analytics?.user.unrealized_pnl || "0") >= 0 ? "+" : ""}{analytics?.user.unrealized_pnl ?? "0.00"}
          </span>
        </div>
        <div className="flex flex-col justify-center rounded-2xl border border-border bg-card p-5 transition-colors hover:border-primary/30">
          <span className="mb-1 text-[11px] font-bold uppercase tracking-wider text-muted-foreground">{pickText(lang, "资金费", "Funding")}</span>
          <span className="text-xl font-mono font-bold text-foreground">{analytics?.user.funding_total ?? "0.00"}</span>
        </div>
      </div>

      {/* Bento Grid: Main Content */}
      <div className="grid grid-cols-1 lg:grid-cols-[1fr_400px] gap-6">
        <div className="flex flex-col gap-6">
          <Card className="overflow-hidden rounded-2xl border-border bg-card shadow-none">
            <div className="flex items-center justify-between border-b border-border bg-secondary/60 px-5 py-3.5">
              <span className="flex items-center gap-2 text-xs font-bold uppercase tracking-wider text-foreground">
                <Activity className="w-4 h-4 text-primary" />
                {t("sections.recentFills")}
              </span>
              <Link href={`/${locale}/app/orders`} className="text-[11px] font-semibold text-primary hover:text-primary/80 transition-colors">
                {pickText(lang, "查看历史", "View history")}
              </Link>
            </div>
            <div className="overflow-x-auto">
              <table className="w-full text-left text-sm">
                <thead className="bg-secondary/80 text-[10px] uppercase tracking-wider text-muted-foreground">
                  <tr>
                    <th className="px-5 py-3 font-semibold">{pickText(lang, "交易对", "Pair")}</th>
                    <th className="px-5 py-3 font-semibold text-right">{pickText(lang, "收益", "PnL")}</th>
                    <th className="px-5 py-3 font-semibold text-right">{pickText(lang, "状态", "Status")}</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-slate-800/50">
                  {(analytics?.fills ?? []).slice(0, 10).map((fill, index) => {
                    const pnl = Number.parseFloat(fill.net_pnl || fill.realized_pnl || "0");
                    const isPositive = pnl >= 0;
                    return (
                      <tr key={index} className="transition-colors hover:bg-secondary/50">
                        <td className="px-5 py-3.5 font-mono text-xs font-bold text-foreground">{fill.symbol}</td>
                        <td className={`px-5 py-3.5 text-right font-mono text-xs font-bold ${isPositive ? "text-emerald-400" : "text-red-400"}`}>
                          {isPositive ? "+" : ""}{pnl.toFixed(4)}
                        </td>
                        <td className="px-5 py-3.5 text-right">
                          <span className={`px-2 py-1 rounded text-[10px] font-bold ${isPositive ? "bg-emerald-500/10 text-emerald-400 border border-emerald-500/20" : "bg-red-500/10 text-red-400 border border-red-500/20"}`}>
                            {isPositive ? pickText(lang, "已平仓", "Closed") : pickText(lang, "追踪中", "Trailing")}
                          </span>
                        </td>
                      </tr>
                    );
                  })}
                  {(!analytics?.fills || analytics.fills.length === 0) && (
                    <tr>
                      <td colSpan={3} className="px-5 py-12 text-center text-xs text-muted-foreground">
                        {pickText(lang, "暂时还没有最近成交，先创建机器人开始运行。", "No recent deals yet. Start a bot to see activity.")}
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>
          </Card>

          <Card className="overflow-hidden rounded-2xl border-border bg-card shadow-none">
            <div className="flex items-center justify-between border-b border-border bg-secondary/60 px-5 py-3.5">
              <span className="flex items-center gap-2 text-xs font-bold uppercase tracking-wider text-foreground">
                <History className="w-4 h-4 text-primary" />
                {pickText(lang, "近期账户活动", "Recent account activity")}
              </span>
            </div>
            <div className="overflow-x-auto">
              <table className="w-full text-left text-sm">
                <thead className="bg-secondary/80 text-[10px] uppercase tracking-wider text-muted-foreground">
                  <tr>
                    <th className="px-5 py-3 font-semibold">{pickText(lang, "时间", "Captured At")}</th>
                    <th className="px-5 py-3 font-semibold">{pickText(lang, "账户", "Account")}</th>
                    <th className="px-5 py-3 font-semibold text-right">{pickText(lang, "资金费", "Funding")}</th>
                    <th className="px-5 py-3 font-semibold text-right">{pickText(lang, "手续费", "Fees")}</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-slate-800/50">
                  {(analytics?.account_snapshots ?? []).slice(0, 6).map((snapshot, index) => (
                    <tr key={`${snapshot.exchange}-${snapshot.captured_at}-${index}`} className="transition-colors hover:bg-secondary/50">
                      <td className="px-5 py-3.5 text-xs text-foreground">{formatTaipeiDateTime(snapshot.captured_at, lang)}</td>
                      <td className="px-5 py-3.5 text-xs font-medium text-foreground">{snapshot.exchange}</td>
                      <td className="px-5 py-3.5 text-right font-mono text-xs text-foreground">{snapshot.funding_total}</td>
                      <td className="px-5 py-3.5 text-right font-mono text-xs text-muted-foreground">{snapshot.fees_paid}</td>
                    </tr>
                  ))}
                  {(!analytics?.account_snapshots || analytics.account_snapshots.length === 0) && (
                    <tr>
                      <td colSpan={4} className="px-5 py-12 text-center text-xs text-muted-foreground">
                        {pickText(lang, "当前还没有账户活动快照。", "No account activity snapshots yet.")}
                      </td>
                    </tr>
                  )}
                </tbody>
              </table>
            </div>
          </Card>
        </div>

        <div className="flex flex-col gap-6">
          <Card className="overflow-hidden rounded-2xl border-border bg-card shadow-none">
            <div className="border-b border-border bg-secondary/60 px-5 py-3.5">
              <span className="flex items-center gap-2 text-xs font-bold uppercase tracking-wider text-foreground">
                <Wallet className="w-4 h-4 text-emerald-400" />
                {pickText(lang, "资产分布", "Asset Allocation")}
              </span>
            </div>
            <div className="p-5 space-y-5">
              {assetAllocation.length > 0 ? (
                <>
                  <div className="flex justify-center rounded-xl border border-border bg-secondary/70 p-4" data-asset-allocation-chart="true">
                    <AssetAllocationChart items={assetAllocation.slice(0, 6)} lang={lang} />
                  </div>
                  <div className="space-y-3">
                    {assetAllocation.slice(0, 5).map((item) => (
                      <div key={item.asset} className="flex items-center justify-between gap-3 text-xs">
                        <div className="flex items-center gap-2.5">
                          <span className="h-3 w-3 rounded-sm shadow-sm" style={{ backgroundColor: item.color }} />
                          <span className="text-foreground font-bold">{item.asset}</span>
                        </div>
                        <div className="text-right">
                          <div className="font-mono font-bold text-foreground">{item.balance}</div>
                          <div className="text-[10px] font-medium text-muted-foreground">{item.share}%</div>
                        </div>
                      </div>
                    ))}
                  </div>
                  <p className="border-t border-border pt-2 text-[10px] text-muted-foreground">
                    {pickText(lang, "按最新快照合并计算 (", "Merged from latest snapshots (") + DISPLAY_TIME_ZONE + ")"}
                  </p>
                </>
              ) : (
                <p className="py-8 text-center text-xs text-muted-foreground">{pickText(lang, "暂无余额数据", "No balance data")}</p>
              )}
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
