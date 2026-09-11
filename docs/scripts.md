# PCB-Forge Scripts Documentation

> Auto-generated reference for workspace build and utility scripts.

## Script: `package-extension.js`

### `const`

package-extension.js
Full end-to-end packaging pipeline for the PCB Forge VS Code extension.
Run via: `node package-extension.js` or `pnpm package` from the repo root.
Pipeline steps:
0. Read the version number from extension/package.json
1. Generate icons via generate_icons.py (Python 3 required)
2. Install / sync all pnpm workspace dependencies
3. Build the React web-ui and sync assets into extension/dist/
4. Compile the extension TypeScript (extension/src/ → extension/out/)
5. Package everything into build/pcb-forge-<version>.vsix

### `function run(command, cwd)`

Runs a shell command synchronously, creating the target `cwd` directory if
it doesn't already exist. Logs the command before executing.
@param {string} command - Shell command to run
@param {string} cwd     - Working directory for the command

---

## Script: `deploy-extension.js`

### `const`

deploy-extension.js
Packages the PCB Forge extension and immediately installs it into a local
VS Code-compatible editor for quick testing.
Usage:
node deploy-extension.js [cliTool]
Arguments:
cliTool  (optional) — the VS Code CLI binary to use for install/uninstall.
Defaults to 'code'. Pass 'cursor' or 'code-insiders' as needed.
Example: node deploy-extension.js cursor
Pipeline steps:
1. Read extension version & publisher from extension/package.json
2. Package the extension into build/pcb-forge-<version>.vsix via vsce
3. Uninstall the previous version from the editor (if present)
4. Install the freshly built VSIX

### `function run(command, cwd)`

Runs a shell command synchronously, creating `cwd` if it doesn't exist.
@param {string} command - Shell command to execute
@param {string} cwd     - Working directory

---

## Script: `update-version.js`

### `const fs = require('fs');`

update-version.js
Synchronizes the version string across all package manifests in the
PCB Forge monorepo in a single command.
Files updated:
- package.json               (root workspace manifest)
- pcbfapi/Cargo.toml         (Rust crate — [package] section only)
- extension/package.json     (VS Code extension manifest)
- extension/web-ui/package.json  (React web-ui manifest)
Usage:
node update-version.js <new-version>
node update-version.js 0.4.0
This script is also invoked automatically by release.sh before tagging.

### `function updateJsonFile(filePath)`

Reads a JSON file, sets `data.version` to `newVersion`, and writes it back.
Preserves existing formatting by using 2-space indentation.
@param {string} filePath - Absolute path to the JSON file

### `function updateCargoToml(filePath)`

Updates the `version = "x.x.x"` line inside the `[package]` section of a
Cargo.toml file using a regex that only touches the first match so workspace
member versions are not accidentally mutated.
@param {string} filePath - Absolute path to the Cargo.toml file

---

## Script: `extension/build-webview.js`

### `const`

build-webview.js
Builds the React web-ui and copies the compiled output into extension/dist/,
which is the directory included in the packaged VSIX file.
Pipeline:
1. Run `pnpm --filter web-ui build` → produces web-ui/dist/
2. Wipe any stale extension/dist/ output
3. Copy web-ui/dist/ → extension/dist/
This script is invoked by the `build:webview` npm script in extension/package.json
and by the root package-extension.js packaging pipeline.

---

