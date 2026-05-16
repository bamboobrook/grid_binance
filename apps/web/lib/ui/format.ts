export function formatPnl(value: number | null | undefined): string {
  if (value == null) return "—";
  const sign = value > 0 ? "+" : "";
  return `${sign}${value.toFixed(2)}`;
}

export function formatPnlWithCurrency(
  value: number | null | undefined,
  currency = "USDT",
): string {
  if (value == null) return "—";
  const sign = value > 0 ? "+" : "";
  return `${sign}${value.toFixed(2)} ${currency}`;
}

export function formatPrice(
  value: number | null | undefined,
  precision?: number,
): string {
  if (value == null) return "—";
  const p = precision ?? (value >= 1000 ? 2 : value >= 1 ? 4 : 6);
  return value.toFixed(p);
}

export function formatAmount(value: number | null | undefined): string {
  if (value == null) return "—";
  return value.toLocaleString("en-US", {
    minimumFractionDigits: 2,
    maximumFractionDigits: 2,
  });
}

export function formatPercent(value: number | null | undefined): string {
  if (value == null) return "—";
  const sign = value > 0 ? "+" : "";
  return `${sign}${value.toFixed(2)}%`;
}

export function pnlColor(value: number | null | undefined): string {
  if (value == null || value === 0) return "text-muted-foreground";
  return value > 0 ? "text-emerald-500" : "text-red-500";
}

export function pnlBg(value: number | null | undefined): string {
  if (value == null || value === 0) return "bg-muted";
  return value > 0 ? "bg-emerald-500/10" : "bg-red-500/10";
}
