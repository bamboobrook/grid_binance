import Link from "next/link";
import { cookies } from "next/headers";
import { getTranslations } from "next-intl/server";
import { Filter, Plus } from "lucide-react";

import { StrategyInventoryTable } from "@/components/strategies/strategy-inventory-table";
import { StopAllStrategiesForm } from "@/components/strategies/stop-all-strategies-form";
import { Card } from "@/components/ui/card";
import { Button } from "@/components/ui/form";
import { StatusBanner } from "@/components/ui/status-banner";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";

const DEFAULT_AUTH_API_BASE_URL = "http://127.0.0.1:8080";

type PageProps = {
  params: Promise<{ locale: string }>;
  searchParams?: Promise<{
    confirmAction?: string | string[];
    confirmDelete?: string | string[];
    confirmIds?: string | string[];
    count?: string | string[];
    error?: string | string[];
    notice?: string | string[];
    status?: string | string[];
    symbol?: string | string[];
    view?: string | string[];
  }>;
};

type StrategyPosition = {
  average_entry_price: string;
  quantity: string;
};

type RawStrategyListItem = {
  budget: string;
  id: string;
  market: string;
  name: string;
  status: string;
  symbol: string;
  draft_revision: { levels: Array<unknown> };
  active_revision?: { levels: Array<unknown> } | null;
  runtime: {
    fills: Array<unknown>;
    orders: Array<unknown>;
    positions: StrategyPosition[];
  };
};

type StrategyListItem = {
  avgEntryPrice: string;
  budget: string;
  fillCount: number;
  gridUtilization?: number;
  gridCount: number;
  gridPnl: string;
  id: string;
  market: string;
  name: string;
  overallPnl: string;
  overallPnlPct?: string;
  runtimeDuration?: string;
  status: string;
  symbol: string;
  todayPnl?: string;
  tradeCount: number;
};

type StrategyListResponse = {
  items: RawStrategyListItem[];
};

type StrategySummaries = Array<{
  average_entry_price: string;
  fill_count: number;
  net_pnl: string;
  order_count: number;
  realized_pnl: string;
  strategy_id: string;
}>;

