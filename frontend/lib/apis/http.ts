const API_BASE_URL = (
  process.env.NEXT_PUBLIC_BACKEND_URL ?? "http://localhost:8081"
).replace(/\/$/, "");

export type QueryValue = string | number | boolean | null | undefined;
export type QueryParams = Record<string, QueryValue>;

function buildUrl(path: string, query?: QueryParams): string {
  const url = path.startsWith("http")
    ? new URL(path)
    : new URL(path, `${API_BASE_URL}/`);

  if (query) {
    Object.entries(query).forEach(([key, value]) => {
      if (value === undefined || value === null) return;
      url.searchParams.set(key, String(value));
    });
  }

  return url.toString();
}

function withDefaults(init?: RequestInit): RequestInit {
  const headers = new Headers(init?.headers ?? undefined);
  if (!headers.has("Accept")) {
    headers.set("Accept", "application/json");
  }

  return {
    credentials: init?.credentials ?? "include",
    ...init,
    headers,
  };
}

export class ApiError extends Error {
  constructor(
    message: string,
    public status: number,
    public path: string,
    public body?: unknown,
  ) {
    super(message);
    this.name = "ApiError";
  }
}

async function toApiError(path: string, res: Response): Promise<ApiError> {
  let parsed: unknown = undefined;
  let raw: string | undefined;
  try {
    raw = await res.text();
    parsed = raw ? JSON.parse(raw) : undefined;
  } catch {
    parsed = raw ?? undefined;
  }
  const statusLabel = res.statusText ? `${res.status} ${res.statusText}` : `${res.status}`;
  const message = `Request to ${path} failed (${statusLabel})`;
  return new ApiError(message, res.status, path, parsed);
}

export function getApiBaseUrl(): string {
  return API_BASE_URL;
}

export function apiFetch(
  path: string,
  init?: RequestInit,
  query?: QueryParams,
): Promise<Response> {
  return fetch(buildUrl(path, query), withDefaults(init));
}

export async function apiJson<T>(
  path: string,
  init?: RequestInit,
  query?: QueryParams,
): Promise<T> {
  const res = await apiFetch(path, init, query);
  if (!res.ok) {
    throw await toApiError(path, res);
  }
  return (await res.json()) as T;
}

export function apiPostJson(
  path: string,
  body: unknown,
  init?: RequestInit,
  query?: QueryParams,
): Promise<Response> {
  const headers = new Headers(init?.headers ?? {});
  if (!headers.has("Content-Type")) {
    headers.set("Content-Type", "application/json");
  }

  return apiFetch(path, {
    ...init,
    method: init?.method ?? "POST",
    headers,
    body: JSON.stringify(body),
  }, query);
}
