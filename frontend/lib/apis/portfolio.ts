import { apiFetch, apiPostJson } from "./http";

export function setPrimaryWallet(walletId: string): Promise<Response> {
  return apiFetch(`/api/wallets/${walletId}/primary`, { method: "POST" });
}

export function fetchPortfolioHistory(walletId: string, limit = 30): Promise<Response> {
  return apiFetch(`/api/portfolio/${walletId}/history`, undefined, { limit });
}

export function fetchPortfolioSnapshots(walletId: string, days = 7): Promise<Response> {
  return apiFetch(`/api/portfolio/${walletId}/snapshots`, undefined, { days });
}

export function createWallet(payload: { address: string; chain_id: number }): Promise<Response> {
  return apiPostJson("/api/wallets", payload);
}

export function deleteWallet(walletId: string): Promise<Response> {
  return apiFetch(`/api/wallets/${walletId}`, { method: "DELETE" });
}
