"use client";

import { useCallback } from "react";
import useSWR from "swr";

type StrategyItem = {
  id: string;
  status: string;
  [key: string]: unknown;
};

type StrategyListResponse = {
  items: StrategyItem[];
  total: number;
};

export function useStrategyActions() {
  const { data, mutate } = useSWR<StrategyListResponse>(
    "/api/user/strategies?page=1&per_page=100",
    (url: string) => fetch(url).then((r) => r.json()),
    { revalidateOnFocus: false },
  );

  const optimisticUpdate = useCallback(
    async (strategyId: string, newStatus: string, action: () => Promise<Response>) => {
      const current = data;
      if (current) {
        const updated = {
          ...current,
          items: current.items.map((s) =>
            s.id === strategyId ? { ...s, status: newStatus } : s,
          ),
        };
        await mutate(updated, false);
      }
      try {
        const res = await action();
        if (!res.ok) throw new Error(`HTTP ${res.status}`);
      } catch {
        await mutate(current, false);
        throw new Error("Action failed, reverted");
      }
      await mutate();
    },
    [data, mutate],
  );

  const startStrategy = useCallback(
    (id: string) =>
      optimisticUpdate(id, "Running", () =>
        fetch(`/api/user/strategies/${id}/start`, { method: "POST" }),
      ),
    [optimisticUpdate],
  );

  const pauseStrategy = useCallback(
    (id: string) =>
      optimisticUpdate(id, "Paused", () =>
        fetch(`/api/user/strategies/${id}/stop`, { method: "POST" }),
      ),
    [optimisticUpdate],
  );

  const stopStrategy = useCallback(
    (id: string) =>
      optimisticUpdate(id, "Stopped", () =>
        fetch(`/api/user/strategies/${id}/stop`, { method: "POST" }),
      ),
    [optimisticUpdate],
  );

  return { startStrategy, pauseStrategy, stopStrategy, strategies: data };
}
