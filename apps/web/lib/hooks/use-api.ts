"use client";

import useSWR from "swr";

const fetcher = (url: string) => fetch(url).then((r) => {
  if (!r.ok) throw new Error(`HTTP ${r.status}`);
  return r.json();
});

export function useStrategies(page = 1, perPage = 20) {
  const { data, error, isLoading, mutate } = useSWR(
    `/api/user/strategies?page=${page}&per_page=${perPage}`,
    fetcher,
    { refreshInterval: 15000, revalidateOnFocus: true },
  );
  return { strategies: data ?? null, error, isLoading, refresh: mutate };
}

export function useOrders(page = 1, perPage = 20) {
  const { data, error, isLoading, mutate } = useSWR(
    `/api/user/orders?page=${page}&per_page=${perPage}`,
    fetcher,
    { refreshInterval: 30000 },
  );
  return { orders: data ?? null, error, isLoading, refresh: mutate };
}

export function useAnalytics() {
  const { data, error, isLoading, mutate } = useSWR(
    "/api/user/analytics",
    fetcher,
    { refreshInterval: 60000 },
  );
  return { analytics: data ?? null, error, isLoading, refresh: mutate };
}
