"use client";

import { CircularProgress, Stack, Typography } from "@mui/material";
import { useEffect } from "react";
import { useRouter } from "next/navigation";
import { useProfile } from "../lib/auth-context";

export default function HomePage() {
  const router = useRouter();
  const { profile, loading, refresh } = useProfile();

  useEffect(() => {
    if (!loading) {
      if (profile) {
        router.replace("/dashboard");
      } else {
        refresh({ silent: true }).finally(() => router.replace("/login"));
      }
    }
  }, [loading, profile, refresh, router]);

  return (
    <Stack
      alignItems="center"
      justifyContent="center"
      minHeight="100vh"
      spacing={2}
      sx={{ background: "#05070c", color: "white" }}
    >
      <CircularProgress />
      <Typography variant="body2" color="text.secondary">
        正在檢查登入狀態...
      </Typography>
    </Stack>
  );
}
