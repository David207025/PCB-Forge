const { execSync } = require('child_process');
const fs = require('fs');
const path = require('path');

const extensionDir = __dirname;
const webDir = path.join(extensionDir, 'web');
const webUiDistDir = path.join(extensionDir, 'web-ui', 'dist');

console.log('🏗️ Building Web UI via pnpm...');
execSync('pnpm --filter web-ui build', { cwd: extensionDir, stdio: 'inherit' });

console.log('🧹 Cleaning and syncing web assets...');
if (fs.existsSync(webDir)) {
  fs.rmSync(webDir, { recursive: true, force: true });
}
fs.mkdirSync(webDir, { recursive: true });

if (fs.existsSync(webUiDistDir)) {
  fs.cpSync(webUiDistDir, webDir, { recursive: true });
  console.log('✅ Web assets successfully synced to extension/web!');
} else {
  console.error('❌ Error: web-ui/dist directory not found after build.');
  process.exit(1);
}