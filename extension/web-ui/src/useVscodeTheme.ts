import { useEffect, useState } from 'react';

export interface VscodeThemeState {
  mode: 'light' | 'dark';
  isHighContrast: boolean;
  themeName: string;
}

function readBodyTheme(): VscodeThemeState {
  const { classList, dataset } = document.body;
  const isDark =
    classList.contains('vscode-dark') ||
    classList.contains('vscode-high-contrast');
  const isHighContrast =
    classList.contains('vscode-high-contrast') ||
    classList.contains('vscode-high-contrast-light');

  return {
    mode: isDark ? 'dark' : 'light',
    isHighContrast,
    themeName: dataset.vscodeThemeName ?? '',
  };
}

export function useVscodeTheme(): VscodeThemeState {
  const [theme, setTheme] = useState<VscodeThemeState>(() => readBodyTheme());

  useEffect(() => {
    const observer = new MutationObserver(() => {
      setTheme(readBodyTheme());
    });

    observer.observe(document.body, {
      attributes: true,
      attributeFilter: ['class', 'data-vscode-theme-kind', 'data-vscode-theme-name'],
    });

    return () => observer.disconnect();
  }, []);

  return theme;
}