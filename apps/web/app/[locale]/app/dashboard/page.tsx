import Link from "next/link";
import { cookies } from "next/headers";
import { getTranslations } from "next-intl/server";
import {
  Activity,
  History,
  ShieldCheck,
  Wallet,
  Zap,
} from "lucide-react";

import { Button } from "@/components/ui/form";
import { Card } from "@/components/ui/card";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";

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

export default async function DashboardPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const lang: UiLanguage = locale === "en" ? "en" : "zh";
  const t = await getTranslations({ locale, namespace: "dashboard" });

  const [analytics, strategies] = await Promise.all([fetchAnalytics(), fetchStrategies()]);
  const runningCount = strategies.filter((item) => item.status === "Running").length;

  const metrics = [
    { label: t("metrics.realizedPnL"), value: analytics?.user.realized_pnl ?? "0.00", color: "text-emerald-500" },
    { label: t("metrics.unrealizedPnL"), value: analytics?.user.unrealized_pnl ?? "0.00", color: "text-blue-500" },
    { label: t("metrics.netPnL"), value: analytics?.user.net_pnl ?? "0.00", color: "text-amber-500" },
    { label: t("metrics.runningBots"), value: String(runningCount), color: "text-emerald-500" },
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

      <div className="grid grid-cols-2 gap-4 md:grid-cols-4">
        {metrics.map((metric) => (
          <div key={metric.label} className="bg-card border border-border/60 rounded-xl p-4 flex flex-col justify-center">
            <span className="text-[11px] font-bold text-muted-foreground uppercase tracking-wider mb-1">{metric.label}</span>
            <span className={`text-xl font-mono font-semibold ${metric.color}`}>{metric.value}</span>
          </div>
        ))}
      </div>

      <div className="grid grid-cols-1 lg:grid-cols-3 gap-4">
        <div className="lg:col-span-2 flex flex-col">
          <Card className="bg-card border-border shadow-none flex-1">
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
        </div>

        <div className="flex flex-col gap-4">
          <Card className="bg-card border-border shadow-none">
            <div className="bg-secondary/30 px-4 py-2.5 border-b border-border">
              <span className="text-xs font-bold text-foreground uppercase tracking-wider flex items-center gap-2">
                <Wallet className="w-4 h-4 text-emerald-500" />
                {t("sections.accountWatch")}
              </span>
            </div>
            <div className="p-4 space-y-4">
              <div className="flex items-center justify-between">
                <div className="flex items-center gap-2">
                  <div className="w-2 h-2 rounded-full bg-emerald-500" />
                  <span className="text-xs text-foreground font-semibold">{pickText(lang, "币安现货", "Binance Spot")}</span>
                </div>
                <span className="text-[10px] bg-secondary text-muted-foreground px-1.5 py-0.5 rounded-sm">{pickText(lang, "已连接", "Connected")}</span>
              </div>

              <div className="h-px bg-secondary" />

              <div className="space-y-2">
                {analytics?.wallets[0]
                  ? Object.entries(analytics.wallets[0].balances).slice(0, 5).map(([asset, amount]) => (
                      <div key={asset} className="flex items-center justify-between text-xs">
                        <span className="text-muted-foreground font-medium">{asset}</span>
                        <span className="font-mono text-foreground">{Number.parseFloat(amount).toFixed(4)}</span>
                      </div>
                    ))
                  : <p className="text-xs text-muted-foreground text-center py-2">{pickText(lang, "暂无余额数据", "No balance data")}</p>}
              </div>
            </div>
          </Card>

          <Card className="bg-card border-border shadow-none">
            <div className="bg-secondary/30 px-4 py-2.5 border-b border-border">
              <span className="text-xs font-bold text-foreground uppercase tracking-wider flex items-center gap-2">
                <ShieldCheck className="w-4 h-4 text-blue-500" />
                {pickText(lang, "系统状态", "System Status")}
              </span>
            </div>
            <div className="p-4 space-y-3">
              <div className="flex items-center justify-between text-xs">
                <span className="text-muted-foreground">{pickText(lang, "交易引擎", "Trading Engine")}</span>
                <span className="text-emerald-500 font-semibold">{pickText(lang, "运行正常", "Operational")}</span>
              </div>
              <div className="flex items-center justify-between text-xs">
                <span className="text-muted-foreground">{pickText(lang, "行情通道", "Market Data")}</span>
                <span className="text-emerald-500 font-semibold">{pickText(lang, "同步中", "Syncing")}</span>
              </div>
              <div className="flex items-center justify-between text-xs">
                <span className="text-muted-foreground">{pickText(lang, "延迟", "Latency")}</span>
                <span className="text-foreground font-mono">12ms</span>
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

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}
