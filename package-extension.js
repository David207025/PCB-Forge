const {execSync} = require('child_process');
const path = require('path');
const fs = require('fs');

const rootDir = __dirname;
const extensionDir = path.join(rootDir, 'extension');
const buildDir = path.join(rootDir, 'build');

function run(command, cwd) {
  if (!fs.existsSync(cwd)) {
    fs.mkdirSync(cwd, {recursive: true});
  }
  console.log(`\n⚙️ Running: ${command} (in ${path.relative(rootDir, cwd) || '.'})`);
  execSync(command, {cwd, stdio: 'inherit', shell: true});
}

try {
  console.log('🚀 Starting PCB Forge Packaging Pipeline...');

  // 0. Extract version dynamically from extension/package.json
  const extensionPkgPath = path.join(extensionDir, 'package.json');
  if (!fs.existsSync(extensionPkgPath)) {
    throw new Error(`Could not find extension package.json at ${extensionPkgPath}`);
  }
  const extensionPkg = JSON.parse(fs.readFileSync(extensionPkgPath, 'utf8'));
  const version = extensionPkg.version;
  console.log(`📦 Detected Extension Version: v${version}`);

  // 1. Run the Python icon generation script from the root directory
  console.log('\n🎨 Generating crisp icons using Python script...');
  const pythonScriptPath = path.join(rootDir, 'generate_icons.py');

  if (fs.existsSync(pythonScriptPath)) {
    run(`python3 ${pythonScriptPath}`, rootDir);
  } else {
    console.warn(`⚠️ Warning: Python script not found at ${pythonScriptPath}. Skipping icon generation.`);
  }

  // 2. Ensure all workspace dependencies are installed/synced via pnpm
  console.log('\n📦 Ensuring workspace dependencies are up to date...');
  run('pnpm install', rootDir);

  // 3. Build React Web UI & Sync Assets via workspace filter
  console.log('\n🏗️ Building Web UI...');
  run('pnpm --filter pcb-forge run build:webview', rootDir);

  // 4. Compile Extension TypeScript via workspace filter
  console.log('\n⚙️ Compiling Extension TypeScript...');
  run('pnpm --filter pcb-forge run compile', rootDir);

  // 5. Package extension into build/ with the dynamic version using pnpm dlx (replaces npx)
  if (!fs.existsSync(buildDir)) {
    fs.mkdirSync(buildDir, {recursive: true});
  }

  const outputPath = path.join(buildDir, `pcb-forge-${version}.vsix`);
  run(`pnpm dlx @vscode/vsce package --out "${outputPath}"`, extensionDir);

  console.log(`\n✅ SUCCESS: Extension packaged at: build/pcb-forge-${version}.vsix`);
} catch (err) {
  console.error('\n❌ Packaging failed:', err.message);
  process.exit(1);
}