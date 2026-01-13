"use client";

import {
  Logout,
  ManageAccounts,
  Notifications,
  Refresh,
  TrendingUp,
  VerifiedUser,
} from "@mui/icons-material";
import {
  Alert,
  Box,
  Button,
  Card,
  CardContent,
  Chip,
  Container,
  Divider,
  Grid,
  MenuItem,
  Paper,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  TextField,
  Tooltip,
  Typography,
} from "@mui/material";
import { useRouter } from "next/navigation";
import { FormEvent, useEffect, useMemo, useState } from "react";
import { AssetDistribution } from "../../components/charts/AssetDistribution";
import { PortfolioSparkline } from "../../components/charts/PortfolioSparkline";
import { useAlertTriggers } from "../../lib/alerts-hooks";
import { refreshAdminRoles } from "../../lib/api";
import { useAuth, useProfile } from "../../lib/auth-context";
import { useI18n } from "../../lib/i18n";
import { usePortfolio, usePortfolioSnapshots, useWallets } from "../../lib/portfolio-hooks";

export default function DashboardPage() {
  const router = useRouter();
  const { profile, loading: profileLoading } = useProfile();
  const { signOut, refreshProfile } = useAuth();
  const { t } = useI18n();
  const {
    wallets,
    loading: walletsLoading,
    error: walletsError,
    refresh: refreshWallets,
    addWallet,
    setPrimary,
    deleteWallet,
    actionLoading: walletActionLoading,
  } = useWallets();
  const [selectedWalletId, setSelectedWalletId] = useState<string | null>(null);
  const [formAddress, setFormAddress] = useState("");
  const [formChainId, setFormChainId] = useState(31337);
  const [formError, setFormError] = useState<string | null>(null);
  const [settingPrimary, setSettingPrimary] = useState<string | null>(null);
  const [roleRefreshMsg, setRoleRefreshMsg] = useState<string | null>(null);
  const [roleRefreshLoading, setRoleRefreshLoading] = useState(false);
  const [chartMode, setChartMode] = useState<"tvl" | "flow" | "return">("tvl");
  const [chainFilter, setChainFilter] = useState<number | "all">("all");
  const {
    triggers,
    loading: alertsLoading,
    error: alertsError,
    refresh: refreshAlerts,
  } = useAlertTriggers(20);

  const shortAddress = (addr: string) =>
    addr.length > 12 ? `${addr.slice(0, 6)}…${addr.slice(-4)}` : addr;

  const {
    snapshot,
    loading: portfolioLoading,
    error: portfolioError,
    refresh: refreshPortfolio,
  } = usePortfolio(selectedWalletId);
  const {
    snapshots,
    loading: snapshotsLoading,
    error: snapshotsError,
    refresh: refreshSnapshots,
  } = usePortfolioSnapshots(selectedWalletId);

  const sortedSnapshots = useMemo(
    () =>
      [...snapshots].sort(
        (a, b) => new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime()
      ),
    [snapshots]
  );

  useEffect(() => {
    if (!profileLoading && !profile) {
      router.replace("/login");
    }
  }, [profileLoading, profile, router]);

  useEffect(() => {
    if (!selectedWalletId && wallets.length > 0) {
      setSelectedWalletId(wallets[0].id);
    }
  }, [wallets, selectedWalletId]);

  const filteredWallets = useMemo(
    () =>
      chainFilter === "all"
        ? wallets
        : wallets.filter((w) => w.chain_id === chainFilter),
    [chainFilter, wallets]
  );

  useEffect(() => {
    if (!selectedWalletId && filteredWallets.length > 0) {
      setSelectedWalletId(filteredWallets[0].id);
    } else if (
      selectedWalletId &&
      filteredWallets.length > 0 &&
      filteredWallets.every((w) => w.id !== selectedWalletId)
    ) {
      setSelectedWalletId(filteredWallets[0]?.id ?? null);
    }
  }, [filteredWallets, selectedWalletId]);

  const statusMessage = useMemo(() => {
    if ((walletsLoading || portfolioLoading) && !snapshot) {
      return t("common.loading");
    }
    if (walletsError) {
      return `${t("dashboard.no_wallet")}${walletsError}`;
    }
    if (portfolioError) {
      return `${t("dashboard.asset_overview")}錯誤：${portfolioError}`;
    }
    if (snapshotsError) {
      return `快照載入錯誤：${snapshotsError}`;
    }
    if (!selectedWalletId) {
      return t("dashboard.no_wallet");
    }
    if (!snapshot) {
      return t("common.loading");
    }
    if (!snapshotsLoading && sortedSnapshots.length === 0) {
      return "尚無快照，索引約每 15 分鐘更新一次。";
    }
    return null;
  }, [
    walletsLoading,
    portfolioLoading,
    walletsError,
    portfolioError,
    snapshotsError,
    selectedWalletId,
    snapshot,
    snapshotsLoading,
    sortedSnapshots.length,
    t,
  ]);

  const historyValues = useMemo(() => {
    const values = sortedSnapshots.map((h) => h.total_usd_value);
    if (values.length === 0 && snapshot) {
      values.push(snapshot.total_usd_value);
    }
    if (chartMode === "tvl") {
      return { label: t("dashboard.asset_trend"), values, unit: "$" };
    }
    if (chartMode === "flow") {
      const flows: number[] = [];
      let acc = 0;
      for (let i = 1; i < values.length; i++) {
        acc += values[i] - values[i - 1];
        flows.push(acc);
      }
      return {
        label: "淨流入累計",
        values: flows.length ? flows : [0],
        unit: "$",
      };
    }
    const base = values[0] || 1;
    const returns = values.map((v) => (v / base - 1) * 100);
    return { label: "報酬率 %", values: returns, unit: "%" };
  }, [chartMode, sortedSnapshots, snapshot, t]);

  const latestSnapshot = useMemo(() => {
    if (sortedSnapshots.length > 0) {
      return sortedSnapshots[sortedSnapshots.length - 1];
    }
    return snapshot ?? null;
  }, [sortedSnapshots, snapshot]);

  const prevSnapshot = useMemo(() => {
    if (sortedSnapshots.length > 1) {
      return sortedSnapshots[sortedSnapshots.length - 2];
    }
    return null;
  }, [sortedSnapshots]);

  const usdDelta = useMemo(() => {
    if (!latestSnapshot || !prevSnapshot) return null;
    return latestSnapshot.total_usd_value - prevSnapshot.total_usd_value;
  }, [latestSnapshot, prevSnapshot]);

  const hasWallets = wallets.length > 0;

  const handleSignOut = async () => {
    await signOut();
    router.replace("/login");
  };

  const handleAddWallet = async (event: FormEvent<HTMLFormElement>) => {
    event.preventDefault();
    setFormError(null);
    try {
      const wallet = await addWallet({ address: formAddress, chainId: formChainId });
      setFormAddress("");
      setFormChainId(formChainId);
      setSelectedWalletId(wallet.id);
      await refreshPortfolio();
    } catch (err) {
      setFormError(err instanceof Error ? err.message : "新增錢包失敗");
    }
  };

  const handleSetPrimary = async (walletId: string) => {
    setSettingPrimary(walletId);
    try {
      await setPrimary(walletId);
      // primary_wallet 在 profile 內，因此讀取最新 profile 並刷新列表
      await Promise.all([refreshProfile({ silent: true }), refreshWallets()]);
      setSelectedWalletId(walletId);
    } catch (err) {
      setFormError(err instanceof Error ? err.message : "設定主錢包失敗");
    } finally {
      setSettingPrimary(null);
    }
  };

  const handleRoleRefresh = async () => {
    setRoleRefreshMsg(null);
    setRoleRefreshLoading(true);
    try {
      const res = await refreshAdminRoles();
      if (!res.ok) {
        throw new Error(`刷新角色快取失敗 (${res.status})`);
      }
      const data = await res.json();
      setRoleRefreshMsg(`已刷新 ${data.refreshed?.length ?? 0} 個錢包角色`);
    } catch (err) {
      setRoleRefreshMsg(
        err instanceof Error ? err.message : "刷新角色快取失敗"
      );
    } finally {
      setRoleRefreshLoading(false);
    }
  };

  const positions = snapshot?.positions ?? [];

  if (!profile && profileLoading) {
    return null;
  }

  return (
    <Box
      sx={{
        minHeight: "100vh",
        py: 6,
        background:
          "radial-gradient(circle at top, rgba(97,218,251,0.12), transparent 40%), #05070c",
      }}
    >
      <Container maxWidth="lg">
        <Stack
          direction={{ xs: "column", sm: "row" }}
          alignItems={{ xs: "flex-start", sm: "center" }}
          justifyContent="space-between"
          spacing={2}
          mb={4}
        >
          <Box>
            <Typography variant="h4" fontWeight={700}>
              {t("dashboard.title")}
            </Typography>
            <Typography variant="body2" color="text.secondary">
              {t("dashboard.subtitle")}
            </Typography>
          </Box>
          <Paper
            sx={{
              p: 1.5,
              borderRadius: 2,
              border: "1px solid rgba(255,255,255,0.08)",
              bgcolor: "rgba(255,255,255,0.02)",
              boxShadow: "0 12px 34px rgba(0,0,0,0.28)",
            }}
          >
            <Stack
              direction={{ xs: "column", md: "row" }}
              spacing={1}
              rowGap={1}
              flexWrap="wrap"
              alignItems={{ xs: "flex-start", md: "center" }}
            >
              <Chip
                size="small"
                label="Price: 後端實價 (Coingecko/靜態備援)"
                variant="outlined"
              />
              {profile?.role === "Admin" && (
                <Button
                  variant="outlined"
                  color="secondary"
                  startIcon={<ManageAccounts />}
                  sx={{ whiteSpace: "nowrap", px: 2.5 }}
                  onClick={() => router.push("/admin/sessions")}
                >
                  {t("dashboard.manage_sessions")}
                </Button>
              )}
              {profile?.role === "Admin" && (
                <Button
                  variant="outlined"
                  startIcon={<VerifiedUser />}
                  sx={{ whiteSpace: "nowrap", px: 2.5 }}
                  onClick={handleRoleRefresh}
                  disabled={roleRefreshLoading}
                >
                  {roleRefreshLoading
                    ? t("dashboard.refresh_role_running")
                    : t("dashboard.refresh_role")}
                </Button>
              )}
              <Button
                variant="outlined"
                startIcon={<TrendingUp />}
                sx={{ whiteSpace: "nowrap", px: 2.5 }}
                onClick={() => router.push("/strategies")}
              >
                {t("dashboard.strategies")}
              </Button>
              <Button
                variant="outlined"
                startIcon={<Notifications />}
                sx={{ whiteSpace: "nowrap", px: 2.5 }}
                onClick={() => router.push("/alerts")}
              >
                {t("dashboard.alerts")}
              </Button>
              {profile?.role === "Admin" && (
                <Button
                  variant="outlined"
                  startIcon={<ManageAccounts />}
                  sx={{ whiteSpace: "nowrap", px: 2.5 }}
                  onClick={() => router.push("/admin/users")}
                >
                  {t("dashboard.admin_users")}
                </Button>
              )}
              <Button
                variant="outlined"
                startIcon={<Refresh />}
                sx={{ whiteSpace: "nowrap", px: 2.5 }}
                onClick={() => {
                  refreshWallets();
                  refreshPortfolio();
                  refreshSnapshots();
                }}
              >
                {t("common.refresh")}
              </Button>
              <Button
                color="secondary"
                startIcon={<Logout />}
                sx={{ whiteSpace: "nowrap", px: 2.5 }}
                onClick={handleSignOut}
              >
                {t("dashboard.logout")}
              </Button>
            </Stack>
          </Paper>
        </Stack>
        <Stack direction={{ xs: "column", sm: "row" }} spacing={1.5} mb={2}>
          <TextField
            select
            size="small"
            label="鏈別"
            value={chainFilter}
            onChange={(e) =>
              setChainFilter(
                e.target.value === "all" ? "all" : Number(e.target.value)
              )
            }
            sx={{ minWidth: 140 }}
          >
            <MenuItem value="all">全部</MenuItem>
            {[...new Set(wallets.map((w) => w.chain_id))].map((cid) => (
              <MenuItem key={cid} value={cid}>
                Chain {cid}
              </MenuItem>
            ))}
          </TextField>
          <Stack direction="row" spacing={1}>
            <Chip
              label="TVL"
              color={chartMode === "tvl" ? "primary" : "default"}
              onClick={() => setChartMode("tvl")}
              variant={chartMode === "tvl" ? "filled" : "outlined"}
            />
            <Chip
              label="淨流入"
              color={chartMode === "flow" ? "primary" : "default"}
              onClick={() => setChartMode("flow")}
              variant={chartMode === "flow" ? "filled" : "outlined"}
            />
            <Chip
              label="報酬率"
              color={chartMode === "return" ? "primary" : "default"}
              onClick={() => setChartMode("return")}
              variant={chartMode === "return" ? "filled" : "outlined"}
            />
          </Stack>
        </Stack>

        {profile && (
          <Card sx={{ mb: 4 }}>
            <CardContent>
              <Typography variant="h6" gutterBottom>
                {t("dashboard.asset_overview")}，{profile.primary_wallet}
              </Typography>
              <Stack direction="row" spacing={2}>
                <Chip label={`User ID: ${profile.id}`} variant="outlined" />
                <Chip
                  label={profile.role === "Admin" ? "Admin" : profile.role}
                  sx={{
                    background:
                      profile.role === "Admin"
                        ? "linear-gradient(120deg, rgba(255,193,7,0.22), rgba(255,255,255,0.08))"
                        : "transparent",
                    borderColor: "rgba(255,255,255,0.18)",
                    color: profile.role === "Admin" ? "#f7c843" : "inherit",
                    fontWeight: 700,
                    letterSpacing: 0.5,
                    textTransform: "uppercase",
                  }}
                  variant="outlined"
                />
              </Stack>
            </CardContent>
          </Card>
        )}

        <Grid container spacing={3}>
          <Grid item xs={12} md={4}>
            <Card>
              <CardContent>
                <Typography variant="h6" gutterBottom>
                  {t("wallets.title")}
                </Typography>
                <Box
                  sx={{
                    display: "grid",
                    gridTemplateColumns: {
                      xs: "1fr",
                      sm: "repeat(auto-fit, minmax(260px, 1fr))",
                    },
                    gap: 1.5,
                  }}
                >
                  {filteredWallets.map((wallet) => {
                    const active = wallet.id === selectedWalletId;
                    const isPrimary =
                      profile?.primary_wallet?.toLowerCase() ===
                      wallet.address.toLowerCase();
                    return (
                      <Card
                        key={wallet.id}
                        variant="outlined"
                        sx={{
                          cursor: "pointer",
                          borderColor: active
                            ? "primary.main"
                            : "rgba(255,255,255,0.12)",
                          background: active
                            ? "linear-gradient(120deg, rgba(97,218,251,0.12), rgba(0,0,0,0))"
                            : "transparent",
                          transition:
                            "transform 120ms ease, border-color 120ms ease, box-shadow 120ms ease",
                          "&:hover": {
                            transform: "translateY(-2px)",
                            boxShadow: 3,
                          },
                        }}
                        onClick={() => {
                          setSelectedWalletId(wallet.id);
                          refreshPortfolio();
                        }}
                      >
                        <CardContent
                          sx={{
                            py: 1.5,
                            display: "flex",
                            flexDirection: "column",
                            gap: 1,
                          }}
                        >
                          <Stack
                            direction="row"
                            spacing={1}
                            alignItems="center"
                            flexWrap="wrap"
                          >
                            <Chip
                              size="small"
                              variant="outlined"
                              label={`Chain ${wallet.chain_id}`}
                              sx={{ borderStyle: "dashed" }}
                            />
                          </Stack>
                          <Stack spacing={0.5}>
                            <Typography
                              variant="subtitle2"
                              fontWeight={800}
                              fontFamily="monospace"
                              sx={{ letterSpacing: 0.4 }}
                            >
                              <Tooltip
                                title={wallet.address}
                                placement="top-start"
                              >
                                <span>{shortAddress(wallet.address)}</span>
                              </Tooltip>
                            </Typography>
                          </Stack>
                          <Stack direction="row" spacing={1} mt={0.5}>
                            {!isPrimary && (
                              <Button
                                size="small"
                                variant="contained"
                                onClick={(e) => {
                                  e.stopPropagation();
                                  handleSetPrimary(wallet.id);
                                }}
                                disabled={
                                  settingPrimary === wallet.id ||
                                  walletActionLoading
                                }
                              >
                                {settingPrimary === wallet.id
                                  ? t("common.setting")
                                  : t("common.set_primary")}
                              </Button>
                            )}
                            <Button
                              size="small"
                              color="error"
                              variant="outlined"
                              onClick={(e) => {
                                e.stopPropagation();
                                deleteWallet(wallet.id)
                                  .then(() => {
                                    if (wallet.id === selectedWalletId) {
                                      setSelectedWalletId(null);
                                      refreshPortfolio();
                                    }
                                  })
                                  .catch((err) =>
                                    setFormError(
                                      err instanceof Error
                                        ? err.message
                                        : "刪除失敗"
                                    )
                                  );
                              }}
                              disabled={walletActionLoading}
                            >
                              {t("common.delete")}
                            </Button>
                          </Stack>
                        </CardContent>
                      </Card>
                    );
                  })}
                  {!hasWallets && (
                    <Typography variant="body2" color="text.secondary">
                      {t("dashboard.no_wallet")}
                    </Typography>
                  )}
                </Box>
                <Divider sx={{ my: 2 }} />
                <Typography variant="subtitle2" gutterBottom>
                  {t("dashboard.add_wallet")}
                </Typography>
                <Box component="form" onSubmit={handleAddWallet}>
                  <Stack spacing={1.5}>
                    <TextField
                      size="small"
                      label={t("dashboard.address_placeholder")}
                      value={formAddress}
                      onChange={(e) => setFormAddress(e.target.value)}
                      required
                    />
                    <TextField
                      size="small"
                      type="number"
                      label={t("dashboard.chain_id")}
                      value={formChainId}
                      onChange={(e) =>
                        setFormChainId(Number.parseInt(e.target.value))
                      }
                      required
                    />
                    <Button type="submit" disabled={!formAddress}>
                      {t("dashboard.add_wallet")}
                    </Button>
                    {formError && (
                      <Alert severity="error" variant="outlined">
                        {formError}
                      </Alert>
                    )}
                  </Stack>
                </Box>
              </CardContent>
            </Card>
          </Grid>

          <Grid item xs={12} md={8}>
            <Card sx={{ mb: 3, minHeight: 220 }}>
              <CardContent>
                <Stack
                  direction={{ xs: "column", sm: "row" }}
                  justifyContent="space-between"
                  alignItems={{ xs: "flex-start", sm: "center" }}
                  spacing={1}
                >
                  <Typography variant="h6" gutterBottom>
                    資產概況
                  </Typography>
                  <Typography variant="caption" color="text.secondary">
                    數據每 15 分鐘更新
                  </Typography>
                </Stack>
                {statusMessage ? (
                  <Typography color="text.secondary">
                    {statusMessage}
                  </Typography>
                ) : (
                  snapshot && (
                    <>
                      <Typography variant="h3" fontWeight={700}>
                        ${snapshot.total_usd_value.toFixed(2)}
                      </Typography>
                      <Stack direction={{ xs: "column", sm: "row" }} spacing={1}>
                        <Typography variant="body2" color="text.secondary">
                          {t("dashboard.latest")}：
                          {new Date(snapshot.timestamp).toLocaleString("zh-TW")}
                        </Typography>
                        {usdDelta !== null && (
                          <Chip
                            size="small"
                            color={usdDelta >= 0 ? "success" : "error"}
                            label={`24h ${usdDelta >= 0 ? "+" : ""}${usdDelta.toFixed(
                              2
                            )}`}
                            sx={{ fontWeight: 700 }}
                          />
                        )}
                      </Stack>
                    </>
                  )
                )}
              </CardContent>
            </Card>
            <Box
              sx={{
                display: "grid",
                gridTemplateColumns: { xs: "1fr", md: "1fr 1fr" },
                gap: 2,
                mb: 2,
              }}
            >
              <Box sx={{ minHeight: 220 }}>
                <AssetDistribution positions={positions} />
              </Box>
              <Box sx={{ minHeight: 220 }}>
                {sortedSnapshots.length > 0 ? (
                  <PortfolioSparkline
                    label={historyValues.label}
                    values={historyValues.values}
                    unit={historyValues.unit}
                  />
                ) : (
                  <Typography variant="body2" color="text.secondary" sx={{ mt: 1 }}>
                    尚無快照，等待索引（約 15 分鐘內）後再試。
                  </Typography>
                )}
              </Box>
            </Box>
            {snapshot && (
              <Card>
                <CardContent>
                  <Typography variant="subtitle1" gutterBottom>
                    {t("dashboard.positions")}
                  </Typography>
                  <Box sx={{ overflowX: "auto" }}>
                    <Table size="small">
                      <TableHead>
                        <TableRow>
                          <TableCell>{t("dashboard.asset")}</TableCell>
                          <TableCell align="right">
                            {t("dashboard.amount")}
                          </TableCell>
                          <TableCell align="right">
                            {t("dashboard.usd")}
                          </TableCell>
                        </TableRow>
                      </TableHead>
                      <TableBody>
                        {positions.map((pos) => (
                          <TableRow key={pos.asset_symbol}>
                            <TableCell>{pos.asset_symbol}</TableCell>
                            <TableCell align="right">
                              {pos.amount.toFixed(6)}
                            </TableCell>
                            <TableCell align="right">
                              ${pos.usd_value.toFixed(2)}
                            </TableCell>
                          </TableRow>
                        ))}
                      </TableBody>
                    </Table>
                  </Box>
                </CardContent>
              </Card>
            )}
          </Grid>
        </Grid>
        <Grid container spacing={3} sx={{ mt: 1 }}>
          <Grid item xs={12}>
            <Card>
              <CardContent>
                <Stack
                  direction={{ xs: "column", sm: "row" }}
                  justifyContent="space-between"
                  alignItems={{ xs: "flex-start", sm: "center" }}
                  spacing={1.5}
                >
                  <Box>
                    <Typography variant="subtitle1" fontWeight={700}>
                      告警列表
                    </Typography>
                    <Typography variant="caption" color="text.secondary">
                      近 20 筆觸發紀錄
                    </Typography>
                  </Box>
                  <Button size="small" onClick={() => refreshAlerts()}>
                    重新整理
                  </Button>
                </Stack>
                {alertsError && (
                  <Alert severity="error" sx={{ mt: 1 }}>
                    {alertsError}
                  </Alert>
                )}
                <Box sx={{ overflowX: "auto" }}>
                  <Table size="small" sx={{ mt: 1, minWidth: 360 }}>
                    <TableHead>
                      <TableRow>
                        <TableCell>時間</TableCell>
                        <TableCell>錢包</TableCell>
                        <TableCell>訊息</TableCell>
                      </TableRow>
                    </TableHead>
                    <TableBody>
                      {triggers.map((t) => (
                        <TableRow key={t.id}>
                          <TableCell>
                            {new Date(t.created_at).toLocaleString()}
                          </TableCell>
                          <TableCell>{t.wallet_id.slice(0, 6)}…</TableCell>
                          <TableCell>{t.message}</TableCell>
                        </TableRow>
                      ))}
                      {!alertsLoading && triggers.length === 0 && (
                        <TableRow>
                          <TableCell colSpan={3}>尚無觸發紀錄</TableCell>
                        </TableRow>
                      )}
                    </TableBody>
                  </Table>
                </Box>
              </CardContent>
            </Card>
          </Grid>
        </Grid>
        {roleRefreshMsg && (
          <Box mt={3}>
            <Alert severity="info">{roleRefreshMsg}</Alert>
          </Box>
        )}
      </Container>
    </Box>
  );
}
