"use client";

import { Search } from "lucide-react";

import { Button, Input } from "@/components/ui/form";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";

export type StrategySymbolItem = {
  base_asset: string;
  market: string;
  quote_asset: string;
  symbol: string;
};

type Props = {
  items: StrategySymbolItem[];
  lang: UiLanguage;
  query: string;
  selectedSymbol: string;
  onQueryChange: (value: string) => void;
  onSearch: () => void;
  onSelect: (item: StrategySymbolItem) => void;
};

export function StrategySymbolPicker({
  items,
  lang,
  query,
  selectedSymbol,
  onQueryChange,
  onSearch,
  onSelect,
}: Props) {
  return (
    <div className="space-y-3 rounded-2xl border border-border bg-card p-4">
      <div className="space-y-1">
        <p className="text-xs font-semibold tracking-wide text-muted-foreground uppercase">
          {pickText(lang, "交易对搜索", "Symbol Search")}
        </p>
        <div className="flex gap-2">
          <div className="relative flex-1">
            <Search className="pointer-events-none absolute left-3 top-1/2 h-4 w-4 -translate-y-1/2 text-muted-foreground" />
            <Input
              className="pl-9"
              onChange={(event) => onQueryChange(event.target.value)}
              onKeyDown={(event) => {
                if (event.key === "Enter") {
                  event.preventDefault();
                  onSearch();
                }
              }}
              placeholder={pickText(lang, "输入 BTC、ETH、USDT 或市场关键词", "Type BTC, ETH, USDT, or a market keyword")}
              value={query}
            />
          </div>
          <Button className="shrink-0" onClick={onSearch} type="button">
            {pickText(lang, "搜索", "Search")}
          </Button>
        </div>
      </div>

      <div className="rounded-xl border border-dashed border-border bg-muted/40 px-3 py-2 text-sm text-foreground">
        <span className="text-xs uppercase tracking-wide text-muted-foreground">
          {pickText(lang, "当前选择", "Selected")}
        </span>
        <div className="mt-1 font-mono text-sm font-semibold">{selectedSymbol || "-"}</div>
      </div>

      <div className="space-y-2" data-symbol-picker-results="true">
        <div className="flex items-center justify-between">
          <p className="text-xs font-semibold tracking-wide text-muted-foreground uppercase">
            {pickText(lang, "可选结果", "Available Results")}
          </p>
          <span className="text-xs text-muted-foreground">{items.length}</span>
        </div>
        {items.length > 0 ? (
          <div className="max-h-72 space-y-2 overflow-y-auto pr-1">
            {items.map((item) => {
              const active = item.symbol === selectedSymbol;
              return (
                <button
                  className={[
                    "w-full rounded-xl border px-3 py-3 text-left transition-colors",
                    active ? "border-primary bg-primary/10" : "border-border bg-background hover:border-primary/50 hover:bg-muted/60",
                  ].join(" ")}
                  key={`${item.market}-${item.symbol}`}
                  onClick={() => onSelect(item)}
                  type="button"
                >
                  <div className="flex items-center justify-between gap-3">
                    <div>
                      <div className="font-mono text-sm font-semibold text-foreground">{item.symbol}</div>
                      <div className="text-xs text-muted-foreground">{item.base_asset}/{item.quote_asset}</div>
                    </div>
                    <span className="rounded-full border border-border px-2 py-1 text-[11px] text-muted-foreground">
                      {describeMarket(lang, item.market)}
                    </span>
                  </div>
                </button>
              );
            })}
          </div>
        ) : (
          <div className="rounded-xl border border-dashed border-border px-3 py-6 text-center text-sm text-muted-foreground">
            {pickText(lang, "先搜索交易对，然后在这里点击选择。", "Search for symbols first, then click one here to select it.")}
          </div>
        )}
      </div>
    </div>
  );
}

function describeMarket(lang: UiLanguage, market: string) {
  switch (market) {
    case "spot":
      return pickText(lang, "现货", "Spot");
    case "usdm":
      return pickText(lang, "U 本位合约", "USD-M Futures");
    case "coinm":
      return pickText(lang, "币本位合约", "COIN-M Futures");
    default:
      return market;
  }
}
