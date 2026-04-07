import { cookies } from "next/headers";

import { AppShellSection } from "@/components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { DataTable } from "@/components/ui/table";
import { StatusBanner } from "@/components/ui/status-banner";
import { UI_LANGUAGE_COOKIE, pickText, resolveUiLanguage, type UiLanguage } from "@/lib/ui/preferences";

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
  const cookieStore = await cookies();
  const lang = resolveUiLanguage(cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const analytics = await fetchAnalytics();

  return (
    <>
      <StatusBanner
        description={pickText(lang, "分析页直接展示后端报表，包括策略汇总、钱包快照与交易所成交。", "Analytics renders the backend report directly, including strategy totals, wallet snapshots, and exchange trades.")}
        title={pickText(lang, "分析状态条", "Analytics status strip")}
       
      />
      <AppShellSection
        description={pickText(lang, "账户级与策略级统计集中展示，便于收益与成本复盘。", "Account-level and strategy-level statistics stay together for PnL and cost review.")}
        eyebrow={pickText(lang, "分析", "Analytics")}
        title={pickText(lang, "分析面板", "Analytics")}
        actions={<div className="button-row"><a className="button button--ghost" href="/api/user/exports/strategy-stats">{pickText(lang, "导出策略统计 CSV", "Download strategy stats CSV")}</a><a className="button button--ghost" href="/api/user/exports/payments">{pickText(lang, "导出付款 CSV", "Download payments CSV")}</a></div>}
      >
        <div className="content-grid content-grid--metrics">
          {[
            [pickText(lang, "已实现收益", "Realized PnL"), analytics?.user.realized_pnl ?? "-"],
            [pickText(lang, "未实现收益", "Unrealized PnL"), analytics?.user.unrealized_pnl ?? "-"],
            [pickText(lang, "已付手续费", "Fees paid"), analytics?.user.fees_paid ?? "-"],
            [pickText(lang, "资金费合计", "Funding total"), analytics?.user.funding_total ?? "-"],
            [pickText(lang, "净收益", "Net PnL"), analytics?.user.net_pnl ?? "-"],
            [pickText(lang, "交易所成交数", "Exchange trades"), String(analytics?.user.exchange_trade_count ?? 0)],
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
            <CardTitle>{pickText(lang, "策略统计", "Strategy statistics")}</CardTitle>
            <CardDescription>{pickText(lang, "这里看每个策略的收益、费用和当前状态。", "Review realized, unrealized, fee, funding, and net totals per strategy.")}</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "strategy", label: pickText(lang, "策略", "Strategy") },
                { key: "symbol", label: pickText(lang, "交易对", "Symbol") },
                { key: "detail", label: pickText(lang, "明细", "Detail") },
                { key: "net", label: pickText(lang, "净值", "Net"), align: "right" },
              ]}
              rows={(analytics?.strategies ?? []).map((row) => ({
                id: row.strategy_id,
                strategy: row.strategy_id,
                symbol: row.symbol,
                detail: describeStrategyDetail(lang, row),
                net: row.net_pnl,
              }))}
            />
          </CardBody>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>{pickText(lang, "钱包快照", "Wallet snapshots")}</CardTitle>
            <CardDescription>{pickText(lang, "记录账户余额快照用于对账与回溯。", "Captured wallet state is preserved for reconciliation and audit.")}</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "exchange", label: pickText(lang, "交易所", "Exchange") },
                { key: "wallet", label: pickText(lang, "钱包", "Wallet") },
                { key: "balances", label: pickText(lang, "余额", "Balances"), align: "right" },
              ]}
              rows={(analytics?.wallets ?? []).map((row, index) => ({
                id: row.exchange + "-" + index,
                exchange: row.exchange,
                wallet: row.wallet_type,
                balances: Object.entries(row.balances).map(([asset, amount]) => asset + " " + amount).join(" | "),
              }))}
            />
          </CardBody>
        </Card>
      </div>
      <div className="content-grid content-grid--split">
        <Card>
          <CardHeader>
            <CardTitle>{pickText(lang, "最近交易所成交", "Recent exchange trades")}</CardTitle>
            <CardDescription>{pickText(lang, "用于核对真实成交、手续费和策略行为。", "Use this table to reconcile real exchange executions, fees, and strategy behavior.")}</CardDescription>
          </CardHeader>
          <CardBody>
            <DataTable
              columns={[
                { key: "at", label: pickText(lang, "时间", "Timestamp") },
                { key: "symbol", label: pickText(lang, "交易对", "Symbol") },
                { key: "detail", label: pickText(lang, "明细", "Detail") },
                { key: "fee", label: pickText(lang, "手续费", "Fee"), align: "right" },
              ]}
              rows={(analytics?.exchange_trades ?? []).map((row) => ({
                id: row.trade_id,
                at: row.traded_at.replace("T", " ").slice(0, 16),
                symbol: row.symbol,
                detail: row.exchange + " · " + describeSide(lang, row.side) + " · " + row.quantity + " @ " + row.price,
                fee: row.fee_amount ? (row.fee_amount + " " + (row.fee_asset ?? "")).trim() : "-",
              }))}
            />
          </CardBody>
        </Card>
        <Card>
          <CardHeader>
            <CardTitle>{pickText(lang, "成本摘要", "Cost summary")}</CardTitle>
            <CardDescription>{pickText(lang, "手续费和资金费分开保留，避免净值掩盖成本。", "Fees and funding stay separate so cost lines are not hidden inside net PnL.")}</CardDescription>
          </CardHeader>
          <CardBody>
            <ul className="text-list">
              <li>{pickText(lang, "手续费", "Fees paid")}: {analytics?.costs.fees_paid ?? "-"}</li>
              <li>{pickText(lang, "资金费", "Funding total")}: {analytics?.costs.funding_total ?? "-"}</li>
              <li>{pickText(lang, "钱包资产数", "Wallet asset count")}: {String(analytics?.user.wallet_asset_count ?? 0)}</li>
            </ul>
          </CardBody>
        </Card>
      </div>
    </>
  );
}

function describeStrategyDetail(lang: UiLanguage, row: AnalyticsReport["strategies"][number]) {
  return pickText(lang, "状态 ", "State ") + row.current_state + " · " + pickText(lang, "已实现 ", "Realized ") + row.realized_pnl + " · " + pickText(lang, "手续费 ", "Fees ") + row.fees_paid + " · " + pickText(lang, "资金费 ", "Funding ") + row.funding_total;
}

function describeSide(lang: UiLanguage, side: string) {
  return side === "Buy" ? pickText(lang, "买入", "Buy") : side === "Sell" ? pickText(lang, "卖出", "Sell") : side;
}

async function fetchAnalytics(): Promise<AnalyticsReport | null> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (sessionToken === "") {
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

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}