export default async function StrategiesPage({ params, searchParams }: PageProps) {
  const { locale } = await params;
  const lang: UiLanguage = locale === "en" ? "en" : "zh";
  const t = await getTranslations({ locale, namespace: "strategies" });

  const searchParamsValue = (await searchParams) ?? {};
  const statusFilter = firstValue(searchParamsValue.status) ?? "all";
  const symbolFilter = firstValue(searchParamsValue.symbol) ?? "";
  const viewMode = firstValue(searchParamsValue.view) === "cards" ? "cards" : "table";
  const errorMessage = firstValue(searchParamsValue.error);
  const noticeMessage = strategyNotice(lang, firstValue(searchParamsValue.notice));
  const confirmAction = normalizeConfirmAction(firstValue(searchParamsValue.confirmAction));
  const confirmIds = parseConfirmIds(firstValue(searchParamsValue.confirmIds) ?? firstValue(searchParamsValue.confirmDelete));
  const confirmCount = parseConfirmCount(firstValue(searchParamsValue.count), confirmIds.length);
  const confirmCopy = confirmAction ? confirmDialogCopy(lang, confirmAction, confirmCount) : null;

  const previewMode = process.env.NEXT_PUBLIC_UI_PREVIEW === "1";
  const [strategyResult, summariesPayload] = await Promise.all([fetchStrategies(lang), fetchAnalytics(lang)]);
  const summaries = new Map((summariesPayload ?? []).map((item) => [item.strategy_id, item]));
  const strategies = strategyResult.items.length > 0
    ? strategyResult.items.map((item) => buildInventoryItem(item, summaries.get(item.id)))
    : previewMode
      ? previewStrategies()
      : [];
  const filteredStrategies = strategies.filter((item) => {
    const statusMatches = statusFilter === "all" || item.status === statusFilter;
    const query = symbolFilter.trim().toLowerCase();
    const symbolMatches = !query || item.symbol.toLowerCase().includes(query) || item.name.toLowerCase().includes(query);
    return statusMatches && symbolMatches;
  });

  return (
    <div className="mx-auto flex h-full max-w-[1760px] flex-col gap-4">
      <div className="flex flex-col gap-3 sm:flex-row sm:items-center sm:justify-between">
        <div>
          <h1 className="text-xl font-bold tracking-tight text-foreground">{pickText(lang, "我的机器人", "My Bots")}</h1>
          <p className="text-sm text-muted-foreground">
            {pickText(lang, "列表会直接展示网格数量、成交、均价和盈亏，处理策略时不必再频繁跳详情页。", "The list now keeps grid counts, fills, holding cost, and PnL visible so you can operate without constantly opening details.")}
          </p>
        </div>
        <div className="flex flex-wrap items-center gap-3">
          <StopAllStrategiesForm lang={lang} viewMode={viewMode} />
          <Link href={`/${locale}/app/strategies/new`}>
            <Button className="h-8 px-4 text-xs font-semibold">
              <Plus className="mr-1.5 h-3.5 w-3.5" />
              {pickText(lang, "新建机器人", "New Bot")}
            </Button>
          </Link>
        </div>
      </div>

      <form action={`/${locale}/app/strategies`} className="flex flex-wrap items-center gap-4 rounded-xl border border-border/60 bg-card p-3" method="get">
        <input name="view" type="hidden" value={viewMode} />
        <div className="flex max-w-[320px] flex-1 items-center gap-2 rounded-xl border border-border bg-input px-3 py-1.5 transition-colors focus-within:border-primary/50">
          <Filter className="h-4 w-4 text-muted-foreground" />
          <input
            className="w-full border-none bg-transparent text-xs text-foreground outline-none placeholder:text-muted-foreground"
            defaultValue={symbolFilter}
            name="symbol"
            placeholder={t("filter")}
            type="text"
          />
        </div>
        <select className="h-9 rounded-xl border border-border bg-input px-3 text-xs text-foreground" defaultValue={statusFilter} name="status">
          <option value="all">{pickText(lang, "全部状态", "All Status")}</option>
          <option value="Draft">{pickText(lang, "草稿", "Draft")}</option>
          <option value="Running">{pickText(lang, "运行中", "Running")}</option>
          <option value="Paused">{pickText(lang, "已暂停", "Paused")}</option>
          <option value="Stopping">{pickText(lang, "停止中", "Stopping")}</option>
          <option value="ErrorPaused">{pickText(lang, "异常阻塞", "Blocked")}</option>
          <option value="Stopped">{pickText(lang, "已停止", "Stopped")}</option>
        </select>
        <Button className="h-9 px-4 text-xs" type="submit">{pickText(lang, "应用筛选", "Apply Filters")}</Button>
      </form>

      {errorMessage ? <StatusBanner description={errorMessage} lang={lang} title={pickText(lang, "操作没有执行", "Action Not Run")} tone="warning" /> : null}
      {noticeMessage ? <StatusBanner description={noticeMessage} lang={lang} title={pickText(lang, "操作已模拟", "Preview Action")} tone="success" /> : null}
      <Card className="border-border bg-card shadow-none">
        <StrategyInventoryTable
          cardViewHref={viewHref(locale, statusFilter, symbolFilter, "cards")}
          items={filteredStrategies}
          lang={lang}
          locale={locale}
          previewMode={previewMode}
          tableViewHref={viewHref(locale, statusFilter, symbolFilter, "table")}
          viewMode={viewMode}
        />
      </Card>
      {confirmAction && confirmCopy ? (
        <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/55 px-4 py-6">
          <section
            aria-describedby="strategy-confirm-dialog-description"
            aria-labelledby="strategy-confirm-dialog-title"
            aria-modal="true"
            className="w-full max-w-md rounded-md border border-border bg-card p-5 text-card-foreground shadow-xl"
            role="dialog"
          >
            <h2 className="text-base font-bold text-foreground" id="strategy-confirm-dialog-title">
              {confirmCopy.title}
            </h2>
            <p className="mt-2 text-sm leading-relaxed text-muted-foreground" id="strategy-confirm-dialog-description">
              {confirmCopy.description}
            </p>
            <div className="mt-5 flex justify-end gap-2">
              <Link className="inline-flex h-8 items-center justify-center rounded-sm border border-border bg-transparent px-3 text-xs font-semibold text-foreground transition-colors hover:bg-accent hover:text-accent-foreground" href={viewHref(locale, statusFilter, symbolFilter, viewMode)}>
                {pickText(lang, "取消", "Cancel")}
              </Link>
              <form action="/api/user/strategies/batch" method="post">
                <input name="confirmAction" type="hidden" value="yes" />
                <input name="intent" type="hidden" value={confirmAction} />
                <input name="returnTo" type="hidden" value="list" />
                <input name="view" type="hidden" value={viewMode} />
                {confirmAction === "delete" ? <input name="confirmDelete" type="hidden" value="yes" /> : null}
                {confirmIds.map((id) => (
                  <input key={id} name="ids" type="hidden" value={id} />
                ))}
                <Button className={`h-8 px-3 text-xs font-semibold ${confirmCopy.buttonClassName}`} type="submit">
                  {confirmCopy.confirmLabel}
                </Button>
              </form>
            </div>
          </section>
        </div>
      ) : null}
    </div>
  );
}

