"use client";

import { useQuery } from "@tanstack/react-query";
import { fetchAlertTriggers } from "./api";

export type AlertTrigger = {
  id: string;
  rule_id: string;
  wallet_id: string;
  message: string;
  created_at: string;
};

export function useAlertTriggers(limit = 20) {
  const query = useQuery({
    queryKey: ["alert-triggers", limit],
    queryFn: async () => {
      const res = await fetchAlertTriggers();
      if (!res.ok) {
        throw new Error(`讀取告警失敗 (${res.status})`);
      }
      return (await res.json()) as AlertTrigger[];
    },
    refetchInterval: 60_000,
  });

  return {
    triggers: query.data ?? [],
    loading: query.isLoading,
    error: query.error ? (query.error as Error).message : null,
    refresh: query.refetch,
  };
}
