import { HashRouter, Routes, Route, useNavigate } from "react-router-dom";
import { useEffect, useMemo } from "react";
import { ThemeProvider, createTheme } from "@mui/material/styles";
import CssBaseline from "@mui/material/CssBaseline";
import HomePage from "./pages/HomePage";
import DBPage from "./pages/DBPage";
import { useVscodeTheme } from "./useVscodeTheme";

function getCssVar(name: string, fallback: string): string {
  const value = getComputedStyle(document.documentElement)
    .getPropertyValue(name)
    .trim();
  return value || fallback;
}

function ExtensionMessageListener() {
  const navigate = useNavigate();

  useEffect(() => {
    const handleMessage = (event: MessageEvent) => {
      const message = event.data;
      if (message.command === "navigate" && message.path) {
        navigate(message.path);
      }
    };

    window.addEventListener("message", handleMessage);
    return () => window.removeEventListener("message", handleMessage);
  }, [navigate]);

  return null;
}

export default function App() {
  const vscodeTheme = useVscodeTheme();

  const theme = useMemo(() => {
    const isDark = vscodeTheme.mode === "dark";
    const bg = getCssVar("--vscode-editor-background", isDark ? "#1e1e1e" : "#ffffff");
    const fg = getCssVar("--vscode-editor-foreground", isDark ? "#cccccc" : "#333333");
    const surface = getCssVar("--vscode-sideBar-background", isDark ? "#252526" : "#f3f3f3");
    const border = getCssVar("--vscode-panel-border", isDark ? "#3c3c3c" : "#e5e5e5");
    const accent = getCssVar("--vscode-button-background", "#0e639c");
    const accentFg = getCssVar("--vscode-button-foreground", "#ffffff");

    return createTheme({
      palette: {
        mode: vscodeTheme.mode,
        background: {
          default: bg,
          paper: surface,
        },
        text: {
          primary: fg,
        },
        primary: {
          main: accent,
          contrastText: accentFg,
        },
        divider: border,
      },
      typography: {
        fontFamily: getCssVar("--vscode-font-family", "system-ui, sans-serif"),
      },
    });
  }, [vscodeTheme]);

  return (
    <ThemeProvider theme={theme}>
      <CssBaseline />
      <HashRouter>
        <ExtensionMessageListener />
        <Routes>
          <Route path="/" index element={<HomePage />} />
          <Route path="/db" element={<DBPage />} />
        </Routes>
      </HashRouter>
    </ThemeProvider>
  );
}