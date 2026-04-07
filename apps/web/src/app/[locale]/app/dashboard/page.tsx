import Link from "next/link";
import { cookies } from "next/headers";
import { getTranslations } from "next-intl/server";
import { 
  TrendingUp, 
  TrendingDown, 
  Activity, 
  Zap, 
  ShieldCheck, 
  Wallet,
  ArrowUpRight,
  ExternalLink,
  History,
  AlertCircle
} from "lucide-react";

import { Card } from "../../../../components/ui/card";
import { Button } from "../../../../components/ui/form";
import { Chip } from "../../../../components/ui/chip";

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

export default async function DashboardPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const t = await getTranslations({ locale, namespace: 'dashboard' });
  const commonT = await getTranslations({ locale, namespace: 'common' });
  
  const results = await Promise.all([fetchAnalytics(), fetchStrategies(), fetchBillingOverview()]);
  const analytics = results[0];
  const strategies = results[1];
  const billing = results[2];

  const membership = billing?.membership ?? null;
  const runningCount = strategies.filter((item) => item.status === "Running").length;
  const pausedCount = strategies.filter((item) => item.status === "Paused").length;
  const blockedCount = strategies.filter((item) => item.status === "ErrorPaused").length;

  const metrics = [
    { label: t('metrics.realizedPnL'), value: analytics?.user.realized_pnl ?? "0.00", icon: TrendingUp, color: 'text-green-500' },
    { label: t('metrics.unrealizedPnL'), value: analytics?.user.unrealized_pnl ?? "0.00", icon: Activity, color: 'text-blue-500' },
    { label: t('metrics.netPnL'), value: analytics?.user.net_pnl ?? "0.00", icon: Zap, color: 'text-amber-500' },
    { label: t('metrics.runningBots'), value: String(runningCount), icon: ShieldCheck, color: 'text-green-500' },
  ];

  return (
    <div className="space-y-6">
      {/* Welcome Section */}
      <div className="flex flex-col md:flex-row md:items-center justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold tracking-tight">{t('title')}</h1>
          <p className="text-muted-foreground text-sm">{t('subtitle')}</p>
        </div>
        <div className="flex items-center gap-3">
          <Button tone="secondary" className="px-3 py-1 text-xs">
            <History className="w-4 h-4 mr-2" />
            Last 24h
          </Button>
          <Link href={`/${locale}/app/strategies/new`}>
            <Button className="bg-amber-500 hover:bg-amber-600 text-white border-none shadow-lg shadow-amber-500/20 px-4 py-1 text-xs">
              New Bot
            </Button>
          </Link>
        </div>
      </div>

      {/* Primary Metrics Grid */}
      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        {metrics.map((metric, i) => (
          <div key={i} className="bg-card border border-border rounded-2xl p-5 shadow-sm hover:shadow-md transition-shadow relative overflow-hidden group">
            <div className="flex flex-col gap-1 relative z-10">
              <span className="text-xs font-semibold text-muted-foreground uppercase tracking-widest">{metric.label}</span>
              <span className={`text-2xl font-bold tracking-tight ${metric.color}`}>
                {metric.value}
              </span>
            </div>
            <metric.icon className={`absolute -right-2 -bottom-2 w-16 h-16 opacity-5 group-hover:opacity-10 transition-opacity ${metric.color}`} />
          </div>
        ))}
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-6">
        {/* Recent Fills Table (Main Content) */}
        <div className="lg:col-span-2 space-y-6">
          <div className="bg-card border border-border rounded-2xl overflow-hidden shadow-sm">
            <div className="px-6 py-4 border-b border-border flex items-center justify-between">
              <h2 className="font-bold flex items-center gap-2">
                <Activity className="w-4 h-4 text-amber-500" />
                {t('sections.recentFills')}
              </h2>
              <Link href={`/${locale}/app/orders`} className="text-xs text-amber-500 hover:underline flex items-center gap-1">
                View All <ArrowUpRight className="w-3 h-3" />
              </Link>
            </div>
            <div className="overflow-x-auto">
              <table className="w-full text-sm text-left">
                <thead className="bg-muted/30 text-muted-foreground">
                  <tr>
                    <th className="px-6 py-3 font-medium uppercase text-[10px] tracking-wider">Symbol</th>
                    <th className="px-6 py-3 font-medium uppercase text-[10px] tracking-wider text-right">PnL</th>
                    <th className="px-6 py-3 font-medium uppercase text-[10px] tracking-wider text-right">Status</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-border">
                  {(analytics?.fills ?? []).slice(0, 8).map((fill, index) => {
                    const pnl = parseFloat(fill.net_pnl || fill.realized_pnl || "0");
                    const isPositive = pnl >= 0;
                    return (
                      <tr key={index} className="hover:bg-muted/20 transition-colors">
                        <td className="px-6 py-4 font-semibold">{fill.symbol}</td>
                        <td className={`px-6 py-4 text-right font-mono font-medium ${isPositive ? 'text-green-500' : 'text-red-500'}`}>
                          {isPositive ? '+' : ''}{pnl.toFixed(4)}
                        </td>
                        <td className="px-6 py-4 text-right">
                          <span className={`px-2 py-0.5 rounded-full text-[10px] font-bold border ${isPositive ? 'bg-green-500/10 text-green-500 border-green-500/20' : 'bg-red-500/10 text-red-500 border-red-500/20'}`}>
                            {isPositive ? 'SETTLED' : 'TRAILING'}
                          </span>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          </div>
        </div>

        {/* Side Panel (Account Watch & Next Actions) */}
        <div className="space-y-6">
          {/* Account Watch Card */}
          <div className="bg-card border border-border rounded-2xl p-6 shadow-sm space-y-4">
            <h2 className="font-bold flex items-center gap-2 text-sm">
              <Wallet className="w-4 h-4 text-blue-500" />
              {t('sections.accountWatch')}
            </h2>
            <div className="space-y-3">
              <div className="flex items-center justify-between text-xs">
                <span className="text-muted-foreground">Status</span>
                <span className="font-bold text-green-500 bg-green-500/10 px-2 py-0.5 rounded">ACTIVE</span>
              </div>
              <div className="flex items-center justify-between text-xs">
                <span className="text-muted-foreground">Exchange</span>
                <span className="font-semibold">Binance Spot</span>
              </div>
              <div className="h-[1px] bg-border my-1" />
              <div className="space-y-2">
                <p className="text-[10px] text-muted-foreground uppercase font-bold tracking-widest">Balances</p>
                {analytics?.wallets[0] ? Object.entries(analytics.wallets[0].balances).slice(0, 3).map(([asset, amount]) => (
                  <div key={asset} className="flex items-center justify-between text-xs">
                    <span className="font-medium">{asset}</span>
                    <span className="font-mono">{parseFloat(amount).toFixed(4)}</span>
                  </div>
                )) : <p className="text-xs italic text-muted-foreground">No data</p>}
              </div>
            </div>
          </div>

          {/* Next Actions */}
          <div className="bg-card border border-border rounded-2xl p-6 shadow-sm space-y-4">
            <h2 className="font-bold flex items-center gap-2 text-sm">
              <Zap className="w-4 h-4 text-amber-500" />
              {t('sections.nextActions')}
            </h2>
            <div className="space-y-2">
              <Link href={`/${locale}/app/exchange`} className="block group">
                <div className="p-3 bg-muted/30 rounded-xl border border-border group-hover:border-amber-500/50 transition-all">
                  <p className="text-xs font-bold mb-1 group-hover:text-amber-500">API Connection</p>
                  <p className="text-[10px] text-muted-foreground">Check your exchange API status and permissions.</p>
                </div>
              </Link>
              <Link href={`/${locale}/app/billing`} className="block group">
                <div className="p-3 bg-muted/30 rounded-xl border border-border group-hover:border-blue-500/50 transition-all">
                  <p className="text-xs font-bold mb-1 group-hover:text-blue-500">Renew Membership</p>
                  <p className="text-[10px] text-muted-foreground">Your subscription is active for 22 more days.</p>
                </div>
              </Link>
            </div>
          </div>
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
