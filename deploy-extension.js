/**
 * deploy-extension.js
 *
 * Packages the PCB Forge extension and immediately installs it into a local
 * VS Code-compatible editor for quick testing.
 *
 * Usage:
 *   node deploy-extension.js [cliTool]
 *
 * Arguments:
 *   cliTool  (optional) — the VS Code CLI binary to use for install/uninstall.
 *            Defaults to 'code'. Pass 'cursor' or 'code-insiders' as needed.
 *            Example: node deploy-extension.js cursor
 *
 * Pipeline steps:
 *   1. Read extension version & publisher from extension/package.json
 *   2. Package the extension into build/pcb-forge-<version>.vsix via vsce
 *   3. Uninstall the previous version from the editor (if present)
 *   4. Install the freshly built VSIX
 */

const { execSync } = require('child_process');
const path = require('path');
const fs = require('fs');

const rootDir = __dirname;
const extensionDir = path.join(rootDir, 'extension');
const buildDir = path.join(rootDir, 'build');

// Accept the VS Code CLI binary name as a CLI argument.
// Common values: 'code', 'cursor', 'code-insiders'.
const cliTool = process.argv[2] || 'code';

/**
 * Runs a shell command synchronously, creating `cwd` if it doesn't exist.
 * @param {string} command - Shell command to execute
 * @param {string} cwd     - Working directory
 */
function run(command, cwd) {
  if (!fs.existsSync(cwd)) {
    fs.mkdirSync(cwd, { recursive: true });
  }
  console.log(`\n⚙️ Running: ${command}`);
  execSync(command, { cwd, stdio: 'inherit', shell: true });
}

try {
  console.log(`🚀 Packaging and Deploying via [ ${cliTool} ]...`);

  // ── Step 1: Read extension metadata ─────────────────────────────────────────
  const extensionPkgPath = path.join(extensionDir, 'package.json');
  if (!fs.existsSync(extensionPkgPath)) {
    throw new Error(`Could not find extension package.json at ${extensionPkgPath}`);
  }
  const extensionPkg = JSON.parse(fs.readFileSync(extensionPkgPath, 'utf8'));
  // The full extension identifier is "<publisher>.<name>" e.g. "vacaroiudavid.pcb-forge"
  const extensionId = `${extensionPkg.publisher}.${extensionPkg.name}`;
  const version = extensionPkg.version;

  // ── Step 2: Package the extension into build/ ────────────────────────────────
  if (!fs.existsSync(buildDir)) {
    fs.mkdirSync(buildDir, { recursive: true });
  }
  const outputPath = path.join(buildDir, `pcb-forge-${version}.vsix`);

  // Remove a stale VSIX from a previous run to avoid vsce complaining
  if (fs.existsSync(outputPath)) {
    fs.unlinkSync(outputPath);
  }

  run(`pnpm dlx @vscode/vsce package --out "${outputPath}"`, extensionDir);

  // ── Step 3: Uninstall the old version ────────────────────────────────────────
  console.log(`\n🗑️ Uninstalling previous version of ${extensionId}...`);
  try {
    execSync(`${cliTool} --uninstall-extension ${extensionId}`, { stdio: 'inherit', shell: true });
  } catch (e) {
    // It is fine if the extension wasn't installed yet
    console.log(`⚠️ Note: Extension not previously installed or removal skipped.`);
  }

  // ── Step 4: Install the fresh VSIX ───────────────────────────────────────────
  console.log(`\n📥 Installing new VSIX...`);
  run(`${cliTool} --install-extension "${outputPath}" --force`, rootDir);

  console.log(`\n✅ SUCCESS: PCB Forge v${version} deployed successfully via ${cliTool}!`);
} catch (err) {
  console.error('\n❌ Deployment failed:', err.message);
  process.exit(1);
}