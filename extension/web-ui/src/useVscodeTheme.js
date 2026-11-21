"use strict";
Object.defineProperty(exports, "__esModule", { value: true });
exports.useVscodeTheme = useVscodeTheme;
const react_1 = require("react");
function readBodyTheme() {
    const { classList, dataset } = document.body;
    const isDark = classList.contains('vscode-dark') ||
        classList.contains('vscode-high-contrast');
    const isHighContrast = classList.contains('vscode-high-contrast') ||
        classList.contains('vscode-high-contrast-light');
    return {
        mode: isDark ? 'dark' : 'light',
        isHighContrast,
        themeName: dataset.vscodeThemeName ?? '',
    };
}
function useVscodeTheme() {
    const [theme, setTheme] = (0, react_1.useState)(() => readBodyTheme());
    (0, react_1.useEffect)(() => {
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
//# sourceMappingURL=useVscodeTheme.js.map