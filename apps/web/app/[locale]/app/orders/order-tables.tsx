import { Chip } from "@/components/ui/chip";
import type { DataTableColumn, DataTableRow } from "@/components/ui/table";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";
import { formatTaipeiDateTime } from "@/lib/ui/time";

import {
  describeOrderState,
  describeSide,
  orderTone,
  type AccountSnapshotRow,
  type ExchangeTradeRow,
  type FillRow,
  type OrderRow,
} from "./order-data";

export function orderColumns(lang: UiLanguage): DataTableColumn[] {
  return [
    { key: "orderId", label: pickText(lang, "订单号", "Order ID") },
    { key: "strategy", label: pickText(lang, "策略", "Strategy") },
    { key: "detail", label: pickText(lang, "明细", "Detail") },
    { key: "state", label: pickText(lang, "状态", "State"), align: "right" },
  ];
}

export function fillColumns(lang: UiLanguage): DataTableColumn[] {
  return [
    { key: "event", label: pickText(lang, "事件", "Event") },
    { key: "symbol", label: pickText(lang, "交易对", "Symbol") },
    { key: "detail", label: pickText(lang, "明细", "Detail") },
    { key: "pnl", label: pickText(lang, "收益", "PnL"), align: "right" },
  ];
}

export function exchangeTradeColumns(lang: UiLanguage): DataTableColumn[] {
  return [
    { key: "at", label: pickText(lang, "时间", "Timestamp") },
    { key: "symbol", label: pickText(lang, "交易对", "Symbol") },
    { key: "detail", label: pickText(lang, "明细", "Detail") },
    { key: "fee", label: pickText(lang, "手续费", "Fee"), align: "right" },
  ];
}

export function accountSnapshotColumns(lang: UiLanguage): DataTableColumn[] {
  return [
    { key: "capturedAt", label: pickText(lang, "时间", "Timestamp") },
    { key: "exchange", label: pickText(lang, "交易所", "Exchange") },
    { key: "detail", label: pickText(lang, "明细", "Detail"), align: "right" },
  ];
}

export function orderRows(lang: UiLanguage, rows: OrderRow[]): DataTableRow[] {
  return rows.map((row) => ({
    detail: row.detail,
    id: row.id,
    orderId: row.orderId,
    state: <Chip tone={orderTone(row.state)}>{describeOrderState(lang, row.state)}</Chip>,
    strategy: row.strategy,
  }));
}

export function fillRows(rows: FillRow[]): DataTableRow[] {
  return rows.map((row) => ({
    detail: row.detail,
    event: row.event,
    id: row.id,
    pnl: row.pnl,
    symbol: row.symbol,
  }));
}

export function exchangeTradeRows(lang: UiLanguage, rows: ExchangeTradeRow[]): DataTableRow[] {
  return rows.map((row) => ({
    at: formatTaipeiDateTime(row.traded_at, lang),
    detail: row.exchange + " · " + describeSide(lang, row.side) + " · " + row.quantity + " @ " + row.price,
    fee: row.fee_amount ? (row.fee_amount + " " + (row.fee_asset ?? "")).trim() : "-",
    id: row.trade_id,
    symbol: row.symbol,
  }));
}

export function accountSnapshotRows(lang: UiLanguage, rows: AccountSnapshotRow[]): DataTableRow[] {
  return rows.map((row, index) => ({
    capturedAt: formatTaipeiDateTime(row.captured_at, lang),
    detail: pickText(lang, "手续费 " + row.fees_paid + " | 资金费 " + row.funding_total, "Fees " + row.fees_paid + " | Funding " + row.funding_total),
    exchange: row.exchange,
    id: row.exchange + "-" + index,
  }));
}
