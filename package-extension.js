const {execSync} = require('child_process');
const path = require('path');
const fs = require('fs');

const rootDir = __dirname;
const cliDir = path.join(rootDir, 'cli');
const extensionDir = path.join(rootDir, 'extension');
const binariesDir = path.join(extensionDir, 'binaries');
const webviewDir = path.join(extensionDir, 'web-ui');
const buildDir = path.join(rootDir, 'build');

function run(command, cwd) {
  if (!fs.existsSync(cwd)) {
    fs.mkdirSync(cwd, {recursive: true});
  }
  console.log(`\n⚙️ Running: ${command} (in ${path.relative(rootDir, cwd) || '.'})`);
  execSync(command, {cwd, stdio: 'inherit', shell: true});
}

// Targets list with explicit flag for native vs cross
const targets = [
  {triple: 'aarch64-apple-darwin', name: 'pcbfcli-darwin-arm64', isNative: process.arch === 'arm64'},
  {triple: 'x86_64-apple-darwin', name: 'pcbfcli-darwin-x64', isNative: process.arch === 'x64'},
  {triple: 'x86_64-pc-windows-gnu', name: 'pcbfcli-win32-x64.exe', isNative: false},
  {triple: 'x86_64-unknown-linux-gnu', name: 'pcbfcli-linux-x64', isNative: false},
  {triple: 'aarch64-unknown-linux-gnu', name: 'pcbfcli-linux-arm64', isNative: false},
];

try {
  console.log('🚀 Starting PCB Forge Packaging...');

  // Ensure binaries folder exists
  if (!fs.existsSync(binariesDir)) {
    fs.mkdirSync(binariesDir, {recursive: true});
  }

  // 1. Compile Rust CLI binaries across targets
  if (fs.existsSync(cliDir)) {
    console.log('\n🦀 Compiling Rust CLI binaries...');
    for (const {triple, name, isNative} of targets) {
      try {
        const cmd = isNative
          ? `cargo build --release --target ${triple}`
          : `cross build --release --target ${triple}`;

        run(cmd, cliDir);

        const binaryExt = triple.includes('windows') ? 'pcbfcli.exe' : 'pcbfcli';
        const srcPath = path.join(cliDir, 'target', triple, 'release', binaryExt);
        const destPath = path.join(binariesDir, name);

        console.log(`🔍 Looking for built binary at: ${srcPath}`);
        if (fs.existsSync(srcPath)) {
          fs.copyFileSync(srcPath, destPath);
          console.log(`  ✅ Successfully copied to: binaries/${name}`);
        } else {
          console.warn(`  ⚠️ Warning: Built binary not found at ${srcPath}`);
        }
      } catch (err) {
        console.error(`  ❌ Failed to build target ${triple}: ${err.message}`);
      }
    }
  }

  // 2. Build React Web UI
  if (fs.existsSync(path.join(webviewDir, 'package.json'))) {
    run('npm install', webviewDir);
    run('npm run build', webviewDir);
  }

  // 3. Compile Extension TypeScript
  if (fs.existsSync(path.join(extensionDir, 'package.json'))) {
    run('npm install', extensionDir);
    run('npm run compile', extensionDir);
  }

  // 4. Package extension into build/
  if (!fs.existsSync(buildDir)) {
    fs.mkdirSync(buildDir, {recursive: true});
  }

  const outputPath = path.join(buildDir, 'pcb-forge-0.1.0.vsix');
  run(`npx @vscode/vsce package --out "${outputPath}"`, extensionDir);

  console.log(`\n✅ SUCCESS: Extension packaged at: build/pcb-forge-0.1.0.vsix`);
} catch (err) {
  console.error('\n❌ Packaging failed:', err.message);
  process.exit(1);
}