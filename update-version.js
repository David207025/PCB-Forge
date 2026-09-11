/**
 * update-version.js
 *
 * Synchronizes the version string across all package manifests in the
 * PCB Forge monorepo in a single command.
 *
 * Files updated:
 *   - package.json               (root workspace manifest)
 *   - pcbfapi/Cargo.toml         (Rust crate — [package] section only)
 *   - extension/package.json     (VS Code extension manifest)
 *   - extension/web-ui/package.json  (React web-ui manifest)
 *
 * Usage:
 *   node update-version.js <new-version>
 *   node update-version.js 0.4.0
 *
 * This script is also invoked automatically by release.sh before tagging.
 */

const fs = require('fs');
const path = require('path');

const rootDir = __dirname;

// Version string must be provided as the first CLI argument
const newVersion = process.argv[2];

if (!newVersion) {
  console.error('❌ Error: Please provide a version string.');
  console.log('💡 Usage: node update-version.js 0.2.0');
  process.exit(1);
}

/**
 * Reads a JSON file, sets `data.version` to `newVersion`, and writes it back.
 * Preserves existing formatting by using 2-space indentation.
 * @param {string} filePath - Absolute path to the JSON file
 */
function updateJsonFile(filePath) {
  if (fs.existsSync(filePath)) {
    const data = JSON.parse(fs.readFileSync(filePath, 'utf8'));
    data.version = newVersion;
    fs.writeFileSync(filePath, JSON.stringify(data, null, 2) + '\n', 'utf8');
    console.log(`✅ Updated version in: ${path.relative(rootDir, filePath)} -> ${newVersion}`);
  } else {
    console.warn(`⚠️ Warning: File not found: ${filePath}`);
  }
}

/**
 * Updates the `version = "x.x.x"` line inside the `[package]` section of a
 * Cargo.toml file using a regex that only touches the first match so workspace
 * member versions are not accidentally mutated.
 * @param {string} filePath - Absolute path to the Cargo.toml file
 */
function updateCargoToml(filePath) {
  if (fs.existsSync(filePath)) {
    let content = fs.readFileSync(filePath, 'utf8');
    // Regex targets `version = "x.x.x"` specifically within the [package] section
    const updatedContent = content.replace(
      /(\[package\][\s\S]*?version\s*=\s*)"[^"]+"/,
      `$1"${newVersion}"`
    );
    fs.writeFileSync(filePath, updatedContent, 'utf8');
    console.log(`✅ Updated version in: ${path.relative(rootDir, filePath)} -> ${newVersion}`);
  } else {
    console.warn(`⚠️ Warning: File not found: ${filePath}`);
  }
}

console.log(`🔄 Syncing all project files to version: ${newVersion}...\n`);

// ── 1. Root workspace package.json ────────────────────────────────────────────
updateJsonFile(path.join(rootDir, 'package.json'));

// ── 2. Rust CLI Cargo.toml ────────────────────────────────────────────────────
updateCargoToml(path.join(rootDir, 'pcbfapi', 'Cargo.toml'));

// ── 3. VS Code extension package.json ─────────────────────────────────────────
updateJsonFile(path.join(rootDir, 'extension', 'package.json'));

// ── 4. React web-ui package.json ──────────────────────────────────────────────
updateJsonFile(path.join(rootDir, 'extension', 'web-ui', 'package.json'));

console.log('\n🎉 All versions successfully synchronized!');