"use client";

import { Box, Chip, Stack, Typography } from "@mui/material";
import { MouseEvent, useId, useMemo, useState } from "react";

type ChartPoint = {
  value: number;
  label?: string;
};

type ChartTag = {
  label: string;
  color?:
    | "default"
    | "primary"
    | "secondary"
    | "success"
    | "error"
    | "info"
    | "warning";
};

type TooltipInfo = {
  title?: string;
  value?: string;
  meta?: string;
};

type Props = {
  title: string;
  subtitle?: string;
  points: ChartPoint[];
  unit?: string;
  accent?: string;
  height?: number;
  viewportWidth?: number;
  baseline?: number;
  tags?: ChartTag[];
  emptyText?: string;
  valueFormatter?: (value: number) => string;
  tooltipFormatter?: (point: ChartPoint, idx: number) => TooltipInfo;
};

export function GlowingAreaChart({
  title,
  subtitle,
  points,
  unit = "",
  accent = "#10d7ff",
  height = 260,
  viewportWidth = 640,
  baseline,
  tags,
  emptyText = "暫無資料",
  valueFormatter,
  tooltipFormatter,
}: Props) {
  const gradientId = useId();
  const lineId = `${gradientId}-line`;
  const safePoints = points.filter((p) => Number.isFinite(p.value));
  const values = safePoints.map((p) => p.value);
  const hasData = values.length >= 2;

  const stats = useMemo(() => {
    if (!hasData) {
      return { min: 0, max: 1, start: 0, end: 0, change: 0 };
    }
    const min = Math.min(...values);
    const max = Math.max(...values);
    const start = values[0];
    const end = values[values.length - 1];
    const change = start > 0 ? (end / start - 1) * 100 : 0;
    return { min, max, start, end, change };
  }, [hasData, values]);

  const width = viewportWidth;
  const pad = { left: 36, right: 18, top: 18, bottom: 36 };
  const innerW = width - pad.left - pad.right;
  const innerH = height - pad.top - pad.bottom;

  const yRange = useMemo(() => {
    if (!hasData) return { min: 0, max: 1 };
    const [min, max] = [stats.min, stats.max];
    if (min === max) {
      return { min: min - 0.5, max: max + 0.5 };
    }
    const padding = (max - min) * 0.08;
    return { min: min - padding, max: max + padding };
  }, [hasData, stats.max, stats.min]);

  const toX = (idx: number) =>
    pad.left + (Math.max(idx, 0) / Math.max(values.length - 1, 1)) * innerW;
  const toY = (val: number) => {
    const clamped = Math.min(yRange.max, Math.max(yRange.min, val));
    const ratio = (clamped - yRange.min) / Math.max(yRange.max - yRange.min, 1e-6);
    return pad.top + (1 - ratio) * innerH;
  };

  const linePath = useMemo(() => {
    if (!hasData) return "";
    return values
      .map((v, idx) => `${idx === 0 ? "M" : "L"} ${toX(idx)} ${toY(v)}`)
      .join(" ");
  }, [hasData, toX, toY, values]);

  const areaPath = useMemo(() => {
    if (!hasData) return "";
    const startPath = `M ${pad.left} ${pad.top + innerH}`;
    const line = values
      .map((v, idx) => `L ${toX(idx)} ${toY(v)}`)
      .join(" ");
    const endPath = `L ${pad.left + innerW} ${pad.top + innerH} Z`;
    return `${startPath} ${line} ${endPath}`;
  }, [hasData, innerH, innerW, pad.left, pad.top, toX, toY, values]);

  const [hoverIdx, setHoverIdx] = useState<number | null>(hasData ? values.length - 1 : null);
  const hoveredIdx = hoverIdx ?? (hasData ? values.length - 1 : null);
  const hoveredPoint = hoveredIdx != null ? safePoints[hoveredIdx] : undefined;
  const hoveredX = hoveredIdx != null ? toX(hoveredIdx) : null;
  const hoveredY = hoveredPoint ? toY(hoveredPoint.value) : null;

  const gridLines = useMemo(() => {
    const lines = [];
    for (let i = 0; i <= 4; i++) {
      const ratio = i / 4;
      const y = pad.top + ratio * innerH;
      const v = yRange.max - ratio * (yRange.max - yRange.min);
      lines.push({ y, v });
    }
    return lines;
  }, [innerH, pad.top, yRange.max, yRange.min]);

  const handleMove = (e: MouseEvent<SVGSVGElement>) => {
    if (!hasData) return;
    const rect = e.currentTarget.getBoundingClientRect();
    const ratio = (e.clientX - rect.left) / Math.max(rect.width, 1);
    const idxFloat = ratio * Math.max(values.length - 1, 0);
    const idx = Math.min(values.length - 1, Math.max(0, Math.round(idxFloat)));
    setHoverIdx(idx);
  };

  const displayValue = (value: number) =>
    valueFormatter ? valueFormatter(value) : `${unit}${value.toFixed(2)}`;

  const tooltip = hoveredPoint
    ? tooltipFormatter?.(hoveredPoint, hoveredIdx ?? 0) ?? {
        title: hoveredPoint.label,
        value: displayValue(hoveredPoint.value),
        meta:
          baseline && baseline > 0
            ? `相對基準 ${(hoveredPoint.value / baseline - 1) * 100 > 0 ? "+" : ""}${(
                (hoveredPoint.value / baseline - 1) *
                100
              ).toFixed(2)}%`
            : undefined,
      }
    : null;

  if (!hasData) {
    return (
      <Box
        sx={{
          p: 2,
          borderRadius: 3,
          border: "1px dashed rgba(255,255,255,0.15)",
          background: "linear-gradient(135deg, rgba(16,215,255,0.06), rgba(0,0,0,0.4))",
        }}
      >
        <Typography variant="subtitle1" fontWeight={700}>
          {title}
        </Typography>
        {subtitle && (
          <Typography variant="body2" color="text.secondary">
            {subtitle}
          </Typography>
        )}
        <Box sx={{ py: 6, textAlign: "center" }}>
          <Typography variant="body2" color="text.secondary">
            {emptyText}
          </Typography>
        </Box>
      </Box>
    );
  }

  return (
    <Box
      sx={{
        p: 2.4,
        borderRadius: 3,
        border: "1px solid rgba(255,255,255,0.08)",
        background:
          "linear-gradient(135deg, rgba(16,215,255,0.07), rgba(0,0,0,0.45)), radial-gradient(circle at 18% 20%, rgba(122,230,255,0.08), transparent 40%)",
        boxShadow: "0 28px 60px rgba(0,0,0,0.35)",
      }}
    >
      <Stack
        direction={{ xs: "column", sm: "row" }}
        justifyContent="space-between"
        alignItems={{ xs: "flex-start", sm: "center" }}
        spacing={1}
        mb={1.5}
      >
        <Box>
          <Typography variant="subtitle1" fontWeight={800}>
            {title}
          </Typography>
          {subtitle && (
            <Typography variant="caption" color="text.secondary">
              {subtitle}
            </Typography>
          )}
        </Box>
        {tags && tags.length > 0 && (
          <Stack direction="row" spacing={0.8} flexWrap="wrap" rowGap={0.6}>
            {tags.map((tag) => (
              <Chip
                key={tag.label}
                size="small"
                label={tag.label}
                color={tag.color ?? "default"}
                variant="outlined"
              />
            ))}
          </Stack>
        )}
      </Stack>

      <Box sx={{ height, position: "relative" }}>
        <svg
          width="100%"
          height="100%"
          viewBox={`0 0 ${width} ${height}`}
          preserveAspectRatio="none"
          onMouseMove={handleMove}
          onMouseLeave={() => setHoverIdx(values.length - 1)}
        >
          <defs>
            <linearGradient id={gradientId} x1="0" y1="0" x2="0" y2="1">
              <stop offset="0%" stopColor={`${accent}BF`} />
              <stop offset="100%" stopColor={`${accent}10`} />
            </linearGradient>
            <linearGradient id={lineId} x1="0" y1="0" x2="1" y2="0">
              <stop offset="0%" stopColor="#7ae6ff" />
              <stop offset="100%" stopColor={accent} />
            </linearGradient>
          </defs>

          {gridLines.map((line, idx) => (
            <g key={`grid-${line.y}-${idx}`}>
              <line
                x1={pad.left}
                y1={line.y}
                x2={pad.left + innerW}
                y2={line.y}
                stroke="rgba(255,255,255,0.08)"
                strokeWidth="0.7"
                strokeDasharray="3 3"
              />
              <text
                x={pad.left - 6}
                y={line.y + 3}
                fontSize="9"
                fill="rgba(255,255,255,0.55)"
                textAnchor="end"
              >
                {valueFormatter ? valueFormatter(line.v) : `${unit}${line.v.toFixed(2)}`}
              </text>
            </g>
          ))}

          <line
            x1={pad.left}
            y1={pad.top + innerH}
            x2={pad.left + innerW}
            y2={pad.top + innerH}
            stroke="rgba(255,255,255,0.22)"
            strokeWidth="0.8"
          />

          <path d={areaPath} fill={`url(#${gradientId})`} stroke="none" />
          <path
            d={linePath}
            fill="none"
            stroke={`url(#${lineId})`}
            strokeWidth="2.6"
            strokeLinejoin="round"
            strokeLinecap="round"
            style={{ filter: "drop-shadow(0px 9px 18px rgba(16,215,255,0.25))" }}
          />

          {hoveredX != null && hoveredY != null && (
            <>
              <line
                x1={hoveredX}
                y1={pad.top}
                x2={hoveredX}
                y2={pad.top + innerH}
                stroke="rgba(255,255,255,0.6)"
                strokeWidth="0.7"
                strokeDasharray="4 2"
              />
              <circle cx={hoveredX} cy={hoveredY} r={3.4} fill="#0af" stroke="#fff" strokeWidth="0.7" />
              <circle cx={hoveredX} cy={hoveredY} r={7} fill="rgba(16,215,255,0.15)" />
            </>
          )}

          <text
            x={pad.left}
            y={pad.top + innerH + 16}
            fontSize="9"
            fill="rgba(255,255,255,0.7)"
            textAnchor="start"
          >
            {safePoints[0]?.label ?? ""}
          </text>
          <text
            x={pad.left + innerW}
            y={pad.top + innerH + 16}
            fontSize="9"
            fill="rgba(255,255,255,0.7)"
            textAnchor="end"
          >
            {safePoints[safePoints.length - 1]?.label ?? ""}
          </text>
        </svg>

        {tooltip && hoveredPoint && (
          <Box
            sx={{
              position: "absolute",
              top: 12,
              right: 12,
              p: 1.25,
              borderRadius: 1.5,
              bgcolor: "rgba(4,10,22,0.86)",
              border: "1px solid rgba(122,230,255,0.25)",
              minWidth: 200,
              boxShadow: "0 14px 30px rgba(0,0,0,0.45)",
            }}
          >
            {tooltip.title && (
              <Typography variant="caption" color="text.secondary">
                {tooltip.title}
              </Typography>
            )}
            <Typography variant="subtitle2" fontWeight={800} sx={{ color: "#7ae6ff" }}>
              {tooltip.value ?? displayValue(hoveredPoint.value)}
            </Typography>
            {tooltip.meta && (
              <Typography variant="caption" color="text.secondary">
                {tooltip.meta}
              </Typography>
            )}
          </Box>
        )}
      </Box>
    </Box>
  );
}
