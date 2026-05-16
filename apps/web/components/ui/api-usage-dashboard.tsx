"use client";

import { pickText, type UiLanguage } from "@/lib/ui/preferences";

type ApiUsage = {
  used: number;
  limit: number;
  label: string;
};

export function ApiUsageDashboard({ lang, usages }: { lang: UiLanguage; usages?: ApiUsage[] }) {
  const defaultUsages: ApiUsage[] = usages ?? [
    { used: 120, limit: 1200, label: pickText(lang, "REST 请求/分钟", "REST req/min") },
    { used: 3, limit: 5, label: pickText(lang, "WebSocket 连接", "WebSocket connections") },
    { used: 8500, limit: 60000, label: pickText(lang, "订单/天", "Orders/day") },
  ];

  return (
    <div className="space-y-3">
      <h3 className="text-sm font-medium text-muted-foreground">
        {pickText(lang, "API 用量", "API Usage")}
      </h3>
      {defaultUsages.map((u) => {
        const pct = Math.min((u.used / u.limit) * 100, 100);
        const isWarning = pct > 80;
        const isDanger = pct > 95;
        return (
          <div key={u.label}>
            <div className="flex items-center justify-between text-xs">
              <span>{u.label}</span>
              <span className={isDanger ? "font-bold text-red-600" : isWarning ? "font-medium text-yellow-600" : ""}>
                {u.used} / {u.limit}
              </span>
            </div>
            <div className="mt-1 h-1.5 w-full rounded-full bg-muted">
              <div
                className={`h-full rounded-full transition-all ${
                  isDanger ? "bg-red-500" : isWarning ? "bg-yellow-500" : "bg-primary"
                }`}
                style={{ width: `${pct}%` }}
              />
            </div>
          </div>
        );
      })}
    </div>
  );
}
