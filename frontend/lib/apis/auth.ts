import { apiFetch } from "./http";

export async function requestSignOut(): Promise<void> {
  await apiFetch("/api/auth/logout", { method: "POST" });
}

export async function requestTokenRefresh(): Promise<Response> {
  return apiFetch("/api/auth/refresh", { method: "POST" });
}
