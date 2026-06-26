import Link from "next/link";
import { cookies } from "next/headers";

import { AppShellSection } from "@/components/shell/app-shell-section";
import { Card, CardBody, CardDescription, CardHeader, CardTitle } from "@/components/ui/card";
import { DataTable, type DataTableColumn, type DataTableRow } from "@/components/ui/table";
import { UI_LANGUAGE_COOKIE, pickText, resolveUiLanguageFromRoute, type UiLanguage } from "@/lib/ui/preferences";

import { getOrdersData } from "./order-data";
import { accountSnapshotColumns, accountSnapshotRows, exchangeTradeColumns, exchangeTradeRows, fillColumns, fillRows, orderColumns, orderRows } from "./order-tables";

const PREVIEW_LIMIT = 5;

export default async function OrdersPage({ params }: { params: Promise<{ locale: string }> }) {
  const { locale } = await params;
  const cookieStore = await cookies();
  const lang = resolveUiLanguageFromRoute(locale, cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const data = await getOrdersData(lang);

  return (
    <>
      <AppShellSection
        actions={
          <a className="inline-flex h-9 items-center justify-center rounded-sm px-4 py-2 text-sm font-medium text-foreground transition-colors hover:bg-secondary" href="/api/user/exports/fills">
            {pickText(lang, "导出成交 CSV", "Download fills CSV")}
          </a>
        }
        description={pickText(lang, "这个页面用于核对挂单、成交与交易所侧执行，不离开用户工作台。", "Use this page to reconcile working orders, fills, and exchange executions without leaving the user shell.")}
        eyebrow={pickText(lang, "用户订单", "User orders")}
        title={pickText(lang, "订单与历史", "Orders & History")}
      >
        <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
          <OrdersSummaryCard
            columns={orderColumns(lang)}
            description={pickText(lang, "检查每个机器人现在还挂着哪些买入或卖出单。", "Check which buy or sell orders each bot is still working.")}
            href={`/${locale}/app/orders/strategy-orders`}
            lang={lang}
            rows={orderRows(lang, data.orderRows).slice(0, PREVIEW_LIMIT)}
            title={pickText(lang, "策略挂单", "Strategy orders")}
            total={data.orderRows.length}
          />
          <OrdersSummaryCard
            columns={fillColumns(lang)}
            description={pickText(lang, "看已经成交的订单、数量和单笔收益。", "Review filled orders, quantities, and per-fill PnL.")}
            href={`/${locale}/app/orders/fills`}
            lang={lang}
            rows={fillRows(data.fillRows).slice(0, PREVIEW_LIMIT)}
            title={pickText(lang, "成交历史", "Fill history")}
            total={data.fillRows.length}
          />
        </div>
      </AppShellSection>

      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        <OrdersSummaryCard
          columns={exchangeTradeColumns(lang)}
          description={pickText(lang, "和交易所侧成交记录对账。", "Reconcile against exchange-side fills.")}
          href={`/${locale}/app/orders/exchange-trades`}
          lang={lang}
          rows={exchangeTradeRows(lang, data.exchangeTrades).slice(0, PREVIEW_LIMIT)}
          title={pickText(lang, "最近交易所成交", "Recent exchange trades")}
          total={data.exchangeTrades.length}
        />
        <OrdersSummaryCard
          columns={accountSnapshotColumns(lang)}
          description={pickText(lang, "看手续费、资金费和同步状态的变化。", "Review fees, funding, and sync status changes.")}
          href={`/${locale}/app/orders/account-activity`}
          lang={lang}
          rows={accountSnapshotRows(lang, data.accountSnapshots).slice(0, PREVIEW_LIMIT)}
          title={pickText(lang, "账户活动快照", "Exchange account activity")}
          total={data.accountSnapshots.length}
        />
      </div>
    </>
  );
}

function OrdersSummaryCard({
  columns,
  description,
  href,
  lang,
  rows,
  title,
  total,
}: {
  columns: readonly DataTableColumn[];
  description: string;
  href: string;
  lang: UiLanguage;
  rows: DataTableRow[];
  title: string;
  total: number;
}) {
  return (
    <Card>
      <CardHeader className="flex-row items-start justify-between gap-3 space-y-0">
        <div className="min-w-0 space-y-1.5">
          <CardTitle>{title}</CardTitle>
          <CardDescription>{description}</CardDescription>
        </div>
        <Link className="shrink-0 rounded-sm px-2 py-1 text-xs font-semibold text-primary transition-colors hover:bg-primary/10" href={href}>
          {pickText(lang, "更多", "More")}
          {total > PREVIEW_LIMIT ? <span className="ml-1 text-muted-foreground">{total}</span> : null}
        </Link>
      </CardHeader>
      <CardBody>
        <DataTable columns={columns} rows={rows} />
      </CardBody>
    </Card>
  );
}
