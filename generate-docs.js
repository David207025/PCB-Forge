/**
 * generate-docs.js
 *
 * Scans the Rust source files (pcbfapi/src), TypeScript extension source,
 * and project utility scripts to generate clean Markdown documentation in docs/.
 *
 * Usage:
 *   node generate-docs.js
 */

const fs = require('fs');
const path = require('path');

const rootDir = __dirname;
const docsDir = path.join(rootDir, 'docs');

if (!fs.existsSync(docsDir)) {
  fs.mkdirSync(docsDir, { recursive: true });
}

/**
 * Extracts Rust doc comments (//! and ///) and signatures from a Rust file.
 */
function parseRustFile(filePath) {
  const content = fs.readFileSync(filePath, 'utf8');
  const lines = content.split('\n');
  const fileName = path.basename(filePath);
  
  let moduleDocs = [];
  let items = [];
  let currentDoc = [];
  let inModuleDocs = true;

  for (let i = 0; i < lines.length; i++) {
    const line = lines[i];
    const trimmed = line.trim();

    if (trimmed.startsWith('//!')) {
      if (inModuleDocs) {
        moduleDocs.push(trimmed.replace(/^\/\/!\s?/, ''));
      }
      continue;
    }

    if (trimmed.length > 0 && !trimmed.startsWith('//')) {
      inModuleDocs = false;
    }

    if (trimmed.startsWith('///')) {
      currentDoc.push(trimmed.replace(/^\/\/\/\s?/, ''));
    } else if (currentDoc.length > 0) {
      // Check if this line is an item declaration (fn, struct, enum, pub trait, type)
      if (
        trimmed.startsWith('pub fn ') ||
        trimmed.startsWith('pub async fn ') ||
        trimmed.startsWith('pub struct ') ||
        trimmed.startsWith('pub enum ') ||
        trimmed.startsWith('pub type ') ||
        trimmed.startsWith('fn ') ||
        trimmed.startsWith('struct ') ||
        trimmed.startsWith('enum ')
      ) {
        // Collect multi-line signature up to '{' or ';'
        let sig = trimmed;
        let j = i;
        while (!sig.includes('{') && !sig.endsWith(';') && j + 1 < lines.length) {
          j++;
          sig += ' ' + lines[j].trim();
        }
        sig = sig.replace(/\{.*$/, '').replace(/;$/, '').trim();

        items.push({
          signature: sig,
          doc: currentDoc.join('\n'),
          line: i + 1,
        });
        currentDoc = [];
      } else if (!trimmed.startsWith('#[')) {
        currentDoc = [];
      }
    }
  }

  return {
    fileName,
    moduleDoc: moduleDocs.join('\n'),
    items,
  };
}

/**
 * Extracts JSDoc / TS doc comments and function/class signatures.
 */
function parseJsTsFile(filePath) {
  const content = fs.readFileSync(filePath, 'utf8');
  const fileName = path.basename(filePath);
  const relPath = path.relative(rootDir, filePath);
  
  const items = [];
  // Regex to find JSDoc blocks followed by function/class/const declaration
  const jsdocRegex = /\/\*\*([\s\S]*?)\*\/\s*\n([^\n]+)/g;
  let match;

  while ((match = jsdocRegex.exec(content)) !== null) {
    const rawDoc = match[1];
    const nextLine = match[2].trim();

    const cleanDoc = rawDoc
      .split('\n')
      .map(l => l.replace(/^\s*\*\s?/, '').trim())
      .filter(l => l.length > 0)
      .join('\n');

    if (nextLine.length > 0) {
      items.push({
        signature: nextLine.replace(/\{.*$/, '').trim(),
        doc: cleanDoc,
      });
    }
  }

  return {
    fileName,
    relPath,
    items,
  };
}

console.log('📚 Generating documentation...');

// 1. Rust pcbfapi documentation
const rustFiles = ['main.rs', 'forge.rs', 'definitions.rs', 'tray.rs'].map(f =>
  path.join(rootDir, 'pcbfapi', 'src', f)
);

let rustMd = '# PCB-Forge API (pcbfapi) Documentation\n\n';
rustMd += '> Auto-generated from Rust source doc comments.\n\n';

for (const f of rustFiles) {
  if (!fs.existsSync(f)) continue;
  const parsed = parseRustFile(f);
  rustMd += `## Module: \`${parsed.fileName}\`\n\n`;
  if (parsed.moduleDoc) {
    rustMd += `${parsed.moduleDoc}\n\n`;
  }

  if (parsed.items.length > 0) {
    rustMd += `### Declarations & Functions\n\n`;
    for (const item of parsed.items) {
      rustMd += `#### \`${item.signature}\`\n\n`;
      rustMd += `${item.doc}\n\n`;
      rustMd += `---\n\n`;
    }
  }
}

fs.writeFileSync(path.join(docsDir, 'pcbfapi.md'), rustMd, 'utf8');
console.log('✅ Generated docs/pcbfapi.md');

// 2. Scripts documentation
const scripts = [
  'package-extension.js',
  'deploy-extension.js',
  'update-version.js',
  'extension/build-webview.js',
];

let scriptsMd = '# PCB-Forge Scripts Documentation\n\n';
scriptsMd += '> Auto-generated reference for workspace build and utility scripts.\n\n';

for (const s of scripts) {
  const fullPath = path.join(rootDir, s);
  if (!fs.existsSync(fullPath)) continue;
  const parsed = parseJsTsFile(fullPath);
  scriptsMd += `## Script: \`${s}\`\n\n`;
  if (parsed.items.length > 0) {
    for (const item of parsed.items) {
      scriptsMd += `### \`${item.signature}\`\n\n`;
      scriptsMd += `${item.doc}\n\n`;
    }
  } else {
    scriptsMd += `*(No JSDoc blocks found)*\n\n`;
  }
  scriptsMd += `---\n\n`;
}

fs.writeFileSync(path.join(docsDir, 'scripts.md'), scriptsMd, 'utf8');
console.log('✅ Generated docs/scripts.md');

// 3. Documentation Index
let indexMd = '# PCB-Forge Technical Documentation\n\n';
indexMd += 'Welcome to the PCB-Forge internal documentation.\n\n';
indexMd += '- [Rust API Backend (`pcbfapi`)](./pcbfapi.md) — Axum HTTP server, Typst compiler world, KiCad asset exporter, and system tray.\n';
indexMd += '- [Scripts & Automation](./scripts.md) — Build, packaging, deploy, and version sync scripts.\n';

fs.writeFileSync(path.join(docsDir, 'README.md'), indexMd, 'utf8');
console.log('✅ Generated docs/README.md');

console.log('🎉 Documentation generation completed in docs/ folder.');