function buildInventoryItem(
  strategy: RawStrategyListItem,
  summary: StrategySummaries[number] | undefined,
): StrategyListItem {
  return {
    avgEntryPrice: metricValue(summary?.average_entry_price) ?? fallbackAverageCost(strategy.runtime.positions),
    budget: strategy.budget,
    fillCount: summary?.fill_count ?? strategy.runtime.fills.length,
    gridCount: strategy.active_revision?.levels?.length ?? strategy.draft_revision.levels.length,
    gridPnl: metricValue(summary?.realized_pnl) ?? "0",
    id: strategy.id,
    market: strategy.market,
    name: strategy.name,
    overallPnl: metricValue(summary?.net_pnl) ?? "0",
    status: strategy.status,
    symbol: strategy.symbol,
    tradeCount: summary?.fill_count ?? strategy.runtime.fills.length,
  };
}

function fallbackAverageCost(positions: StrategyPosition[]) {
  if (positions.length === 0) {
    return "-";
  }
  let totalQuantity = 0;
  let weightedCost = 0;
  for (const position of positions) {
    const quantity = Number.parseFloat(position.quantity);
    const average = Number.parseFloat(position.average_entry_price);
    if (!Number.isFinite(quantity) || !Number.isFinite(average)) {
      continue;
    }
    totalQuantity += quantity;
    weightedCost += quantity * average;
  }
  if (totalQuantity <= 0) {
    return "-";
  }
  return trimNumeric(weightedCost / totalQuantity, 4);
}

function metricValue(value?: string | null) {
  if (!value) {
    return null;
  }
  const trimmed = value.trim();
  return trimmed || null;
}

function trimNumeric(value: number, digits: number) {
  return value.toFixed(digits).replace(/\.0+$/, "").replace(/(\.\d*?)0+$/, "$1");
}

function firstValue(value?: string | string[]) {
  return Array.isArray(value) ? value[0] : value;
}

function parseConfirmIds(value?: string) {
  if (!value) {
    return [];
  }
  return value.split(",").map((item) => item.trim()).filter(Boolean);
}

function parseConfirmCount(value: string | undefined, fallback: number) {
  if (!value) {
    return fallback;
  }
  const parsed = Number.parseInt(value, 10);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : fallback;
}

function normalizeConfirmAction(value?: string) {
  if (value === "delete" || value === "pause" || value === "start" || value === "stop-all") {
    return value;
  }
  return null;
}

