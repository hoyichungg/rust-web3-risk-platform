"use client";

import "./pulse.css";

import {
  Alert,
  Box,
  Button,
  Card,
  CardContent,
  Container,
  Stack,
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableRow,
  TextField,
  MenuItem,
  Typography,
  Chip,
} from "@mui/material";
import { useEffect, useState } from "react";
import { useAuth, useProfile } from "../../lib/auth-context";
import {
  createAlert,
  deleteAlert,
  fetchAlertTriggers,
  fetchAlerts,
  simulateAlertTrigger,
  updateAlert,
} from "../../lib/api";
import { useRouter } from "next/navigation";
import { useI18n } from "../../lib/i18n";

type AlertRule = {
  id: string;
  user_id: string;
  type: string;
  threshold: number;
  enabled: boolean;
  cooldown_secs?: number;
};

type AlertTrigger = {
  id: string;
  rule_id: string;
  wallet_id: string;
  message: string;
  created_at: string;
};

const ALERT_TYPES = [
  { value: "tvl_drop_pct", label: "TVL 下跌 %", hint: "最新 TVL 與上一筆相比下跌百分比" },
  { value: "tvl_below", label: "TVL 低於美元", hint: "總資產低於指定金額 (USD)" },
  { value: "exposure_pct", label: "資產曝險 %", hint: "單一資產佔比超過門檻" },
  { value: "net_outflow_pct", label: "24h 淨流出 %", hint: "過去 24h 淨流出佔比" },
  { value: "approval_spike", label: "Approval 激增", hint: "最近區塊內 Approval 次數" },
];

