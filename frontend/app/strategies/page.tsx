"use client";

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
  Typography,
} from "@mui/material";
import {
  AutoGraph,
  Info,
  ManageSearch,
  PlayArrow,
  Timeline,
  UploadFile,
} from "@mui/icons-material";
import { ChangeEvent, FormEvent, MouseEvent, useEffect, useMemo, useState } from "react";
import { useProfile } from "../../lib/auth-context";
import {
  backtestStrategy,
  createStrategy,
  deleteStrategy,
  fetchStrategies,
  fetchStrategyBacktests,
} from "../../lib/api";
import { useRouter } from "next/navigation";
import { useI18n } from "../../lib/i18n";
import { EquityCurveChart } from "../../components/charts/EquityCurveChart";

type Strategy = {
  id: string;
  user_id: string;
  name: string;
  type: string;
  params: Record<string, unknown>;
};

type BacktestResult = {
  strategy_id: string;
  equity_curve: [string, number][];
  metrics: Record<string, unknown>;
  completed_at?: string | null;
};

export default function StrategiesPage() {
  const { profile, loading } = useProfile();
  const router = useRouter();
  const { t } = useI18n();
  const [strategies, setStrategies] = useState<Strategy[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [form, setForm] = useState({
    name: "",
    type: "MA_CROSS",
    short: 5,
    long: 20,
    lookback: 20,
    lag: 5,
    symbol: "ETH",
    days: 30,
  });
  const [backtestResult, setBacktestResult] = useState<BacktestResult | null>(null);
  const [backtestHistory, setBacktestHistory] = useState<BacktestResult[]>([]);
  const [historyLoading, setHistoryLoading] = useState(false);
  const [selectedStrategyId, setSelectedStrategyId] = useState<string | null>(null);
  const [runningId, setRunningId] = useState<string | null>(null);
  const [csvPrices, setCsvPrices] = useState<{ timestamp: string; price: number }[] | null>(null);
  const [guideOpen, setGuideOpen] = useState(true);
  const presetSymbols = ["ETH", "WETH", "USDC"];
  const [deletingId, setDeletingId] = useState<string | null>(null);

  const handleCsvUpload = (e: ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      const text = reader.result as string;
      const lines = text.split(/\r?\n/).filter(Boolean);
      const parsed: { timestamp: string; price: number }[] = [];
      for (const line of lines) {
        const [ts, price] = line.split(",");
        const num = Number(price);
        if (!ts || Number.isNaN(num)) continue;
        parsed.push({ timestamp: new Date(ts).toISOString(), price: num });
      }
      setCsvPrices(parsed.length ? parsed : null);
    };
    reader.readAsText(file);
  };

  useEffect(() => {
    if (!loading && !profile) {
      router.replace("/login");
    }
  }, [loading, profile, router]);

  useEffect(() => {
    fetchStrategies()
      .then(async (res) => {
        if (!res.ok) throw new Error(`載入策略失敗 (${res.status})`);
        const data = await res.json();
        setStrategies(data);
        if (data.length > 0) {
          setSelectedStrategyId(data[0].id);
          applyStrategyToForm(data[0]);
          loadBacktests(data[0].id);
        }
      })
      .catch((err) => setError(err instanceof Error ? err.message : "未知錯誤"));
  }, []);

  const loadBacktests = async (strategyId: string) => {
    setHistoryLoading(true);
    try {
      const res = await fetchStrategyBacktests(strategyId, 5);
      if (res.ok) {
        const data: BacktestResult[] = await res.json();
        setBacktestHistory(data);
      }
    } finally {
      setHistoryLoading(false);
    }
  };

  const applyStrategyToForm = (strategy: Strategy) => {
    const type = strategy.type.toUpperCase();
    setForm((prev) => ({
      ...prev,
      type,
      short: Number(strategy.params?.short_window ?? prev.short) || 5,
      long: Number(strategy.params?.long_window ?? prev.long) || 20,
      lookback: Number(strategy.params?.lookback ?? prev.lookback) || 20,
      lag: Number(strategy.params?.lag ?? prev.lag) || 5,
    }));
  };

  const submitStrategy = async (e: FormEvent) => {
    e.preventDefault();
    setError(null);
    try {
      const res = await createStrategy({
        name: form.name,
        type: form.type,
        params: {
          short_window: form.short,
          long_window: form.long,
          lookback: form.lookback,
          lag: form.lag,
        },
      });
      if (!res.ok) throw new Error(`建立策略失敗 (${res.status})`);
      const data = await res.json();
      setStrategies((prev) => [data, ...prev]);
      setSelectedStrategyId(data.id);
      applyStrategyToForm(data);
      loadBacktests(data.id);
      setForm({
        name: "",
        type: "MA_CROSS",
        short: 5,
        long: 20,
        lookback: 20,
        lag: 5,
        symbol: "ETH",
        days: 30,
      });
    } catch (err) {
      setError(err instanceof Error ? err.message : "未知錯誤");
    }
  };

  const handleDelete = async (strategyId: string) => {
    setDeletingId(strategyId);
    setError(null);
    try {
      const res = await deleteStrategy(strategyId);
      if (!res.ok) throw new Error(`刪除策略失敗 (${res.status})`);
      setStrategies((prev) => prev.filter((s) => s.id !== strategyId));
      if (backtestResult?.strategy_id === strategyId) {
        setBacktestResult(null);
      }
      if (selectedStrategyId === strategyId) {
        setSelectedStrategyId(null);
        setBacktestHistory([]);
      }
    } catch (err) {
      setError(err instanceof Error ? err.message : "未知錯誤");
    } finally {
      setDeletingId(null);
    }
  };

  const runBacktest = async (strategyId: string) => {
    const strategy = strategies.find((s) => s.id === strategyId);
    if (!strategy) {
      setError("請先選擇或建立策略");
      return;
    }
    setRunningId(strategyId);
    setSelectedStrategyId(strategyId);
    setError(null);
    try {
      const payload: any = {};
      if (csvPrices && csvPrices.length > 0) {
        payload.prices = csvPrices.map((p) => ({ timestamp: p.timestamp, price: p.price }));
      } else {
        payload.symbol = form.symbol;
        payload.days = form.days;
      }
      const type = strategy.type.toUpperCase();
      payload.type = type;
      if (type === "MA_CROSS") {
        payload.short_window = form.short;
        payload.long_window = form.long;
      }
      if (type === "VOLATILITY") {
        payload.lookback = form.lookback;
      }
      if (type === "CORRELATION") {
        payload.lag = form.lag;
      }
      const res = await backtestStrategy(strategyId, payload);
      if (!res.ok) throw new Error(`回測失敗 (${res.status})`);
      const data = await res.json();
      setBacktestResult(data);
      loadBacktests(strategyId);
    } catch (err) {
      setError(err instanceof Error ? err.message : "未知錯誤");
    } finally {
      setRunningId(null);
    }
  };

  return (
    <Container sx={{ py: 4 }}>
      <Stack
        direction={{ xs: "column", md: "row" }}
        alignItems={{ xs: "flex-start", md: "center" }}
        justifyContent="space-between"
        spacing={2}
      >
        <Box>
          <Typography variant="h4" fontWeight={800} gutterBottom>
            策略與回測中心
          </Typography>
          <Typography variant="body2" color="text.secondary">
            建立策略、匯入價格、立即回測並查看風險指標。
          </Typography>
        </Box>
        <Stack direction="row" spacing={1} flexWrap="wrap">
          <Chip label="實價來源: Coingecko" variant="outlined" />
          <Chip label="CSV 匯入" variant="outlined" color={csvPrices ? "success" : "default"} />
          <Chip label="MA / Vol / Corr" variant="outlined" />
        </Stack>
      </Stack>

      {guideOpen && (
        <Paper
          sx={{
            mt: 2,
            p: 2,
            borderRadius: 2,
            border: "1px solid rgba(255,255,255,0.08)",
          }}
        >
          <Stack direction={{ xs: "column", md: "row" }} spacing={2}>
            <Box flex={1}>
              <Typography variant="subtitle1" fontWeight={700} gutterBottom>
                使用手冊
              </Typography>
              <Typography variant="body2" color="text.secondary" sx={{ whiteSpace: "pre-line" }}>
                1) 建立策略：選擇類型並填入參數。MA 需短/長均線；Vol 需 lookback；Corr 需 lag。
{"\n"}2) 回測：可選預置資產+天數抓實價，或匯入 CSV（timestamp,price）。
{"\n"}3) 結果：查看總報酬、年化、最大回撤、Sharpe；右側 JSON 為後端指標。
{"\n"}4) 如果抓不到價格：檢查後端 price oracle/Coingecko；CSV 需有效時間格式。
              </Typography>
            </Box>
            <Button
              variant="outlined"
              startIcon={<Info />}
              onClick={() => setGuideOpen(false)}
              sx={{ alignSelf: "flex-start" }}
            >
              關閉指南
            </Button>
          </Stack>
        </Paper>
      )}

      <Grid container spacing={3} sx={{ mt: 2 }}>
        <Grid item xs={12} md={4}>
          <Card>
            <CardContent>
              <Stack direction="row" spacing={1} alignItems="center">
                <AutoGraph color="primary" />
                <Typography variant="subtitle1" fontWeight={700}>
                  建立策略
                </Typography>
              </Stack>
              <Divider sx={{ my: 1.5 }} />
              <Box
                component="form"
                onSubmit={submitStrategy}
                sx={{
                  display: "grid",
                  gap: 1.25,
                }}
              >
                <Typography variant="overline" color="text.secondary" sx={{ letterSpacing: 0.6 }}>
                  策略參數
                </Typography>
                <Box
                  sx={{
                    display: "grid",
                    gap: 1.25,
                    gridTemplateColumns: { xs: "1fr", sm: "1fr 1fr" },
                  }}
                >
                  <TextField
                    label="策略名稱"
                    value={form.name}
                    onChange={(e) => setForm((f) => ({ ...f, name: e.target.value }))}
                    required
                    sx={{ gridColumn: { xs: "span 1", sm: "span 2" } }}
                  />
                  <TextField
                    select
                    fullWidth
                    label="策略類型"
                    value={form.type}
                    onChange={(e) => setForm((f) => ({ ...f, type: e.target.value }))}
                  >
                    <MenuItem value="MA_CROSS">MA Cross</MenuItem>
                    <MenuItem value="VOLATILITY">Volatility</MenuItem>
                    <MenuItem value="CORRELATION">Correlation</MenuItem>
                  </TextField>
                  {form.type === "MA_CROSS" && (
                    <>
                      <TextField
                        label={t("strategies.short")}
                        type="number"
                        value={form.short}
                        onChange={(e) => setForm((f) => ({ ...f, short: Number(e.target.value) }))}
                        fullWidth
                      />
                      <TextField
                        label={t("strategies.long")}
                        type="number"
                        value={form.long}
                        onChange={(e) => setForm((f) => ({ ...f, long: Number(e.target.value) }))}
                        fullWidth
                      />
                    </>
                  )}
                  {form.type === "VOLATILITY" && (
                    <TextField
                      label="Lookback"
                      type="number"
                      value={form.lookback}
                      onChange={(e) => setForm((f) => ({ ...f, lookback: Number(e.target.value) }))}
                      fullWidth
                      sx={{ gridColumn: { xs: "span 1", sm: "span 2" } }}
                    />
                  )}
                  {form.type === "CORRELATION" && (
                    <TextField
                      label="Lag"
                      type="number"
                      value={form.lag}
                      onChange={(e) => setForm((f) => ({ ...f, lag: Number(e.target.value) }))}
                      fullWidth
                      sx={{ gridColumn: { xs: "span 1", sm: "span 2" } }}
                    />
                  )}
                </Box>

                <Divider sx={{ mt: 0.5 }} />
                <Typography variant="overline" color="text.secondary" sx={{ letterSpacing: 0.6 }}>
                  回測資料來源
                </Typography>
                <Box
                  sx={{
                    display: "grid",
                    gap: 1.25,
                    gridTemplateColumns: { xs: "1fr", sm: "1fr 1fr" },
                    alignItems: "center",
                  }}
                >
                  <TextField
                    select
                    label="預置資產"
                    value={form.symbol}
                    onChange={(e) => setForm((f) => ({ ...f, symbol: e.target.value }))}
                  >
                    {presetSymbols.map((s) => (
                      <MenuItem key={s} value={s}>
                        {s}
                      </MenuItem>
                    ))}
                  </TextField>
                  <TextField
                    label="天數"
                    type="number"
                    value={form.days}
                    onChange={(e) => setForm((f) => ({ ...f, days: Number(e.target.value) }))}
                  />
                  <Button
                    component="label"
                    variant="outlined"
                    size="small"
                    color={csvPrices ? "success" : "primary"}
                    startIcon={<UploadFile />}
                    sx={{ justifyContent: "flex-start" }}
                  >
                    {csvPrices ? "已選 CSV" : "匯入 CSV"}
                    <input
                      hidden
                      accept=".csv"
                      type="file"
                      onChange={(e) => handleCsvUpload(e)}
                    />
                  </Button>
                  <Typography variant="caption" color="text.secondary">
                    CSV: timestamp,price，會覆蓋預置資產價格。
                  </Typography>
                </Box>
                <Button type="submit" variant="contained" fullWidth>
                  {t("strategies.create")}
                </Button>
              </Box>
              {error && (
                <Alert severity="error" sx={{ mt: 2 }}>
                  {error}
                </Alert>
              )}
            </CardContent>
          </Card>
        </Grid>

        <Grid item xs={12} md={8}>
          <Card sx={{ mb: 2 }}>
            <CardContent>
              <Stack direction="row" spacing={1} alignItems="center">
                <Timeline color="primary" />
                <Typography variant="subtitle1" fontWeight={700}>
                  策略列表
                </Typography>
              </Stack>
              <Divider sx={{ my: 1.5 }} />
              <Table size="small">
                <TableHead>
                  <TableRow>
                    <TableCell>名稱</TableCell>
                    <TableCell>類型 / 參數</TableCell>
                    <TableCell align="right">操作</TableCell>
                  </TableRow>
                </TableHead>
                <TableBody>
                  {strategies.map((s) => (
                    <TableRow
                      key={s.id}
                      hover
                      selected={selectedStrategyId === s.id}
                      sx={{ cursor: "pointer" }}
                      onClick={() => {
                        setSelectedStrategyId(s.id);
                        loadBacktests(s.id);
                        applyStrategyToForm(s);
                      }}
                    >
                      <TableCell>{s.name}</TableCell>
                      <TableCell>
                        {s.type === "MA_CROSS"
                          ? `MA(${(s.params?.short_window as number) ?? "-"}, ${(s.params?.long_window as number) ?? "-"})`
                          : s.type === "VOLATILITY"
                            ? `Vol lookback ${(s.params?.lookback as number) ?? "-"}`
                            : `Corr lag ${(s.params?.lag as number) ?? "-"}`}
                      </TableCell>
                      <TableCell align="right">
                        <Button
                          size="small"
                          startIcon={<PlayArrow />}
                          onClick={(e: MouseEvent) => {
                            e.stopPropagation();
                            runBacktest(s.id);
                          }}
                          disabled={runningId === s.id}
                        >
                          {runningId === s.id ? t("strategies.running") : t("strategies.run")}
                        </Button>
                        <Button
                          size="small"
                          color="secondary"
                          sx={{ ml: 1 }}
                          onClick={(e: MouseEvent) => {
                            e.stopPropagation();
                            setSelectedStrategyId(s.id);
                            loadBacktests(s.id);
                          }}
                        >
                          歷史
                        </Button>
                        <Button
                          size="small"
                          color="error"
                          sx={{ ml: 1 }}
                          onClick={(e: MouseEvent) => {
                            e.stopPropagation();
                            handleDelete(s.id);
                          }}
                          disabled={deletingId === s.id}
                        >
                          {deletingId === s.id ? "刪除中..." : "刪除"}
                        </Button>
                      </TableCell>
                    </TableRow>
                  ))}
                  {strategies.length === 0 && (
                    <TableRow>
                      <TableCell colSpan={3}>{t("dashboard.no_data")}</TableCell>
                    </TableRow>
                  )}
                </TableBody>
              </Table>
            </CardContent>
          </Card>

          <Card>
            <CardContent>
              <Stack direction="row" spacing={1} alignItems="center">
                <ManageSearch color="secondary" />
                <Typography variant="subtitle1" fontWeight={700}>
                  回測結果
                </Typography>
              </Stack>
              <Divider sx={{ my: 1.5 }} />
              <Stack spacing={2}>
                <Card variant="outlined">
                  <CardContent>
                    <Stack direction="row" justifyContent="space-between" alignItems="center">
                      <Typography variant="subtitle2">最近回測</Typography>
                      <Typography variant="caption" color="text.secondary">
                        {historyLoading
                          ? "載入中..."
                          : selectedStrategyId
                            ? `策略 ${selectedStrategyId.slice(0, 6)}`
                            : "未選擇策略"}
                      </Typography>
                    </Stack>
                    {backtestHistory.length === 0 ? (
                      <Typography variant="body2" color="text.secondary" sx={{ mt: 1 }}>
                        尚無回測記錄，從列表點「回測」或「歷史」載入。
                      </Typography>
                    ) : (
                      <Table size="small" sx={{ mt: 1 }}>
                        <TableHead>
                          <TableRow>
                            <TableCell>完成時間</TableCell>
                            <TableCell>總報酬</TableCell>
                            <TableCell>指標</TableCell>
                          </TableRow>
                        </TableHead>
                        <TableBody>
                          {backtestHistory.map((r) => {
                            const summary = summarizeResult(r);
                            return (
                              <TableRow key={`${r.strategy_id}-${summary.completedAt ?? "na"}`}>
                                <TableCell>
                                  {summary.completedAt
                                    ? new Date(summary.completedAt).toLocaleString("zh-TW")
                                    : "—"}
                                </TableCell>
                                <TableCell>
                                  <Chip
                                    size="small"
                                    color={summary.totalReturn >= 0 ? "success" : "error"}
                                    label={`${summary.totalReturn >= 0 ? "+" : ""}${(
                                      summary.totalReturn * 100
                                    ).toFixed(2)}%`}
                                  />
                                </TableCell>
                                <TableCell>
                                  <Typography variant="body2" color="text.secondary">
                                    {summary.label}
                                  </Typography>
                                </TableCell>
                              </TableRow>
                            );
                          })}
                        </TableBody>
                      </Table>
                    )}
                  </CardContent>
                </Card>
                {backtestResult ? (
                  <EquityCurveChart
                    equityCurve={backtestResult.equity_curve}
                    metrics={backtestResult.metrics}
                  />
                ) : (
                  <Typography variant="body2" color="text.secondary">
                    尚未回測，從列表點「回測」開始。
                  </Typography>
                )}
              </Stack>
            </CardContent>
          </Card>
        </Grid>
      </Grid>
    </Container>
  );
}

function summarizeResult(result: BacktestResult) {
  const curve = result.equity_curve
    .map(([ts, v]) => ({ ts: new Date(ts), v: Number(v) }))
    .filter((p) => !Number.isNaN(p.v) && Number.isFinite(p.v))
    .sort((a, b) => a.ts.getTime() - b.ts.getTime());
  const start = curve[0]?.v ?? 0;
  const end = curve[curve.length - 1]?.v ?? start;
  const totalReturn =
    start > 0 ? end / start - 1 : (result.metrics?.total_return as number | undefined) ?? 0;
  const label = result.metrics?.type
    ? `策略 ${String(result.metrics.type)}`
    : "回測";
  return { totalReturn, completedAt: result.completed_at, label };
}