function confirmDialogCopy(lang: UiLanguage, action: NonNullable<ReturnType<typeof normalizeConfirmAction>>, count: number) {
  switch (action) {
    case "start":
      return {
        buttonClassName: "border border-emerald-500 bg-emerald-500 text-white hover:bg-emerald-600",
        confirmLabel: pickText(lang, "确认启动", "Start"),
        description: pickText(lang, `将启动选中的 ${count} 个机器人。`, `Start ${count} selected bot(s).`),
        title: pickText(lang, "确认批量启动", "Confirm Batch Start"),
      };
    case "pause":
      return {
        buttonClassName: "border border-amber-500 bg-amber-500 text-white hover:bg-amber-600",
        confirmLabel: pickText(lang, "确认暂停", "Pause"),
        description: pickText(lang, `将暂停选中的 ${count} 个机器人。`, `Pause ${count} selected bot(s).`),
        title: pickText(lang, "确认批量暂停", "Confirm Batch Pause"),
      };
    case "delete":
      return {
        buttonClassName: "border border-red-500 bg-red-500 text-white hover:bg-red-600",
        confirmLabel: pickText(lang, "确认删除", "Delete"),
        description: pickText(lang, `将删除选中的 ${count} 个机器人。确认后不可撤回。`, `Delete ${count} selected bot(s). This cannot be undone.`),
        title: pickText(lang, "确认批量删除", "Confirm Batch Delete"),
      };
    case "stop-all":
      return {
        buttonClassName: "border border-red-500 bg-red-500 text-white hover:bg-red-600",
        confirmLabel: pickText(lang, "确认停止", "Stop"),
        description: pickText(lang, "将停止所有正在运行的机器人。停止后需要你手动重新启动。", "Stop every running bot. You will need to restart them manually."),
        title: pickText(lang, "确认全部停止", "Confirm Stop All"),
      };
  }
}

function viewHref(locale: string, status: string, symbol: string, view: "cards" | "table") {
  const params = new URLSearchParams();
  params.set("view", view);
  if (status !== "all") {
    params.set("status", status);
  }
  if (symbol.trim()) {
    params.set("symbol", symbol.trim());
  }
  return `/${locale}/app/strategies?${params.toString()}`;
}

function strategyNotice(lang: UiLanguage, notice?: string) {
  switch (notice) {
    case "preview-batch-start":
      return pickText(lang, "已模拟批量启动，真实账户不会执行。", "Batch start was simulated. No live account action was run.");
    case "preview-batch-pause":
      return pickText(lang, "已模拟批量暂停，真实账户不会执行。", "Batch pause was simulated. No live account action was run.");
    case "preview-batch-delete":
      return pickText(lang, "已模拟批量删除，真实账户不会执行。", "Batch delete was simulated. No live account action was run.");
    case "preview-start":
      return pickText(lang, "已模拟启动机器人。", "Bot start was simulated.");
    case "preview-pause":
      return pickText(lang, "已模拟暂停机器人。", "Bot pause was simulated.");
    case "preview-stop":
      return pickText(lang, "已模拟停止机器人。", "Bot stop was simulated.");
    case "preview-delete":
      return pickText(lang, "已模拟删除机器人。", "Bot delete was simulated.");
    case "preview-stop-all":
      return pickText(lang, "已模拟全部停止，真实账户不会执行。", "Stop all was simulated. No live account action was run.");
    case "preview-martingale-create":
      return pickText(lang, "已模拟创建马丁机器人，真实账户不会执行。", "DCA bot creation was simulated. No live account action was run.");
    case "batch-start-complete":
      return pickText(lang, "批量启动已提交。", "Batch start submitted.");
    case "batch-pause-complete":
      return pickText(lang, "批量暂停已提交。", "Batch pause submitted.");
    case "batch-delete-complete":
      return pickText(lang, "批量删除已提交。", "Batch delete submitted.");
    case "stop-all-complete":
      return pickText(lang, "全部停止已提交。", "Stop all submitted.");
    default:
      return null;
  }
}

