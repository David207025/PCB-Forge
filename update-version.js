const fs = require('fs');
const path = require('path');

const rootDir = __dirname;
const newVersion = process.argv[2];

if (!newVersion) {
  console.error('❌ Error: Please provide a version string.');
  console.log('💡 Usage: node update-version.js 0.2.0');
  process.exit(1);
}

// Helper to update JSON files safely
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

// Helper to update Cargo.toml version field under [package]
function updateCargoToml(filePath) {
  if (fs.existsSync(filePath)) {
    let content = fs.readFileSync(filePath, 'utf8');
    // Regex targets version = "x.x.x" specifically under the [package] section
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

// 1. Root package.json
updateJsonFile(path.join(rootDir, 'package.json'));

// 2. Rust Cargo.toml
updateCargoToml(path.join(rootDir, 'pcbfapi', 'Cargo.toml'));

// 3. Extension package.json
updateJsonFile(path.join(rootDir, 'extension', 'package.json'));

// 4. Web-UI package.json
updateJsonFile(path.join(rootDir, 'extension', 'web-ui', 'package.json'));

console.log('\n🎉 All versions successfully synchronized!');