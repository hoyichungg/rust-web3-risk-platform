"use client";

import {
  Alert,
  Box,
  Button,
  Card,
  CardContent,
  Chip,
  CircularProgress,
  Container,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  Typography,
} from "@mui/material";
import { useRouter } from "next/navigation";
import { useCallback, useEffect, useMemo, useState } from "react";
import { fetchAdminSessions, revokeAdminSession } from "../../../lib/api";
import { useAuth } from "../../../lib/auth-context";

type AdminSession = {
  id: string;
  user_id: string;
  wallet_id: string;
  wallet_address: string;
  primary_wallet: string;
  created_at: string;
  refreshed_at: string;
  expires_at: string;
  revoked_at: string | null;
};

function formatDate(value: string | null) {
  if (!value) return "—";
  return new Date(value).toLocaleString("zh-TW");
}

function getStatus(session: AdminSession) {
  if (session.revoked_at) {
    return { label: "已撤銷", color: "error" as const };
  }
  const now = Date.now();
  if (new Date(session.expires_at).getTime() < now) {
    return { label: "已過期", color: "warning" as const };
  }
  return { label: "有效", color: "success" as const };
}

export default function AdminSessionsPage() {
  const { profile, loading } = useAuth();
  const router = useRouter();
  const [sessions, setSessions] = useState<AdminSession[]>([]);
  const [fetching, setFetching] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [revokingId, setRevokingId] = useState<string | null>(null);

  const isAdmin = useMemo(() => profile?.role === "Admin", [profile]);

  const loadSessions = useCallback(async () => {
    setFetching(true);
    setError(null);
    try {
      const res = await fetchAdminSessions();
      if (!res.ok) {
        throw new Error(`讀取 sessions 失敗 (${res.status})`);
      }
      const data: AdminSession[] = await res.json();
      setSessions(data);
    } catch (err) {
      setError(err instanceof Error ? err.message : "未知錯誤");
    } finally {
      setFetching(false);
    }
  }, []);

  const revoke = useCallback(
    async (sessionId: string) => {
      setRevokingId(sessionId);
      try {
        const res = await revokeAdminSession(sessionId);
        if (!res.ok) {
          throw new Error(`撤銷失敗 (${res.status})`);
        }
        await loadSessions();
      } catch (err) {
        setError(err instanceof Error ? err.message : "撤銷失敗");
      } finally {
        setRevokingId(null);
      }
    },
    [loadSessions],
  );

  useEffect(() => {
    if (loading) return;
    if (!profile) {
      router.replace("/login");
      return;
    }
    if (!isAdmin) {
      router.replace("/dashboard");
      return;
    }
    loadSessions();
  }, [profile, isAdmin, loading, loadSessions, router]);

  return (
    <Box
      sx={{
        minHeight: "100vh",
        py: 6,
        background:
          "radial-gradient(circle at top left, rgba(97,218,251,0.12), transparent 40%), #05070c",
      }}
    >
      <Container maxWidth="lg">
        <Stack
          direction={{ xs: "column", sm: "row" }}
          justifyContent="space-between"
          spacing={2}
          alignItems={{ xs: "flex-start", sm: "center" }}
          mb={4}
        >
          <Box>
            <Typography variant="h4" fontWeight={700}>
              Admin Sessions
            </Typography>
            <Typography variant="body2" color="text.secondary">
              查看並撤銷登入 session，防止風險或僅允許新 refresh token。
            </Typography>
          </Box>
          <Stack direction="row" spacing={1.5}>
            <Button variant="outlined" onClick={() => router.push("/dashboard")}>
              返回 Dashboard
            </Button>
            <Button onClick={loadSessions} disabled={fetching}>
              {fetching ? "重新整理中..." : "重新整理"}
            </Button>
          </Stack>
        </Stack>

        <Card>
          <CardContent sx={{ overflowX: "auto" }}>
            <Stack direction="row" spacing={1} mb={2}>
              <Chip
                label={`目前 Admin：${profile?.primary_wallet ?? ""}`}
                color="primary"
                variant="outlined"
              />
              <Chip label={`Sessions: ${sessions.length}`} variant="outlined" />
            </Stack>

            {error && (
              <Alert severity="error" sx={{ mb: 2 }}>
                {error}
              </Alert>
            )}

            <Table size="small" sx={{ minWidth: 960 }}>
              <TableHead>
                <TableRow>
                  <TableCell>狀態</TableCell>
                  <TableCell>Session ID</TableCell>
                  <TableCell>Primary Wallet</TableCell>
                  <TableCell>Wallet</TableCell>
                  <TableCell>建立時間</TableCell>
                  <TableCell>Refresh</TableCell>
                  <TableCell>到期</TableCell>
                  <TableCell align="right">操作</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {sessions.map((session) => {
                  const status = getStatus(session);
                  const expired =
                    new Date(session.expires_at).getTime() < Date.now();
                  const disableRevoke = !!session.revoked_at || expired;
                  return (
                    <TableRow key={session.id} hover>
                      <TableCell>
                        <Chip size="small" label={status.label} color={status.color} />
                      </TableCell>
                      <TableCell>
                        <Typography variant="body2" fontWeight={600}>
                          {session.id}
                        </Typography>
                        <Typography variant="caption" color="text.secondary">
                          User {session.user_id}
                        </Typography>
                      </TableCell>
                      <TableCell>
                        <Typography variant="body2">
                          {session.primary_wallet}
                        </Typography>
                      </TableCell>
                      <TableCell>
                        <Typography variant="body2" fontFamily="monospace">
                          {session.wallet_address}
                        </Typography>
                        <Typography variant="caption" color="text.secondary">
                          Wallet ID: {session.wallet_id}
                        </Typography>
                      </TableCell>
                      <TableCell>{formatDate(session.created_at)}</TableCell>
                      <TableCell>{formatDate(session.refreshed_at)}</TableCell>
                      <TableCell>{formatDate(session.expires_at)}</TableCell>
                      <TableCell align="right">
                        <Button
                          size="small"
                          color="secondary"
                          disabled={disableRevoke || revokingId === session.id}
                          onClick={() => revoke(session.id)}
                          startIcon={
                            revokingId === session.id ? (
                              <CircularProgress size={14} />
                            ) : undefined
                          }
                        >
                          撤銷
                        </Button>
                      </TableCell>
                    </TableRow>
                  );
                })}
                {sessions.length === 0 && (
                  <TableRow>
                    <TableCell colSpan={8}>
                      <Typography color="text.secondary">
                        尚無任何 session 紀錄。
                      </Typography>
                    </TableCell>
                  </TableRow>
                )}
              </TableBody>
            </Table>
          </CardContent>
        </Card>
      </Container>
    </Box>
  );
}
