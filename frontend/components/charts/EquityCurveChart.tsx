"use client";

import { Box, Grid, Paper, Stack, Typography } from "@mui/material";
import { useMemo } from "react";
import { GlowingAreaChart } from "./GlowingAreaChart";

type Props = {
  equityCurve: [string, number][];
  metrics: Record<string, unknown>;
};

export function EquityCurveChart({ equityCurve, metrics }: Props) {
  const sorted = useMemo(
    () =>
      equityCurve
        .map(([ts, v]) => ({ ts: new Date(ts), value: Number(v) }))
        .filter((p) => !Number.isNaN(p.value) && Number.isFinite(p.value))
        .sort((a, b) => a.ts.getTime() - b.ts.getTime()),
    [equityCurve]
  );

  const values = sorted.map((p) => p.value);
  const start = values[0] ?? 0;
  const end = values[values.length - 1] ?? 0;
  const totalReturn = start > 0 ? end / start - 1 : 0;

  let peak = -Infinity;
  let maxDrawdown = 0;
  values.forEach((v) => {
    peak = Math.max(peak, v);
    if (peak > 0) {
      const dd = v / peak - 1;
      maxDrawdown = Math.min(maxDrawdown, dd);
    }
  });

  const days =
    sorted.length > 1
      ? Math.max(
          1,
          Math.round(
            (sorted[sorted.length - 1].ts.getTime() - sorted[0].ts.getTime()) /
              (1000 * 60 * 60 * 24)
          )
        )
      : 0;
  const cagr = start > 0 && days > 0 ? Math.pow(end / start, 365 / Math.max(days, 1)) - 1 : totalReturn;

  const quickStats = [
    { label: "總報酬", value: `${(totalReturn * 100).toFixed(2)}%` },
    { label: "最大回撤", value: `${Math.abs(maxDrawdown * 100).toFixed(2)}%` },
    { label: "年化報酬(估)", value: `${(cagr * 100).toFixed(2)}%` },
    { label: "期間", value: days ? `${days} 天` : "—" },
  ];

  const labelMap: Record<string, string> = {
    total_return: "總報酬",
    short_window: "短均線",
    long_window: "長均線",
    annualized_vol: "年化波動",
    lookback: "Lookback",
    lag: "Lag",
    correlation: "相關係數",
    type: "策略類型",
    sharpe: "Sharpe",
    cagr: "CAGR",
    max_drawdown: "最大回撤",
  };

  const metricEntries = Object.entries(metrics ?? {}).filter(([key]) => key !== "equity_curve");

  const startLabel = sorted[0]?.ts.toLocaleDateString("zh-TW") ?? "";
  const endLabel = sorted[sorted.length - 1]?.ts.toLocaleDateString("zh-TW") ?? "";

  return (
    <Stack spacing={2}>
      <GlowingAreaChart
        title="Equity Curve"
        subtitle={startLabel && endLabel ? `${startLabel} → ${endLabel}` : undefined}
        points={sorted.map((p) => ({
          value: p.value,
          label: p.ts.toLocaleString("zh-TW"),
        }))}
        unit=""
        baseline={start}
        height={300}
        viewportWidth={860}
        tags={[
          {
            label: `${totalReturn >= 0 ? "+" : ""}${(totalReturn * 100).toFixed(2)}%`,
            color: totalReturn >= 0 ? "success" : "error",
          },
          { label: `最大回撤 ${Math.abs(maxDrawdown * 100).toFixed(2)}%` },
          { label: days ? `${days} 天` : "—" },
        ]}
        valueFormatter={(v) => v.toFixed(3)}
      />

      <Grid container spacing={1.5}>
        {quickStats.map((item) => (
          <Grid item xs={6} md={3} key={item.label}>
            <Paper
              variant="outlined"
              sx={{
                p: 1.5,
                borderRadius: 2,
                background: "linear-gradient(135deg, rgba(255,255,255,0.04), rgba(0,0,0,0.25))",
                borderColor: "rgba(255,255,255,0.08)",
              }}
            >
              <Typography variant="caption" color="text.secondary">
                {item.label}
              </Typography>
              <Typography variant="subtitle1" fontWeight={700}>
                {item.value}
              </Typography>
            </Paper>
          </Grid>
        ))}
      </Grid>

      {metricEntries.length > 0 && (
        <Grid container spacing={1.2}>
          {metricEntries.map(([key, raw]) => {
            const label = labelMap[key] ?? key;
            const value =
              typeof raw === "number"
                ? Math.abs(raw) < 1 && raw !== 0
                  ? raw.toPrecision(3)
                  : raw.toFixed(2)
                : String(raw);
            return (
              <Grid item xs={12} sm={6} md={4} key={key}>
                <Paper
                  variant="outlined"
                  sx={{
                    p: 1.25,
                    borderRadius: 2,
                    background: "rgba(255,255,255,0.03)",
                    borderColor: "rgba(255,255,255,0.06)",
                  }}
                >
                  <Typography variant="caption" color="text.secondary">
                    {label}
                  </Typography>
                  <Typography variant="subtitle2" fontWeight={700}>
                    {value}
                  </Typography>
                </Paper>
              </Grid>
            );
          })}
        </Grid>
      )}

      {!metricEntries.length && (
        <Box sx={{ border: "1px dashed rgba(255,255,255,0.12)", p: 1.5, borderRadius: 2 }}>
          <Typography variant="body2" color="text.secondary">
            沒有額外指標。
          </Typography>
        </Box>
      )}
    </Stack>
  );
}
