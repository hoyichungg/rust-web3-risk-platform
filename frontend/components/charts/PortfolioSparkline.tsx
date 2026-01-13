"use client";

import { useMemo } from "react";
import { GlowingAreaChart } from "./GlowingAreaChart";
import { useI18n } from "../../lib/i18n";

type Props = {
  label: string;
  values: number[];
  unit?: string;
};

export function PortfolioSparkline({ label, values, unit = "" }: Props) {
  const { t } = useI18n();

  const stats = useMemo(() => {
    if (!values || values.length === 0) {
      return {
        min: 0,
        max: 0,
        start: 0,
        end: 0,
        change: 0,
      };
    }
    const min = Math.min(...values);
    const max = Math.max(...values);
    const start = values[0];
    const end = values[values.length - 1];
    const change = start > 0 ? (end / start - 1) * 100 : 0;
    return { min, max, start, end, change };
  }, [values]);

  const points = useMemo(
    () =>
      values.map((v, idx) => ({
        value: v,
        label: `${t("dashboard.latest")} #${idx + 1}`,
      })),
    [t, values]
  );

  return (
    <GlowingAreaChart
      title={label}
      subtitle={t("dashboard.asset_trend")}
      points={points}
      unit={unit}
      baseline={stats.start}
      height={210}
      viewportWidth={520}
      tags={[
        {
          label: `${stats.change >= 0 ? "+" : ""}${stats.change.toFixed(2)}%`,
          color: stats.change >= 0 ? "success" : "error",
        },
        { label: `${unit}${stats.end.toFixed(2)}` },
      ]}
      emptyText={t("dashboard.no_history")}
      valueFormatter={(v) => `${unit}${v.toFixed(2)}`}
    />
  );
}
