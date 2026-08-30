const { execSync } = require('child_process');
const path = require('path');
const fs = require('fs');

const rootDir = __dirname;
const extensionDir = path.join(rootDir, 'extension');
const buildDir = path.join(rootDir, 'build');

// Accept CLI tool name as an argument, defaults to 'agy-ide'
const cliTool = process.argv[2] || 'agy-ide';

function run(command, cwd) {
  if (!fs.existsSync(cwd)) {
    fs.mkdirSync(cwd, { recursive: true });
  }
  console.log(`\n⚙️ Running: ${command}`);
  execSync(command, { cwd, stdio: 'inherit', shell: true });
}

try {
  console.log(`🚀 Packaging and Deploying via [ ${cliTool} ]...`);

  // 1. Extract extension version/publisher metadata
  const extensionPkgPath = path.join(extensionDir, 'package.json');
  if (!fs.existsSync(extensionPkgPath)) {
    throw new Error(`Could not find extension package.json at ${extensionPkgPath}`);
  }
  const extensionPkg = JSON.parse(fs.readFileSync(extensionPkgPath, 'utf8'));
  const extensionId = `${extensionPkg.publisher}.${extensionPkg.name}`;
  const version = extensionPkg.version;

  // 2. Package into build/
  if (!fs.existsSync(buildDir)) {
    fs.mkdirSync(buildDir, { recursive: true });
  }
  const outputPath = path.join(buildDir, `pcb-forge-${version}.vsix`);

  if (fs.existsSync(outputPath)) {
    fs.unlinkSync(outputPath);
  }

  run(`pnpm dlx @vscode/vsce package --out "${outputPath}"`, extensionDir);

  // 3. Uninstall previous version (ignoring errors if it wasn't installed)
  console.log(`\n🗑️ Uninstalling previous version of ${extensionId}...`);
  try {
    execSync(`${cliTool} --uninstall-extension ${extensionId}`, { stdio: 'inherit', shell: true });
  } catch (e) {
    console.log(`⚠️ Note: Extension not previously installed or removal skipped.`);
  }

  // 4. Install the fresh VSIX package
  console.log(`\n📥 Installing new VSIX...`);
  run(`${cliTool} --install-extension "${outputPath}" --force`, rootDir);

  console.log(`\n✅ SUCCESS: PCB Forge v${version} deployed successfully via ${cliTool}!`);
} catch (err) {
  console.error('\n❌ Deployment failed:', err.message);
  process.exit(1);
}