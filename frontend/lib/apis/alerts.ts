import { apiFetch, apiPostJson } from "./http";

export function fetchAlerts(): Promise<Response> {
  return apiFetch("/api/alerts");
}

export function createAlert(payload: {
  type: string;
  threshold: number;
  enabled?: boolean;
  cooldown_secs?: number;
}) {
  return apiPostJson("/api/alerts", payload);
}

export function updateAlert(
  id: string,
  payload: { type: string; threshold: number; enabled?: boolean; cooldown_secs?: number },
) {
  return apiPostJson(`/api/alerts/${id}`, payload, { method: "PUT" });
}

export function deleteAlert(id: string) {
  return apiFetch(`/api/alerts/${id}`, { method: "DELETE" });
}

export function fetchAlertTriggers() {
  return apiFetch("/api/alerts/triggers");
}

export function simulateAlertTrigger(id: string) {
  return apiFetch(`/api/alerts/${id}/test`, { method: "POST" });
}
