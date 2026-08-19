const {execSync} = require('child_process');
const path = require('path');
const fs = require('fs');

const rootDir = __dirname;
const extensionDir = path.join(rootDir, 'extension');
const webviewDir = path.join(extensionDir, 'web-ui');
const buildDir = path.join(rootDir, 'build');

function run(command, cwd) {
  if (!fs.existsSync(cwd)) {
    fs.mkdirSync(cwd, {recursive: true});
  }
  console.log(`\n⚙️ Running: ${command} (in ${path.relative(rootDir, cwd) || '.'})`);
  execSync(command, {cwd, stdio: 'inherit', shell: true});
}

try {
  console.log('🚀 Starting PCB Forge Packaging (Skipping local CLI builds)...');

  // 1. Build React Web UI
  if (fs.existsSync(path.join(webviewDir, 'package.json'))) {
    run('npm install', webviewDir);
    run('npm run build', webviewDir);
  }

  // 2. Compile Extension TypeScript
  if (fs.existsSync(path.join(extensionDir, 'package.json'))) {
    run('npm install', extensionDir);
    run('npm run compile', extensionDir);
  }

  // 3. Package extension into build/
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