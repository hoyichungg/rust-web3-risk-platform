"use client";

import {
  Card,
  CardContent,
  Chip,
  Container,
  Stack,
  Typography,
} from "@mui/material";
import { useEffect, useState } from "react";
import { useProfile } from "../../../lib/auth-context";
import { useRouter } from "next/navigation";
import { fetchAdminUsers } from "../../../lib/api";

type AdminWallet = {
  id: string;
  address: string;
  chain_id: number;
  cached_role?: number;
  cached_at?: string;
};

type AdminUser = {
  id: string;
  primary_wallet: string;
  wallets: AdminWallet[];
};

export default function AdminUsersPage() {
  const { profile, loading } = useProfile();
  const router = useRouter();
  const [users, setUsers] = useState<AdminUser[]>([]);

  useEffect(() => {
    if (!loading && profile?.role !== "Admin") {
      router.replace("/dashboard");
    }
  }, [loading, profile, router]);

  useEffect(() => {
    fetchAdminUsers()
      .then(async (res) => {
        if (!res.ok) throw new Error();
        setUsers(await res.json());
      })
      .catch(() => {});
  }, []);

  return (
    <Container sx={{ py: 4 }}>
      <Typography variant="h4" fontWeight={800} gutterBottom>
        Admin: 用戶 / 錢包
      </Typography>
      <Stack spacing={2}>
        {users.map((u) => (
          <Card key={u.id}>
            <CardContent>
              <Typography variant="subtitle1" fontWeight={700}>
                {u.id}
              </Typography>
              <Typography variant="caption" color="text.secondary">
                Primary: {u.primary_wallet}
              </Typography>
              <Stack spacing={1.2} mt={1.2}>
                {u.wallets.map((w) => (
                  <Stack
                    key={w.id}
                    direction="row"
                    spacing={1}
                    alignItems="center"
                    sx={{ fontFamily: "monospace" }}
                  >
                    <Typography variant="body2">{w.address}</Typography>
                    <Chip size="small" label={`Chain ${w.chain_id}`} variant="outlined" />
                    {w.cached_role !== undefined && (
                      <Chip
                        size="small"
                        label={`RoleCache ${w.cached_role}`}
                        variant="outlined"
                      />
                    )}
                    {w.cached_at && (
                      <Typography variant="caption" color="text.secondary">
                        {new Date(w.cached_at).toLocaleString("zh-TW")}
                      </Typography>
                    )}
                  </Stack>
                ))}
              </Stack>
            </CardContent>
          </Card>
        ))}
        {users.length === 0 && (
          <Typography variant="body2" color="text.secondary">
            尚無資料。
          </Typography>
        )}
      </Stack>
    </Container>
  );
}
