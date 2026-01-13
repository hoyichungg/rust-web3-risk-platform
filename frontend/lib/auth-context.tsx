"use client";

import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
} from "react";
import { apiFetch, requestSignOut, requestTokenRefresh } from "./api";

export type UserWallet = {
  id: string;
  address: string;
  chain_id: number;
};

export type UserProfile = {
  id: string;
  primary_wallet: string;
  role: string;
  wallets: UserWallet[];
};

type RefreshOptions = {
  silent?: boolean;
};

type AuthContextValue = {
  profile: UserProfile | null;
  loading: boolean;
  error: string | null;
  refreshProfile: (options?: RefreshOptions) => Promise<UserProfile | null>;
  signOut: () => Promise<void>;
};

const AuthContext = createContext<AuthContextValue | undefined>(undefined);

export function AuthProvider({ children }: { children: React.ReactNode }) {
  const [profile, setProfile] = useState<UserProfile | null>(null);
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const bootstrappedRef = useRef(false);

  const refreshProfile = useCallback(
    async (options?: RefreshOptions): Promise<UserProfile | null> => {
      const silent = options?.silent ?? false;
      if (!silent) {
        setLoading(true);
        setError(null);
      }

      try {
        let res = await apiFetch("/api/me");
        if (res.status === 401) {
          const refreshRes = await requestTokenRefresh();
          if (refreshRes.ok) {
            res = await apiFetch("/api/me");
          }
        }
        if (!res.ok) {
          if (res.status === 401 || res.status === 403) {
            setProfile(null);
            if (!silent) {
              setError("UNAUTHORIZED");
            }
            return null;
          }
          throw new Error(`failed to load profile (${res.status})`);
        }
        const data: UserProfile = await res.json();
        setProfile(data);
        if (!silent) {
          setError(null);
        }
        return data;
      } catch (err) {
        if (!silent) {
          setError(err instanceof Error ? err.message : "未知錯誤");
        }
        return null;
      } finally {
        if (!silent) {
          setLoading(false);
        }
      }
    },
    [],
  );

  const signOut = useCallback(async () => {
    await requestSignOut();
    setProfile(null);
    setError(null);
  }, []);

  useEffect(() => {
    if (bootstrappedRef.current) {
      return;
    }
    bootstrappedRef.current = true;
    refreshProfile({ silent: false }).catch(() => {
      /* ignore */
    });
  }, [refreshProfile]);

  const value = useMemo(
    () => ({
      profile,
      loading,
      error,
      refreshProfile,
      signOut,
    }),
    [profile, loading, error, refreshProfile, signOut],
  );

  return <AuthContext.Provider value={value}>{children}</AuthContext.Provider>;
}

export function useAuth(): AuthContextValue {
  const ctx = useContext(AuthContext);
  if (!ctx) {
    throw new Error("useAuth must be used within AuthProvider");
  }
  return ctx;
}

export function useProfile() {
  const { profile, loading, error, refreshProfile } = useAuth();
  return { profile, loading, error, refresh: refreshProfile };
}
