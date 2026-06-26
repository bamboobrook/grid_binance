"use client";

import { useState } from "react";

import { requestBacktestApi } from "@/components/backtest/request-client";
import { Button } from "@/components/ui/form";
import { StatusBanner } from "@/components/ui/status-banner";
import { DataTable, type DataTableRow } from "@/components/ui/table";
import type { ExchangePreconfigureResponse } from "@/lib/api-types";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";

type Props = {
  portfolioId: string;
  lang: UiLanguage;
  disabled?: boolean;
};

export function ExchangePreconfigurePanel({ portfolioId, lang, disabled }: Props) {
  const [result, setResult] = useState<ExchangePreconfigureResponse | null>(null);
  const [pending, setPending] = useState("");
  const [feedback, setFeedback] = useState("");
  const [confirmHedge, setConfirmHedge] = useState(false);
  const [confirmMultiAssets, setConfirmMultiAssets] = useState(false);
  const [confirmOrders, setConfirmOrders] = useState(false);
  const [confirmSymbols, setConfirmSymbols] = useState(false);

  async function check() {
    setPending("check");
    setFeedback(pickText(lang, "正在读取 Binance Futures 配置…", "Reading Binance Futures settings..."));
    try {
      const response = await requestBacktestApi(`/api/user/martingale-portfolios/${portfolioId}/exchange-preflight`, { cache: "no-store" });
      if (!response.ok) {
        setFeedback(response.message);
        return;
      }
      setResult(response.data as ExchangePreconfigureResponse);
      setFeedback(pickText(lang, "检查完成，请确认目标与当前状态。", "Check complete. Confirm target and current state."));
    } finally {
      setPending("");
    }
  }

  async function configure() {
    setPending("configure");
    setFeedback(pickText(lang, "正在配置 Hedge Mode / 逐仓 / 杠杆…", "Configuring Hedge Mode / margin type / leverage..."));
    try {
      const response = await requestBacktestApi(`/api/user/martingale-portfolios/${portfolioId}/exchange-preconfigure`, {
        method: "POST",
        headers: { "content-type": "application/json" },
        body: JSON.stringify({
          confirm_account_level_hedge_mode_change: confirmHedge,
          confirm_account_level_multi_assets_mode_change: confirmMultiAssets,
          confirm_no_auto_orders: confirmOrders,
          confirm_symbol_margin_leverage_change: confirmSymbols,
        }),
      });
      if (!response.ok) {
        setFeedback(response.message);
        return;
      }
      setResult(response.data as ExchangePreconfigureResponse);
      setFeedback(pickText(lang, "预配置完成，启动前仍需最终确认。", "Preconfigure complete. Final start confirmation is still required."));
    } finally {
      setPending("");
    }
  }

  const rows: DataTableRow[] = result?.symbols.map((symbol) => ({
    id: symbol.symbol,
    symbol: symbol.symbol,
    margin: `${symbol.current_margin_mode ?? "?"} → ${symbol.target_margin_mode}`,
    leverage: `${symbol.current_leverage ?? "?"}x → ${symbol.target_leverage}x`,
    status: symbol.status,
    message: symbol.message,
  })) ?? [];
  const needsHedgeConfirmation = Boolean(result?.hedge_mode.target);
  const needsMultiAssetsConfirmation = result?.multi_assets_mode.target === false;
  const canConfigure =
    confirmOrders
    && confirmSymbols
    && (!needsHedgeConfirmation || confirmHedge)
    && (!needsMultiAssetsConfirmation || confirmMultiAssets);

  return (
    <div className="space-y-4">
      <div className="grid gap-3 md:grid-cols-4">
        <MiniMetric label={pickText(lang, "预配置状态", "Preconfigure status")} value={result?.status ?? "-"} />
        <MiniMetric label={pickText(lang, "Hedge Mode", "Hedge Mode")} value={hedgeModeText(result, lang)} />
        <MiniMetric label={pickText(lang, "Multi-Assets", "Multi-Assets")} value={multiAssetsModeText(result, lang)} />
        <MiniMetric label={pickText(lang, "检查时间", "Checked at")} value={formatDateTime(result?.checked_at, lang)} />
      </div>

      {result?.warnings?.length ? (
        <StatusBanner
          description={result.warnings.join("\n")}
          lang={lang}
          title={pickText(lang, "交易所配置风险确认", "Exchange setting risk confirmation")}
          tone={result.status === "ready" ? "info" : "warning"}
        />
      ) : null}

      <div className="flex flex-wrap gap-2">
        <Button aria-busy={pending === "check"} disabled={disabled || pending !== ""} onClick={() => void check()} size="sm" tone="outline" type="button">
          {pickText(lang, "读取当前配置", "Read current settings")}
        </Button>
        <Button aria-busy={pending === "configure"} disabled={disabled || pending !== "" || !canConfigure} onClick={() => void configure()} size="sm" type="button">
          {pickText(lang, "自动设置逐仓/杠杆", "Apply margin/leverage")}
        </Button>
      </div>

      <div className="grid gap-2 text-sm text-muted-foreground">
        <label className="flex items-start gap-2">
          <input checked={confirmHedge} className="mt-1" onChange={(event) => setConfirmHedge(event.target.checked)} type="checkbox" />
          <span>{pickText(lang, "我确认 Hedge Mode 是账户级设置，可能影响同一 Binance USDT-M 账户上的其他策略。", "I confirm Hedge Mode is account-level and may affect other USDT-M strategies on this Binance account.")}</span>
        </label>
        <label className="flex items-start gap-2">
          <input checked={confirmMultiAssets} className="mt-1" onChange={(event) => setConfirmMultiAssets(event.target.checked)} type="checkbox" />
          <span>{pickText(lang, "我确认 Multi-Assets mode 是账户级设置；为了使用逐仓，系统会关闭它，可能影响同一 Binance USDT-M 账户上的其他策略。", "I confirm Multi-Assets mode is account-level; to use isolated margin, the system will disable it, which may affect other USDT-M strategies on this Binance account.")}</span>
        </label>
        <label className="flex items-start gap-2">
          <input checked={confirmOrders} className="mt-1" onChange={(event) => setConfirmOrders(event.target.checked)} type="checkbox" />
          <span>{pickText(lang, "我确认此操作不会下单、不会撤单、不会平仓，只修改交易所配置。", "I confirm this action places no orders, cancels no orders, and closes no positions; it only changes exchange settings.")}</span>
        </label>
        <label className="flex items-start gap-2">
          <input checked={confirmSymbols} className="mt-1" onChange={(event) => setConfirmSymbols(event.target.checked)} type="checkbox" />
          <span>{pickText(lang, "我确认系统会按组合成员设置每个交易对的逐仓/全仓与杠杆。", "I confirm the system will set margin mode and leverage per portfolio symbol.")}</span>
        </label>
      </div>

      <DataTable
        columns={[
          { key: "symbol", label: "Symbol" },
          { key: "margin", label: pickText(lang, "当前→目标保证金模式", "Current→target margin") },
          { key: "leverage", label: pickText(lang, "当前→目标杠杆", "Current→target leverage") },
          { key: "status", label: pickText(lang, "状态", "Status") },
          { key: "message", label: pickText(lang, "说明", "Message") },
        ]}
        emptyMessage={pickText(lang, "先点击读取当前配置。", "Click read current settings first.")}
        rows={rows}
      />
      {feedback ? <p aria-live="polite" className="text-xs text-muted-foreground" role="status">{feedback}</p> : null}
    </div>
  );
}

