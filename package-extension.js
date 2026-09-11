/**
 * package-extension.js
 *
 * Full end-to-end packaging pipeline for the PCB Forge VS Code extension.
 * Run via: `node package-extension.js` or `pnpm package` from the repo root.
 *
 * Pipeline steps:
 *   0. Read the version number from extension/package.json
 *   1. Generate icons via generate_icons.py (Python 3 required)
 *   2. Install / sync all pnpm workspace dependencies
 *   3. Build the React web-ui and sync assets into extension/dist/
 *   4. Compile the extension TypeScript (extension/src/ → extension/out/)
 *   5. Package everything into build/pcb-forge-<version>.vsix
 */

const {execSync} = require('child_process');
const path = require('path');
const fs = require('fs');

const rootDir = __dirname;
const extensionDir = path.join(rootDir, 'extension');
const buildDir = path.join(rootDir, 'build');

/**
 * Runs a shell command synchronously, creating the target `cwd` directory if
 * it doesn't already exist. Logs the command before executing.
 * @param {string} command - Shell command to run
 * @param {string} cwd     - Working directory for the command
 */
function run(command, cwd) {
  if (!fs.existsSync(cwd)) {
    fs.mkdirSync(cwd, {recursive: true});
  }
  console.log(`\n⚙️ Running: ${command} (in ${path.relative(rootDir, cwd) || '.'}) `);
  execSync(command, {cwd, stdio: 'inherit', shell: true});
}

try {
  console.log('🚀 Starting PCB Forge Packaging Pipeline...');

  // ── Step 0: Resolve extension version ──────────────────────────────────────
  const extensionPkgPath = path.join(extensionDir, 'package.json');
  if (!fs.existsSync(extensionPkgPath)) {
    throw new Error(`Could not find extension package.json at ${extensionPkgPath}`);
  }
  const extensionPkg = JSON.parse(fs.readFileSync(extensionPkgPath, 'utf8'));
  const version = extensionPkg.version;
  console.log(`📦 Detected Extension Version: v${version}`);

  // ── Step 1: Generate crisp PNG icons from SVG source ───────────────────────
  console.log('\n🎨 Generating crisp icons using Python script...');
  const pythonScriptPath = path.join(rootDir, 'generate_icons.py');
  if (fs.existsSync(pythonScriptPath)) {
    run(`python3 ${pythonScriptPath}`, rootDir);
  } else {
    console.warn(`⚠️ Warning: Python script not found at ${pythonScriptPath}. Skipping icon generation.`);
  }

  // ── Step 2: Sync workspace dependencies ────────────────────────────────────
  console.log('\n📦 Ensuring workspace dependencies are up to date...');
  run('pnpm install', rootDir);

  // ── Step 3: Build React web-ui and copy assets into extension/dist/ ────────
  // Delegates to extension/build-webview.js via the build:webview script.
  console.log('\n🏗️ Building Web UI...');
  run('pnpm --filter pcb-forge run build:webview', rootDir);

  // ── Step 4: Compile extension TypeScript ───────────────────────────────────
  // Output goes to extension/out/ as specified by extension/tsconfig.json.
  console.log('\n⚙️ Compiling Extension TypeScript...');
  run('pnpm --filter pcb-forge run compile', rootDir);

  // ── Step 5: Package into a VSIX using vsce ─────────────────────────────────
  // The .vscodeignore file controls what ends up inside the archive.
  // web-ui/ is excluded; extension/dist/ (built assets) IS included.
  if (!fs.existsSync(buildDir)) {
    fs.mkdirSync(buildDir, {recursive: true});
  }
  const outputPath = path.join(buildDir, `pcb-forge-${version}.vsix`);
  run(`npx @vscode/vsce package --out "${outputPath}"`, extensionDir);

  console.log(`\n✅ SUCCESS: Extension packaged at: build/pcb-forge-${version}.vsix`);
} catch (err) {
  console.error('\n❌ Packaging failed:', err.message);
  process.exit(1);
}