import Link from "next/link";
import { notFound } from "next/navigation";
import { cookies } from "next/headers";

import { AppShellSection } from "@/components/shell/app-shell-section";
import { Card, CardBody } from "@/components/ui/card";
import { DataTable, type DataTableColumn, type DataTableRow } from "@/components/ui/table";
import { UI_LANGUAGE_COOKIE, pickText, resolveUiLanguageFromRoute, type UiLanguage } from "@/lib/ui/preferences";

import { getOrdersData } from "../order-data";
import { accountSnapshotColumns, accountSnapshotRows, exchangeTradeColumns, exchangeTradeRows, fillColumns, fillRows, orderColumns, orderRows } from "../order-tables";

type OrderDetailKind = "account-activity" | "exchange-trades" | "fills" | "strategy-orders";

type PageProps = {
  params: Promise<{
    kind: string;
    locale: string;
  }>;
};

export default async function OrdersDetailPage({ params }: PageProps) {
  const { kind, locale } = await params;
  const detailKind = normalizeKind(kind);
  if (!detailKind) {
    notFound();
  }

  const cookieStore = await cookies();
  const lang = resolveUiLanguageFromRoute(locale, cookieStore.get(UI_LANGUAGE_COOKIE)?.value);
  const data = await getOrdersData(lang);
  const table = detailTable(detailKind, lang, data);

  return (
    <AppShellSection
      actions={
        <Link className="inline-flex h-9 items-center justify-center rounded-sm px-4 py-2 text-sm font-medium text-foreground transition-colors hover:bg-secondary" href={`/${locale}/app/orders`}>
          {pickText(lang, "返回订单概览", "Back to orders")}
        </Link>
      }
      description={table.description}
      eyebrow={pickText(lang, "订单明细", "Order detail")}
      title={table.title}
    >
      <Card>
        <CardBody className="pt-4">
          <DataTable columns={table.columns} rows={table.rows} />
        </CardBody>
      </Card>
    </AppShellSection>
  );
}

function normalizeKind(kind: string): OrderDetailKind | null {
  if (kind === "account-activity" || kind === "exchange-trades" || kind === "fills" || kind === "strategy-orders") {
    return kind;
  }
  return null;
}

function detailTable(kind: OrderDetailKind, lang: UiLanguage, data: Awaited<ReturnType<typeof getOrdersData>>): {
  columns: DataTableColumn[];
  description: string;
  rows: DataTableRow[];
  title: string;
} {
  switch (kind) {
    case "strategy-orders":
      return {
        columns: orderColumns(lang),
        description: pickText(lang, "查看所有机器人当前挂单。", "View all current working bot orders."),
        rows: orderRows(lang, data.orderRows),
        title: pickText(lang, "全部策略挂单", "All strategy orders"),
      };
    case "fills":
      return {
        columns: fillColumns(lang),
        description: pickText(lang, "查看所有已成交记录。", "View all filled order records."),
        rows: fillRows(data.fillRows),
        title: pickText(lang, "全部成交历史", "All fill history"),
      };
    case "exchange-trades":
      return {
        columns: exchangeTradeColumns(lang),
        description: pickText(lang, "查看交易所侧返回的成交记录。", "View exchange-side trade records."),
        rows: exchangeTradeRows(lang, data.exchangeTrades),
        title: pickText(lang, "全部交易所成交", "All exchange trades"),
      };
    case "account-activity":
      return {
        columns: accountSnapshotColumns(lang),
        description: pickText(lang, "查看账户级费用和资金费快照。", "View account-level fee and funding snapshots."),
        rows: accountSnapshotRows(lang, data.accountSnapshots),
        title: pickText(lang, "全部账户活动快照", "All account activity snapshots"),
      };
  }
}
