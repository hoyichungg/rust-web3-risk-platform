import { apiFetch, apiPostJson } from "./http";

export function fetchStrategies(): Promise<Response> {
  return apiFetch("/api/strategies");
}

export function createStrategy(payload: {
  name: string;
  type: string;
  params: Record<string, unknown>;
}): Promise<Response> {
  return apiPostJson("/api/strategies", payload);
}

export function backtestStrategy(
  id: string,
  payload: { prices?: any[]; short_window?: number; long_window?: number },
): Promise<Response> {
  return apiPostJson(`/api/strategies/${id}/backtest`, payload);
}

export function deleteStrategy(id: string): Promise<Response> {
  return apiFetch(`/api/strategies/${id}`, { method: "DELETE" });
}

export function fetchStrategyBacktests(id: string, limit = 5): Promise<Response> {
  return apiFetch(`/api/strategies/${id}/backtests`, undefined, { limit });
}
