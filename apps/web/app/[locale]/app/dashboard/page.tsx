import { cookies } from "next/headers";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";
import { formatTaipeiDateTime } from "@/lib/ui/time";
import { PnlTrendChart } from "@/components/ui/pnl-trend-chart";
import { StrategyHealthCards } from "@/components/ui/strategy-health-cards";
import { EmptyStateGuide } from "@/components/onboarding/empty-state-guide";
import { formatPnl, formatPercent } from "@/lib/ui/format";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

type AnalyticsResponse = {
  total_pnl: string;
  total_pnl_pct: string;
  fee_total: string;
  funding_total: string;
  strategy_health: { running: number; paused: number; error_paused: number; stopped: number; draft: number };
  pnl_trend: Array<{ date: string; pnl: number }>;
};

export default async function DashboardPage({
  params,
}: {
  params: Promise<{ locale: string }>;
}) {
  const { locale } = await params;
  const lang = (locale === "zh" ? "zh" : "en") as UiLanguage;
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  const analytics = sessionToken ? await fetchAnalytics(sessionToken) : null;
  const strategies = sessionToken ? await fetchStrategies() : null;
  const membership = sessionToken ? await fetchMembership(sessionToken) : null;
  const accountSnapshots = sessionToken ? await fetchAccountSnapshots(sessionToken) : null;
  const assetAllocation = sessionToken ? await fetchAssetAllocation(sessionToken) : null;
  const hasStrategies = analytics && (analytics.strategy_health.running + analytics.strategy_health.paused + analytics.strategy_health.error_paused + analytics.strategy_health.stopped + analytics.strategy_health.draft) > 0;
  const health = analytics
    ? { running: analytics.strategy_health.running, paused: analytics.strategy_health.paused, errorPaused: analytics.strategy_health.error_paused, stopped: analytics.strategy_health.stopped, draft: analytics.strategy_health.draft }
    : { running: 0, paused: 0, errorPaused: 0, stopped: 0, draft: 0 };
  const pnlTrend = analytics?.pnl_trend ?? [];

  return (
    <div className="space-y-6">
      <h1 className="text-2xl font-bold">
        {pickText(lang, "总览", "Dashboard")}
      </h1>

      {!hasStrategies ? (
        <EmptyStateGuide lang={lang} locale={locale} />
      ) : (
        <>
          <StrategyHealthCards health={health} lang={lang} />

          {pnlTrend.length > 0 && (
            <div className="rounded-lg border p-4">
              <h2 className="mb-3 text-sm font-medium text-muted-foreground">
                {pickText(lang, "收益趋势 (近30天)", "PnL Trend (30d)")}
              </h2>
              <PnlTrendChart data={pnlTrend} lang={lang} height={220} />
            </div>
          )}

          <div className="grid gap-4 sm:grid-cols-2 lg:grid-cols-4">
            <div className="rounded-lg border p-4">
              <p className="text-xs text-muted-foreground">{pickText(lang, "累计收益", "Total PnL")}</p>
              <p className="mt-1 text-lg font-semibold">{analytics ? formatPnl(Number(analytics.total_pnl)) : "—"}</p>
            </div>
            <div className="rounded-lg border p-4">
              <p className="text-xs text-muted-foreground">{pickText(lang, "收益率", "Return")}</p>
              <p className="mt-1 text-lg font-semibold">{analytics ? formatPercent(Number(analytics.total_pnl_pct)) : "—"}</p>
            </div>
            <div className="rounded-lg border p-4">
              <p className="text-xs text-muted-foreground">{pickText(lang, "手续费", "Fees")}</p>
              <p className="mt-1 text-lg font-semibold">{analytics ? formatPnl(Number(analytics.fee_total)) : "—"}</p>
            </div>
            <div className="rounded-lg border p-4">
              <p className="text-xs text-muted-foreground">{pickText(lang, "资金费", "Funding")}</p>
              <p className="mt-1 text-lg font-semibold">{analytics ? formatPnl(Number(analytics.funding_total)) : "—"}</p>
            </div>
          </div>
        </>
      )}

      <div className="rounded-lg border p-4">
        <h2 className="mb-3 text-sm font-medium text-muted-foreground">
          {pickText(lang, "会员状态", "Membership Status")}
        </h2>
        {membership ? (
          <dl className="grid gap-2 text-sm sm:grid-cols-2">
            <div>
              <dt className="text-muted-foreground">{pickText(lang, "套餐", "Plan")}</dt>
              <dd className="font-medium">{membership.plan}</dd>
            </div>
            <div>
              <dt className="text-muted-foreground">{pickText(lang, "到期时间", "Expires")}</dt>
              <dd>{membership.expires_at ?? pickText(lang, "永久", "Permanent")}</dd>
            </div>
          </dl>
        ) : (
          <p className="text-sm text-muted-foreground">{pickText(lang, "暂无会员信息", "No membership info")}</p>
        )}
      </div>

      <div className="rounded-lg border p-4">
        <h2 className="mb-3 text-sm font-medium text-muted-foreground">
          {pickText(lang, "近期账户活动", "Recent account activity")}
        </h2>
        {accountSnapshots && accountSnapshots.length > 0 ? (
          <ul className="space-y-2 text-sm">
            {accountSnapshots.slice(0, 5).map((snap) => (
              <li key={snap.id} className="flex items-center justify-between">
                <span>{snap.description}</span>
                <span className="text-xs text-muted-foreground">{formatTaipeiDateTime(snap.created_at, lang)}</span>
              </li>
            ))}
          </ul>
        ) : (
          <p className="text-sm text-muted-foreground">{pickText(lang, "暂无活动记录", "No recent activity")}</p>
        )}
      </div>

      <div className="rounded-lg border p-4" data-asset-allocation-chart>
        <h2 className="mb-3 text-sm font-medium text-muted-foreground">
          {pickText(lang, "资产分布", "Asset Allocation")}
        </h2>
        {assetAllocation && assetAllocation.length > 0 ? (
          <ul className="space-y-2 text-sm">
            {assetAllocation.map((item) => (
              <li key={item.symbol} className="flex items-center justify-between">
                <span className="font-mono font-medium">{item.symbol}</span>
                <span>{item.value} ({item.pct}%)</span>
              </li>
            ))}
          </ul>
        ) : (
          <p className="text-sm text-muted-foreground">{pickText(lang, "暂无资产数据", "No asset data")}</p>
        )}
      </div>

      <div className="grid gap-4 sm:grid-cols-2">
        <div className="rounded-lg border p-4">
          <h2 className="mb-2 text-sm font-medium text-muted-foreground">
            {pickText(lang, "快捷操作", "Quick Actions")}
          </h2>
          <div className="flex flex-wrap gap-2">
            <a
              href={`/${locale}/app/strategies/new`}
              className="inline-flex items-center rounded-md bg-primary px-3 py-1.5 text-sm font-medium text-primary-foreground hover:bg-primary/90"
            >
              {pickText(lang, "新建策略", "New Strategy")}
            </a>
            <a
              href={`/${locale}/app/exchange`}
              className="inline-flex items-center rounded-md border px-3 py-1.5 text-sm font-medium hover:bg-muted"
            >
              {pickText(lang, "交易所设置", "Exchange Setup")}
            </a>
            <a
              href={`/${locale}/app/billing`}
              className="inline-flex items-center rounded-md border px-3 py-1.5 text-sm font-medium hover:bg-muted"
            >
              {pickText(lang, "会员续费", "Renew Membership")}
            </a>
          </div>
        </div>
        <div className="rounded-lg border p-4">
          <h2 className="mb-2 text-sm font-medium text-muted-foreground">
            {pickText(lang, "新手引导", "Getting Started")}
          </h2>
          <ol className="space-y-2 text-sm">
            <li className="flex items-start gap-2">
              <span className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-primary text-xs text-primary-foreground">1</span>
              <span>{pickText(lang, "绑定交易所 API 密钥", "Connect exchange API key")}</span>
            </li>
            <li className="flex items-start gap-2">
              <span className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-primary text-xs text-primary-foreground">2</span>
              <span>{pickText(lang, "创建并启动网格策略", "Create and start a grid strategy")}</span>
            </li>
            <li className="flex items-start gap-2">
              <span className="flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-primary text-xs text-primary-foreground">3</span>
              <span>{pickText(lang, "监控收益与策略状态", "Monitor PnL and strategy health")}</span>
            </li>
          </ol>
        </div>
      </div>
    </div>
  );
}

async function fetchStrategies(): Promise<Array<{ id: string; name: string; status: string }> | null> {
  try {
    const response = await fetch(authApiBaseUrl() + "/strategies", {
      method: "GET",
      headers: { authorization: "Bearer " + "" },
      cache: "no-store",
    });
    if (!response.ok) return null;
    return (await response.json()) as Array<{ id: string; name: string; status: string }>;
  } catch {
    return null;
  }
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

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}
