"use client";

import {
  Alert,
  Box,
  Button,
  Card,
  CardContent,
  Stack,
  TextField,
  Typography,
} from "@mui/material";
import { useRouter } from "next/navigation";
import { useCallback, useEffect, useState } from "react";
import { apiFetch, apiPostJson } from "../../lib/api";
import { useAuth } from "../../lib/auth-context";

type EthereumProvider = {
  request: (args: { method: string; params?: unknown[] }) => Promise<any>;
};

declare global {
  interface Window {
    ethereum?: EthereumProvider;
  }
}

export default function LoginPage() {
  const [walletAddress, setWalletAddress] = useState("");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const router = useRouter();
  const { profile, refreshProfile } = useAuth();

  const connectWallet = useCallback(async () => {
    setError(null);
    const provider = window.ethereum;
    if (!provider) {
      setError("找不到錢包，請確認已安裝 MetaMask 或其他 EIP-1193 provider。");
      return;
    }

    try {
      const accounts = await provider.request({
        method: "eth_requestAccounts",
      });
      if (accounts && accounts.length > 0) {
        setWalletAddress(accounts[0]);
      }
    } catch (err) {
      console.error(err);
      setError("連接錢包失敗");
    }
  }, []);

  const handleLogin = useCallback(async () => {
    if (!walletAddress) {
      setError("請先連接錢包");
      return;
    }
    const provider = window.ethereum;
    if (!provider) {
      setError("找不到錢包 provider");
      return;
    }

    setLoading(true);
    setError(null);
    const statement = "Sign in to Rust Web3 Risk Platform";

    try {
      const nonceRes = await apiFetch("/api/auth/nonce");
      if (!nonceRes.ok) {
        throw new Error("無法取得 nonce");
      }
      const nonceData = await nonceRes.json();
      const chainIdHex = await provider.request({
        method: "eth_chainId",
      });
      const chainId = Number.parseInt(
        typeof chainIdHex === "string" ? chainIdHex : "0x1",
        16
      );
      const issuedAt = new Date().toISOString();
      const domain = window.location.host;
      const origin = window.location.origin;
      const message = `${domain} wants you to sign in with your Ethereum account:
${walletAddress}

${statement}

URI: ${origin}
Version: 1
Chain ID: ${chainId}
Nonce: ${nonceData.nonce}
Issued At: ${issuedAt}`;

      const signature = await provider.request({
        method: "personal_sign",
        params: [message, walletAddress],
      });

      const loginRes = await apiPostJson("/api/auth/login", {
        message,
        signature,
      });

      if (!loginRes.ok) {
        throw new Error("登入失敗");
      }

      await refreshProfile();
      router.push("/dashboard");
    } catch (err) {
      console.error(err);
      setError(err instanceof Error ? err.message : "登入失敗");
    } finally {
      setLoading(false);
    }
  }, [refreshProfile, router, walletAddress]);

  useEffect(() => {
    if (profile) {
      router.replace("/dashboard");
    }
  }, [profile, router]);

  return (
    <Box
      sx={{
        minHeight: "100vh",
        display: "flex",
        alignItems: "center",
        justifyContent: "center",
        background:
          "radial-gradient(circle at 20% 20%, rgba(97,218,251,0.15), transparent 40%), radial-gradient(circle at 80% 0%, rgba(187,134,252,0.12), transparent 40%), #05070c",
      }}
    >
      <Card sx={{ width: 420, bgcolor: "background.paper", boxShadow: 24 }}>
        <CardContent>
          <Stack spacing={3}>
            <Box>
              <Typography variant="h4" fontWeight={700}>
                Web3 Login
              </Typography>
              <Typography variant="body2" color="text.secondary">
                使用錢包簽名登入平台，開始檢視多鏈資產。
              </Typography>
            </Box>
            <Stack direction="row" spacing={1.5}>
              <Button
                variant="outlined"
                color="primary"
                onClick={connectWallet}
                sx={{ whiteSpace: "nowrap" }}
              >
                連接錢包
              </Button>
              <TextField
                fullWidth
                placeholder="0x..."
                label="錢包地址"
                value={walletAddress}
                onChange={(e) => setWalletAddress(e.target.value)}
              />
            </Stack>
            <Button
              onClick={handleLogin}
              disabled={loading || !walletAddress}
              size="large"
            >
              {loading ? "登入中..." : "使用簽名登入"}
            </Button>
            {error && (
              <Alert severity="error" variant="outlined">
                {error}
              </Alert>
            )}
            {profile?.role && (
              <Typography variant="body2" color="text.secondary">
                目前角色：{profile.role}
              </Typography>
            )}
          </Stack>
        </CardContent>
      </Card>
    </Box>
  );
}
