"use client";

import { IconButton, Tooltip } from "@mui/material";
import TranslateIcon from "@mui/icons-material/Translate";
import { useI18n } from "../lib/i18n";

export function LanguageSwitcher() {
  const { lang, toggle } = useI18n();
  return (
    <Tooltip title="切換語言 / Switch language">
      <IconButton
        onClick={toggle}
        size="small"
        sx={{
          position: "fixed",
          bottom: 16,
          right: 16,
          bgcolor: "rgba(255,255,255,0.08)",
          color: "white",
          zIndex: 1300,
          "&:hover": { bgcolor: "rgba(255,255,255,0.16)" },
        }}
      >
        <TranslateIcon fontSize="small" />
        <span style={{ fontSize: 12, marginLeft: 4 }}>{lang.toUpperCase()}</span>
      </IconButton>
    </Tooltip>
  );
}