function previewStrategies(): StrategyListItem[] {
  return [
    {
      avgEntryPrice: "68420.50",
      budget: "1200 USDT",
      fillCount: 18,
      gridCount: 36,
      gridPnl: "+86.40",
      id: "preview-btc-grid",
      market: "Spot",
      name: "BTC 稳健网格",
      overallPnl: "+128.64",
      overallPnlPct: "+10.72%",
      runtimeDuration: "3天 6小时",
      status: "Running",
      symbol: "BTCUSDT",
      todayPnl: "+18.20",
      tradeCount: 18,
      gridUtilization: 0.72,
    },
    {
      avgEntryPrice: "3568.20",
      budget: "800 USDT",
      fillCount: 9,
      gridCount: 28,
      gridPnl: "+24.15",
      id: "preview-eth-grid",
      market: "FuturesUsdM",
      name: "ETH 合约小额试跑",
      overallPnl: "+31.80",
      overallPnlPct: "+3.98%",
      runtimeDuration: "1天 14小时",
      status: "Paused",
      symbol: "ETHUSDT",
      todayPnl: "+0.00",
      tradeCount: 9,
      gridUtilization: 0.46,
    },
    {
      avgEntryPrice: "142.35",
      budget: "500 USDT",
      fillCount: 5,
      gridCount: 24,
      gridPnl: "-12.60",
      id: "preview-sol-dca",
      market: "FuturesUsdM",
      name: "SOL 马丁观察仓",
      overallPnl: "-18.72",
      overallPnlPct: "-3.74%",
      runtimeDuration: "8小时",
      status: "ErrorPaused",
      symbol: "SOLUSDT",
      todayPnl: "-6.18",
      tradeCount: 5,
      gridUtilization: 0.31,
    },
    {
      avgEntryPrice: "-",
      budget: "300 USDT",
      fillCount: 0,
      gridCount: 18,
      gridPnl: "0.00",
      id: "preview-bnb-draft",
      market: "Spot",
      name: "BNB 新手模板草稿",
      overallPnl: "0.00",
      overallPnlPct: "0.00%",
      runtimeDuration: "—",
      status: "Draft",
      symbol: "BNBUSDT",
      todayPnl: "0.00",
      tradeCount: 0,
      gridUtilization: 0,
    },
  ];
}

async function fetchStrategies(lang: UiLanguage): Promise<{ items: StrategyListResponse["items"]; error: string | null }> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (!sessionToken) {
    return { items: [], error: null };
  }
  const response = await fetch(authApiBaseUrl() + "/strategies", {
    method: "GET",
    headers: { authorization: "Bearer " + sessionToken },
    cache: "no-store",
  });
  if (!response.ok) {
    return { items: [], error: pickText(lang, "策略列表暂时不可用。", "Strategy catalog is temporarily unavailable.") };
  }
  return { items: ((await response.json()) as StrategyListResponse).items, error: null };
}

async function fetchAnalytics(lang: UiLanguage): Promise<StrategySummaries | null> {
  const cookieStore = await cookies();
  const sessionToken = cookieStore.get("session_token")?.value ?? "";
  if (!sessionToken) {
    return null;
  }
  const response = await fetch(authApiBaseUrl() + "/analytics/strategies", {
    method: "GET",
    headers: { authorization: "Bearer " + sessionToken },
    cache: "no-store",
  });
  if (!response.ok) {
    console.warn(pickText(lang, "策略页统计拉取失败，将退回运行态摘要。", "Strategy-page analytics fetch failed; falling back to runtime summaries."));
    return null;
  }
  return (await response.json()) as StrategySummaries;
}

function authApiBaseUrl() {
  return process.env.AUTH_API_BASE_URL?.trim().replace(/\/+$/, "") || DEFAULT_AUTH_API_BASE_URL;
}
