"use client";

import useSWR from "swr";

type StrategyDetail = {
  id: string;
  name: string;
  status: string;
  symbol: string;
  strategy_type: string;
  runtime?: {
    current_price?: number;
    total_pnl?: string;
    filled_grids?: number;
    total_grids?: number;
  };
};

const fetcher = (url: string) => fetch(url).then((r) => r.json());

export function useStrategyDetail(id: string) {
  const { data, error, isLoading, mutate } = useSWR<StrategyDetail>(
    `/api/user/strategies/${id}`,
    fetcher,
    {
      refreshInterval: 10000,
      revalidateOnFocus: true,
    },
  );

  return {
    strategy: data ?? null,
    error,
    isLoading,
    refresh: mutate,
  };
}
