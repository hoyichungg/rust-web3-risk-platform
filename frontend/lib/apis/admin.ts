import { apiFetch } from "./http";

export function fetchAdminSessions(): Promise<Response> {
  return apiFetch("/api/admin/sessions");
}

export function revokeAdminSession(sessionId: string): Promise<Response> {
  return apiFetch(`/api/admin/sessions/${sessionId}/revoke`, { method: "POST" });
}

export function refreshAdminRoles(): Promise<Response> {
  return apiFetch("/api/admin/roles/refresh", { method: "POST" });
}

export function fetchAdminUsers(): Promise<Response> {
  return apiFetch("/api/admin/users");
}
