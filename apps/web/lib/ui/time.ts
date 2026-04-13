import type { UiLanguage } from "@/lib/ui/preferences";

const TAIPEI_TIME_ZONE = "Asia/Taipei";

export function formatTaipeiDateTime(
  value: string | Date | null | undefined,
  lang: UiLanguage = "zh",
  options?: { fallback?: string; withSeconds?: boolean },
) {
  const date = normalizeDate(value);
  if (!date) {
    return options?.fallback ?? "-";
  }

  const formatter = new Intl.DateTimeFormat(lang === "en" ? "en-CA" : "zh-TW", {
    day: "2-digit",
    hour: "2-digit",
    hour12: false,
    hourCycle: "h23",
    minute: "2-digit",
    month: "2-digit",
    second: options?.withSeconds ? "2-digit" : undefined,
    timeZone: TAIPEI_TIME_ZONE,
    year: "numeric",
  });

  const parts = Object.fromEntries(
    formatter
      .formatToParts(date)
      .filter((part) => part.type !== "literal")
      .map((part) => [part.type, part.value]),
  ) as Record<string, string>;

  const hour = parts.hour === "24" ? "00" : (parts.hour ?? "00");
  const seconds = options?.withSeconds ? `:${parts.second ?? "00"}` : "";
  return `${parts.year}-${parts.month}-${parts.day} ${hour}:${parts.minute ?? "00"}${seconds}`;
}

export function formatTaipeiDate(
  value: string | Date | null | undefined,
  lang: UiLanguage = "zh",
  fallback = "-",
) {
  const date = normalizeDate(value);
  if (!date) {
    return fallback;
  }

  const formatter = new Intl.DateTimeFormat(lang === "en" ? "en-CA" : "zh-TW", {
    day: "2-digit",
    month: "2-digit",
    timeZone: TAIPEI_TIME_ZONE,
    year: "numeric",
  });

  const parts = Object.fromEntries(
    formatter
      .formatToParts(date)
      .filter((part) => part.type !== "literal")
      .map((part) => [part.type, part.value]),
  ) as Record<string, string>;

  return `${parts.year}-${parts.month}-${parts.day}`;
}

function normalizeDate(value: string | Date | null | undefined) {
  if (!value) {
    return null;
  }

  const date = value instanceof Date ? value : new Date(value);
  return Number.isNaN(date.getTime()) ? null : date;
}

export const DISPLAY_TIME_ZONE = "UTC+8";
