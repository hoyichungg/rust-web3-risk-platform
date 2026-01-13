"use client";

import {
  Card,
  CardContent,
  Stack,
  Typography,
  LinearProgress,
  Box,
  Chip,
  Avatar,
} from "@mui/material";
import { useI18n } from "../../lib/i18n";
import { PortfolioPosition } from "../../lib/portfolio-hooks";

type Props = {
  positions: PortfolioPosition[];
};

export function AssetDistribution({ positions }: Props) {
  const { t } = useI18n();
  const total = positions.reduce((sum, p) => sum + p.usd_value, 0);
  const palette = ["#7ae6ff", "#9c6bff", "#f7c843", "#ff8f70", "#55efc4", "#fa5b7a"];

  return (
    <Card
      sx={{
        height: "100%",
        background: "linear-gradient(135deg, #0b1019, #0f1626)",
        border: "1px solid rgba(255,255,255,0.06)",
        boxShadow: "0 18px 45px rgba(0,0,0,0.35)",
      }}
    >
      <CardContent sx={{ height: "100%", display: "flex", flexDirection: "column" }}>
        <Stack direction="row" justifyContent="space-between" alignItems="center" mb={1}>
          <Typography variant="subtitle1" fontWeight={700}>
            {t("dashboard.asset_dist")}
          </Typography>
          <Chip
            size="small"
            label={`總額 ${total > 0 ? `$${total.toFixed(2)}` : "-"}`}
            variant="outlined"
          />
        </Stack>
        <Stack spacing={1.2} sx={{ flex: 1, justifyContent: "flex-start" }}>
          {positions.map((p, idx) => {
            const pct = total > 0 ? (p.usd_value / total) * 100 : 0;
            const color = palette[idx % palette.length];
            return (
              <Box
                key={p.asset_symbol}
                sx={{
                  "@keyframes fadeInUp": {
                    from: { opacity: 0, transform: "translateY(8px)" },
                    to: { opacity: 1, transform: "translateY(0)" },
                  },
                  animation: "fadeInUp 260ms ease forwards",
                  animationDelay: `${idx * 60}ms`,
                  opacity: 0,
                }}
              >
                <Stack direction="row" justifyContent="space-between" alignItems="center">
                  <Stack direction="row" spacing={1} alignItems="center">
                    <Avatar
                      sx={{
                        bgcolor: `${color}33`,
                        color,
                        width: 28,
                        height: 28,
                        fontSize: 12,
                        fontWeight: 800,
                      }}
                    >
                      {p.asset_symbol.slice(0, 2)}
                    </Avatar>
                    <Typography variant="body2">{p.asset_symbol}</Typography>
                  </Stack>
                  <Stack spacing={0.2} alignItems="flex-end">
                    <Typography variant="caption" color="text.secondary">
                      ${p.usd_value.toFixed(2)}
                    </Typography>
                    <Typography variant="caption" color="text.secondary">
                      {pct.toFixed(1)}%
                    </Typography>
                  </Stack>
                </Stack>
                <LinearProgress
                  variant="determinate"
                  value={Math.min(100, pct)}
                  sx={{
                    height: 10,
                    borderRadius: 6,
                    backgroundColor: "rgba(255,255,255,0.06)",
                    "& .MuiLinearProgress-bar": {
                      borderRadius: 6,
                      background: `linear-gradient(90deg, ${color}, ${color}aa)`,
                    },
                  }}
                />
              </Box>
            );
          })}
          {positions.length === 0 && (
            <Typography variant="caption" color="text.secondary">
              {t("dashboard.no_data")}
            </Typography>
          )}
        </Stack>
      </CardContent>
    </Card>
  );
}
