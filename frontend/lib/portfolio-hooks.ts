"use client";

import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  apiFetch,
  apiPostJson,
  setPrimaryWallet as apiSetPrimaryWallet,
  fetchPortfolioHistory,
  fetchPortfolioSnapshots,
} from "./api";

export type WalletInfo = {
  id: string;
  address: string;
  chain_id: number;
};

export type PortfolioPosition = {
  asset_symbol: string;
  amount: number;
  usd_value: number;
};

export type PortfolioSnapshot = {
  wallet_id: string;
  total_usd_value: number;
  timestamp: string;
  positions: PortfolioPosition[];
};

export function usePortfolioHistory(walletId?: string | null, limit = 30) {
  const query = useQuery({
    queryKey: ["portfolio-history", walletId, limit],
    enabled: !!walletId,
    queryFn: async () => {
      if (!walletId) return [];
      const res = await fetchPortfolioHistory(walletId, limit);
      if (!res.ok) {
        throw new Error(`歷史資料讀取失敗 (${res.status})`);
      }
      return (await res.json()) as PortfolioSnapshot[];
    },
    refetchInterval: 30_000,
  });

  return {
    history: query.data ?? [],
    loading: query.isLoading,
    error: query.error ? (query.error as Error).message : null,
    refresh: query.refetch,
  };
}

export function usePortfolioSnapshots(walletId?: string | null, days = 7) {
  const query = useQuery({
    queryKey: ["portfolio-snapshots", walletId, days],
    enabled: !!walletId,
    queryFn: async () => {
      if (!walletId) return [];
      const res = await fetchPortfolioSnapshots(walletId, days);
      if (!res.ok) {
        throw new Error(`快照讀取失敗 (${res.status})`);
      }
      return (await res.json()) as PortfolioSnapshot[];
    },
    refetchInterval: 60_000, // align with price cache TTL/refresh
  });

  return {
    snapshots: query.data ?? [],
    loading: query.isLoading,
    error: query.error ? (query.error as Error).message : null,
    refresh: query.refetch,
  };
}

export function useWallets() {
  const client = useQueryClient();
  const walletQuery = useQuery({
    queryKey: ["wallets"],
    queryFn: async () => {
      const res = await apiFetch("/api/wallets");
      if (!res.ok) {
        throw new Error(`Failed to load wallets (${res.status})`);
      }
      return (await res.json()) as WalletInfo[];
    },
  });

  const addWallet = useMutation({
    mutationFn: async ({ address, chainId }: { address: string; chainId: number }) => {
      const res = await apiPostJson("/api/wallets", {
        address,
        chain_id: chainId,
      });
      if (!res.ok) {
        throw new Error(`建立錢包失敗 (${res.status})`);
      }
      return (await res.json()) as WalletInfo;
    },
    onSuccess: () => client.invalidateQueries({ queryKey: ["wallets"] }),
  });

  const deleteWallet = useMutation({
    mutationFn: async (walletId: string) => {
      const res = await apiFetch(`/api/wallets/${walletId}`, { method: "DELETE" });
      if (!res.ok) {
        throw new Error(`刪除錢包失敗 (${res.status})`);
      }
    },
    onSuccess: () => client.invalidateQueries({ queryKey: ["wallets"] }),
  });

  const setPrimary = useMutation({
    mutationFn: async (walletId: string) => {
      const res = await apiSetPrimaryWallet(walletId);
      if (!res.ok) {
        throw new Error(`設定主錢包失敗 (${res.status})`);
      }
    },
    onSuccess: () => client.invalidateQueries({ queryKey: ["wallets"] }),
  });

  return {
    wallets: walletQuery.data ?? [],
    loading: walletQuery.isLoading,
    error: walletQuery.error ? (walletQuery.error as Error).message : null,
    actionLoading: addWallet.isPending || deleteWallet.isPending || setPrimary.isPending,
    refresh: walletQuery.refetch,
    addWallet: addWallet.mutateAsync,
    deleteWallet: deleteWallet.mutateAsync,
    setPrimary: setPrimary.mutateAsync,
  };
}

export function usePortfolio(walletId?: string | null) {
  const query = useQuery({
    queryKey: ["portfolio", walletId],
    enabled: !!walletId,
    queryFn: async () => {
      if (!walletId) return null;
      const res = await apiFetch(`/api/portfolio/${walletId}`);
      if (res.status === 404) {
        return null;
      }
      if (!res.ok) {
        throw new Error(`載入資產失敗 (${res.status})`);
      }
      return (await res.json()) as PortfolioSnapshot;
    },
    refetchInterval: 30_000,
  });

  return {
    snapshot: query.data ?? null,
    loading: query.isLoading,
    error: query.error ? (query.error as Error).message : null,
    refresh: query.refetch,
  };
}