function MiniMetric({ label, value }: { label: string; value: string }) {
  return (
    <div className="rounded-sm border border-border/70 bg-muted/20 p-3">
      <div className="text-xs text-muted-foreground">{label}</div>
      <div className="mt-1 text-sm font-semibold text-foreground">{value}</div>
    </div>
  );
}

function hedgeModeText(result: ExchangePreconfigureResponse | null, lang: UiLanguage) {
  if (!result) {
    return "-";
  }
  const current = result.hedge_mode.current == null ? "?" : result.hedge_mode.current ? "on" : "off";
  const target = result.hedge_mode.target ? "on" : "off";
  return pickText(lang, `当前 ${current} → 目标 ${target}`, `current ${current} → target ${target}`);
}

function multiAssetsModeText(result: ExchangePreconfigureResponse | null, lang: UiLanguage) {
  if (!result) {
    return "-";
  }
  const current = result.multi_assets_mode.current == null ? "?" : result.multi_assets_mode.current ? "on" : "off";
  const target = result.multi_assets_mode.target ? "on" : "off";
  return pickText(lang, `当前 ${current} → 目标 ${target}`, `current ${current} → target ${target}`);
}

function formatDateTime(value: string | undefined, lang: UiLanguage) {
  if (!value) return "-";
  const date = new Date(value);
  if (Number.isNaN(date.getTime())) return value;
  return new Intl.DateTimeFormat(lang === "zh" ? "zh-CN" : "en-US", { dateStyle: "medium", timeStyle: "short" }).format(date);
}
