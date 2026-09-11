/**
 * build-webview.js
 *
 * Builds the React web-ui and copies the compiled output into extension/dist/,
 * which is the directory included in the packaged VSIX file.
 *
 * Pipeline:
 *   1. Run `pnpm --filter web-ui build` → produces web-ui/dist/
 *   2. Wipe any stale extension/dist/ output
 *   3. Copy web-ui/dist/ → extension/dist/
 *
 * This script is invoked by the `build:webview` npm script in extension/package.json
 * and by the root package-extension.js packaging pipeline.
 */

const { execSync } = require('child_process');
const fs = require('fs');
const path = require('path');

// Absolute path to the extension/ directory (where this script lives)
const extensionDir = __dirname;

// Destination directory that gets bundled into the VSIX.
// Must NOT include web-ui/ itself — only the compiled output goes here.
const distDir = path.join(extensionDir, 'dist');

// Source: where Vite writes its output after `pnpm --filter web-ui build`
const webUiDistDir = path.join(extensionDir, 'web-ui', 'dist');

// Step 1: Build the React application
execSync('pnpm --filter web-ui build', { cwd: extensionDir, stdio: 'inherit' });

// Step 2: Wipe stale output so no old assets linger between builds
console.log('🧹 Cleaning stale build output...');
if (fs.existsSync(distDir)) {
  fs.rmSync(distDir, { recursive: true, force: true });
}
fs.mkdirSync(distDir, { recursive: true });

// Step 3: Copy the fresh Vite output into extension/dist/
if (fs.existsSync(webUiDistDir)) {
  fs.cpSync(webUiDistDir, distDir, { recursive: true });
  console.log('✅ Web assets successfully synced to extension/dist/');
} else {
  console.error('❌ Error: web-ui/dist directory not found after build.');
  process.exit(1);
}