export default function AlertsPage() {
  const { profile, loading } = useProfile();
  const router = useRouter();
  const { t } = useI18n();
  const [rules, setRules] = useState<AlertRule[]>([]);
  const [triggers, setTriggers] = useState<AlertTrigger[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [form, setForm] = useState({ type: "tvl_drop_pct", threshold: 10, cooldown: 300 });
  const [saving, setSaving] = useState(false);
  const [togglingId, setTogglingId] = useState<string | null>(null);
  const [deletingId, setDeletingId] = useState<string | null>(null);
  const [editing, setEditing] = useState<{
    id: string;
    threshold: number;
    cooldown: number;
    saving: boolean;
  } | null>(null);

  useEffect(() => {
    if (!loading && !profile) {
      router.replace("/login");
    }
  }, [loading, profile, router]);

  const load = () => {
    fetchAlerts()
      .then(async (res) => {
        if (!res.ok) throw new Error(`載入規則失敗 (${res.status})`);
        setRules(await res.json());
      })
      .catch((err) => setError(err instanceof Error ? err.message : "未知錯誤"));
    fetchAlertTriggers()
      .then(async (res) => {
        if (!res.ok) throw new Error(`載入觸發失敗 (${res.status})`);
        setTriggers(await res.json());
      })
      .catch(() => {});
  };

  useEffect(() => {
    load();
  }, []);

  const submit = async () => {
    if (!form.threshold || form.threshold <= 0) {
      setError("閾值需大於 0");
      return;
    }
    setError(null);
    setSaving(true);
    try {
      const res = await createAlert({
        type: form.type,
        threshold: form.threshold,
        enabled: true,
        cooldown_secs: form.cooldown,
      });
      if (!res.ok) throw new Error(`建立失敗 (${res.status})`);
      load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "未知錯誤");
    } finally {
      setSaving(false);
    }
  };

  const toggle = async (rule: AlertRule) => {
    setTogglingId(rule.id);
    try {
      await updateAlert(rule.id, {
        type: rule.type,
        threshold: rule.threshold,
        enabled: !rule.enabled,
        cooldown_secs: rule.cooldown_secs,
      });
      load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "更新失敗");
    } finally {
      setTogglingId(null);
    }
  };

  const remove = async (rule: AlertRule) => {
    setDeletingId(rule.id);
    try {
      await deleteAlert(rule.id);
      load();
    } finally {
      setDeletingId(null);
    }
  };

  const startEdit = (rule: AlertRule) => {
    setEditing({
      id: rule.id,
      threshold: rule.threshold,
      cooldown: rule.cooldown_secs ?? 300,
      saving: false,
    });
  };

  const cancelEdit = () => setEditing(null);

  const saveEdit = async () => {
    if (!editing) return;
    if (!editing.threshold || editing.threshold <= 0) {
      setError("閾值需大於 0");
      return;
    }
    setEditing({ ...editing, saving: true });
    try {
      await updateAlert(editing.id, {
        type: rules.find((r) => r.id === editing.id)?.type ?? "tvl_drop_pct",
        threshold: editing.threshold,
        enabled: rules.find((r) => r.id === editing.id)?.enabled ?? true,
        cooldown_secs: editing.cooldown,
      });
      load();
      setEditing(null);
    } catch (err) {
      setError(err instanceof Error ? err.message : "更新失敗");
      setEditing((prev) => (prev ? { ...prev, saving: false } : prev));
    }
  };

  const simulate = async (rule: AlertRule) => {
    setError(null);
    try {
      const res = await simulateAlertTrigger(rule.id);
      if (!res.ok) throw new Error(`模擬觸發失敗 (${res.status})`);
      load();
    } catch (err) {
      setError(err instanceof Error ? err.message : "未知錯誤");
    }
  };

  return (
    <Container sx={{ py: 4 }}>
      <Typography variant="h4" fontWeight={800} gutterBottom>
        {t("alerts.title")}
      </Typography>
      <Stack spacing={3}>
        <Card>
          <CardContent>
            <Typography variant="subtitle1" fontWeight={700} gutterBottom>
              {t("alerts.create_rule")}
            </Typography>
            <Stack spacing={2}>
              <Stack direction={{ xs: "column", sm: "row" }} spacing={2} alignItems="flex-end">
                <TextField
                  label={t("alerts.type")}
                  select
                  value={form.type}
                  onChange={(e) => setForm((f) => ({ ...f, type: e.target.value }))}
                  sx={{ minWidth: 220 }}
                >
                  {ALERT_TYPES.map((t) => (
                    <MenuItem key={t.value} value={t.value}>
                      {t.label}
                    </MenuItem>
                  ))}
                </TextField>
                <TextField
                  label={t("alerts.threshold")}
                  type="number"
                  value={form.threshold}
                  onChange={(e) => setForm((f) => ({ ...f, threshold: Number(e.target.value) }))}
                  helperText={ALERT_TYPES.find((t) => t.value === form.type)?.hint}
                />
                <TextField
                  label="Cooldown (秒)"
                  type="number"
                  value={form.cooldown}
                  onChange={(e) => setForm((f) => ({ ...f, cooldown: Number(e.target.value) }))}
                  helperText="避免短時間重複告警"
                />
                <Button variant="contained" onClick={submit} disabled={saving}>
                  {saving ? "建立中..." : t("alerts.add")}
                </Button>
              </Stack>
              <Typography variant="caption" color="text.secondary">
                支援: TVL 下跌%、資產曝險%、24h 淨流出%、Approval 激增、TVL 低於門檻。
              </Typography>
            </Stack>
            {error && (
              <Alert severity="error" sx={{ mt: 2 }}>
                {error}
              </Alert>
            )}
          </CardContent>
        </Card>

        <Card>
          <CardContent>
            <Typography variant="subtitle1" fontWeight={700} gutterBottom>
              {t("alerts.rules")}
            </Typography>
            <Table size="small">
              <TableHead>
                <TableRow>
                  <TableCell>{t("alerts.type")}</TableCell>
                  <TableCell>{t("alerts.threshold")}</TableCell>
                  <TableCell>{t("alerts.status")}</TableCell>
                  <TableCell align="right">{t("alerts.actions")}</TableCell>
                </TableRow>
              </TableHead>
              <TableBody>
                {rules.map((r) => (
                  <TableRow key={r.id}>
                    <TableCell>{r.type}</TableCell>
                    <TableCell>
                      {editing?.id === r.id ? (
                        <Stack direction="row" spacing={1} alignItems="center">
                          <TextField
                            size="small"
                            type="number"
                            value={editing.threshold}
                            onChange={(e) =>
                              setEditing((prev) =>
                                prev ? { ...prev, threshold: Number(e.target.value) } : prev
                              )
                            }
                            sx={{ width: 120 }}
                          />
                          {r.type.includes("pct") ? "%" : ""}
                        </Stack>
                      ) : (
                        <>
                          {r.threshold}
                          {r.type.includes("pct") ? "%" : ""}
                        </>
                      )}
                    </TableCell>
                    <TableCell>
                      <Box
                        sx={{
                          display: "inline-flex",
                          alignItems: "center",
                          gap: 0.75,
                        }}
                      >
                        <Box
                          sx={{
                            width: 12,
                            height: 12,
                            borderRadius: "50%",
                            bgcolor: r.enabled ? "success.main" : "grey.600",
                            position: "relative",
                            "&::after": r.enabled
                              ? {
                                  content: '""',
                                  position: "absolute",
                                  inset: -6,
                                  borderRadius: "50%",
                                  background:
                                    "radial-gradient(circle, rgba(76,175,80,0.28), transparent 60%)",
                                  animation: "pulseGlow 1.6s ease-in-out infinite",
                                }
                              : undefined,
                          }}
                        />
                        <Typography variant="body2" color="text.secondary">
                          {r.enabled ? t("alerts.on") : t("alerts.off")}
                        </Typography>
                      </Box>
                      <Typography variant="caption" color="text.secondary" display="block">
                        {editing?.id === r.id ? (
                          <TextField
                            size="small"
                            type="number"
                            value={editing.cooldown}
                            onChange={(e) =>
                              setEditing((prev) =>
                                prev ? { ...prev, cooldown: Number(e.target.value) } : prev
                              )
                            }
                            sx={{ width: 140 }}
                            helperText="Cooldown (秒)"
                          />
                        ) : (
                          <>Cooldown: {r.cooldown_secs ?? 300}s</>
                        )}
                      </Typography>
                    </TableCell>
                    <TableCell align="right">
                      <Stack direction="row" spacing={1} justifyContent="flex-end" alignItems="center">
                        <Button
                          size="small"
                          variant="outlined"
                          color="secondary"
                          onClick={() => simulate(r)}
                          sx={{ minWidth: 78 }}
                        >
                          模擬觸發
                        </Button>
                        {editing?.id === r.id ? (
                          <>
                            <Button
                              size="small"
                              variant="contained"
                              color="primary"
                              onClick={saveEdit}
                              disabled={editing.saving}
                            >
                              {editing.saving ? "儲存中..." : "儲存"}
                            </Button>
                            <Button size="small" variant="text" onClick={cancelEdit}>
                              取消
                            </Button>
                          </>
                        ) : (
                          <>
                            <Button
                              size="small"
                              variant="outlined"
                              onClick={() => toggle(r)}
                              sx={{ minWidth: 78 }}
                              disabled={togglingId === r.id}
                            >
                              {togglingId === r.id
                                ? "更新中..."
                                : r.enabled
                                  ? t("alerts.toggle_off")
                                  : t("alerts.toggle_on")}
                            </Button>
                            <Button
                              size="small"
                              variant="outlined"
                              onClick={() => startEdit(r)}
                            >
                              編輯
                            </Button>
                            <Button
                              size="small"
                              color="error"
                              onClick={() => remove(r)}
                              sx={{ minWidth: 78 }}
                              disabled={deletingId === r.id}
                            >
                              {deletingId === r.id ? "刪除中..." : t("alerts.delete")}
                            </Button>
                          </>
                        )}
                      </Stack>
                    </TableCell>
                  </TableRow>
                ))}
                {rules.length === 0 && (
                  <TableRow>
                    <TableCell colSpan={4}>{t("dashboard.no_data")}</TableCell>
                  </TableRow>
                )}
              </TableBody>
            </Table>
          </CardContent>
        </Card>

        <Card>
          <CardContent>
            <Typography variant="subtitle1" fontWeight={700} gutterBottom>
              {t("alerts.triggers")}
            </Typography>
            <Stack spacing={1}>
              {triggers.map((t) => (
                <Box key={t.id} sx={{ border: "1px solid rgba(255,255,255,0.08)", p: 1.2, borderRadius: 1 }}>
                  <Typography variant="body2">{t.message}</Typography>
                  <Typography variant="caption" color="text.secondary">
                    {new Date(t.created_at).toLocaleString("zh-TW")}
                  </Typography>
                </Box>
              ))}
              {triggers.length === 0 && (
                <Typography variant="caption" color="text.secondary">
                  {t("dashboard.no_data")}
                </Typography>
              )}
            </Stack>
          </CardContent>
        </Card>
      </Stack>
    </Container>
  );
}
