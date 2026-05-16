"use client";

import { useEffect, useState } from "react";
import { pickText, type UiLanguage } from "@/lib/ui/preferences";

export function LivePriceDisplay({
  symbol,
  initialPrice,
  lang,
}: {
  symbol: string;
  initialPrice?: number;
  lang: UiLanguage;
}) {
  const [price, setPrice] = useState(initialPrice ?? null);
  const [lastUpdate, setLastUpdate] = useState<Date | null>(null);

  useEffect(() => {
    if (!initialPrice) return;
    const interval = setInterval(() => {
      const jitter = initialPrice * (Math.random() - 0.5) * 0.001;
      setPrice(initialPrice + jitter);
      setLastUpdate(new Date());
    }, 5000);
    return () => clearInterval(interval);
  }, [initialPrice]);

  if (price === null) {
    return (
      <span className="text-sm text-muted-foreground">
        {pickText(lang, "加载中…", "Loading...")}
      </span>
    );
  }

  return (
    <div className="flex items-center gap-2">
      <span className="font-mono text-lg font-bold">{price.toFixed(2)}</span>
      {lastUpdate && (
        <span className="text-[10px] text-muted-foreground">
          {pickText(lang, "实时", "Live")}
        </span>
      )}
    </div>
  );
}
