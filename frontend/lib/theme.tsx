"use client";

import createCache from "@emotion/cache";
import { CacheProvider } from "@emotion/react";
import { CssBaseline, ThemeProvider, createTheme } from "@mui/material";
import { useServerInsertedHTML } from "next/navigation";
import { ReactNode, useState } from "react";

const theme = createTheme({
  palette: {
    mode: "dark",
    background: {
      default: "#0b0e14",
      paper: "#121723",
    },
    primary: {
      main: "#7CE7FF",
      light: "#A3ECFF",
      dark: "#3DB9D6",
    },
    secondary: {
      main: "#B18CFF",
      light: "#C9B3FF",
      dark: "#7C5BC8",
    },
    success: {
      main: "#5CE2B8",
    },
    warning: {
      main: "#F7B955",
    },
  },
  typography: {
    fontFamily: "Space Grotesk, Inter, 'Noto Sans TC', system-ui, -apple-system, sans-serif",
    h4: { fontWeight: 800, letterSpacing: -0.4 },
    h6: { fontWeight: 700 },
    subtitle1: { fontWeight: 700 },
  },
  shape: {
    borderRadius: 14,
  },
  shadows: [
    "none",
    "0px 4px 14px rgba(0,0,0,0.25)",
    "0px 6px 20px rgba(0,0,0,0.28)",
    ...Array(22).fill("0px 10px 30px rgba(0,0,0,0.32)"),
  ] as any,
  components: {
    MuiCssBaseline: {
      styleOverrides: {
        body: {
          background:
            "radial-gradient(circle at 10% 20%, rgba(92,226,184,0.06), transparent 28%), radial-gradient(circle at 90% 10%, rgba(177,140,255,0.08), transparent 26%), #0b0e14",
        },
      },
    },
    MuiButton: {
      defaultProps: {
        variant: "contained",
      },
      styleOverrides: {
        root: {
          textTransform: "none",
          borderRadius: 999,
          boxShadow: "0 12px 30px rgba(124,231,255,0.15)",
        },
      },
    },
    MuiCard: {
      styleOverrides: {
        root: {
          backgroundImage:
            "linear-gradient(135deg, rgba(255,255,255,0.04), rgba(255,255,255,0.02))",
          borderRadius: 22,
          border: "1px solid rgba(255,255,255,0.06)",
          boxShadow: "0 20px 50px rgba(0,0,0,0.35)",
        },
      },
    },
    MuiPaper: {
      styleOverrides: {
        root: {
          backgroundImage: "none",
          borderRadius: 18,
          border: "1px solid rgba(255,255,255,0.06)",
        },
      },
    },
    MuiChip: {
      styleOverrides: {
        root: {
          borderRadius: 999,
          borderColor: "rgba(255,255,255,0.15)",
        },
      },
    },
  },
});

function NextAppDirEmotionCacheProvider({
  children,
}: {
  children: ReactNode;
}) {
  const [{ cache, flush }] = useState(() => {
    const cache = createCache({ key: "mui", prepend: true });
    cache.compat = true;
    let inserted: string[] = [];
    const prevInsert = cache.insert;
    cache.insert = (
      ...args: Parameters<typeof prevInsert>
    ): ReturnType<typeof prevInsert> => {
      const [selector, serialized, sheet, shouldCache] = args;
      if (cache.inserted[serialized.name] === undefined) {
        inserted.push(serialized.name);
      }
      return prevInsert(selector, serialized, sheet, shouldCache);
    };
    const flush = () => {
      const prev = inserted;
      inserted = [];
      return prev;
    };
    return { cache, flush };
  });

  useServerInsertedHTML(() => {
    const names = flush();
    if (names.length === 0) {
      return null;
    }

    let styles = "";
    for (const name of names) {
      const style = cache.inserted[name];
      if (typeof style === "string") {
        styles += style;
      }
    }

    return (
      <style
        data-emotion={`${cache.key} ${names.join(" ")}`}
        dangerouslySetInnerHTML={{ __html: styles }}
      />
    );
  });

  return <CacheProvider value={cache}>{children}</CacheProvider>;
}

export function AppThemeProvider({ children }: { children: ReactNode }) {
  return (
    <NextAppDirEmotionCacheProvider>
      <ThemeProvider theme={theme}>
        <CssBaseline />
        {children}
      </ThemeProvider>
    </NextAppDirEmotionCacheProvider>
  );
}
