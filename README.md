# PCB Forge ⚡

A comprehensive, developer-centric documentation toolchain for hardware and PCB engineers. **PCB Forge** integrates a **VS Code extension**, a **native Rust backend (`pcbfapi`)**, automated **KiCad CLI** exports, and an in-memory **Typst** compilation engine to generate publication-grade PDF documentation, manufacturing packets, and schematic sheets directly from your EDA project files.

---

## 🏗️ Architecture Overview

The PCB Forge monorepo is structured into three primary components:

```
PCB-Forge/
├── pcbfapi/                   # Native Rust background daemon & HTTP server
│   ├── src/
│   │   ├── main.rs            # Axum HTTP server (port 47210) & Tao system tray
│   │   ├── forge.rs           # KiCad CLI runner, asset caching & Typst compiler
│   │   ├── definitions.rs     # Typst World implementation & project schema models
│   │   └── tray.rs            # Native macOS / Windows system tray icon & menu
│   └── typst/                 # Vendored Typst packages (e.g., cmarker plugin)
│
├── extension/                 # VS Code extension (TypeScript)
│   ├── src/extension.ts       # Extension entry point, command palette & webview provider
│   ├── web-ui/                # Modern React + Vite frontend for template & project editing
│   └── package.json           # Extension manifest & configuration
│
├── docs/                      # Auto-generated Markdown documentation (pcbfapi.md, scripts.md)
└── scripts                    # Workspace build, release, and packaging utilities
```

### How It Works
1. **The Extension (`extension/`)**: Launches when you open a PCB project in VS Code. It provides a visual UI (`extension/web-ui`) for editing templates, selecting page layouts, and configuring fields.
2. **The API Daemon (`pcbfapi`)**: A lightweight background service written in Rust. It runs an Axum HTTP server listening on `127.0.0.1:47210` with a native OS system tray icon indicating compile status.
3. **Asset Generation**: Whenever a project build is triggered, `pcbfapi` invokes `kicad-cli` to render vector SVG exports of schematic sheets and copper/silk layers.
4. **Typst Compilation**: Hardware schematics, layer SVGs, markdown notes, and project metadata are stitched together using custom Typst templates and compiled in-memory to vector PDFs.

---

## 🛠️ Subscripts Tutorial & Reference

The project includes several automation scripts at the repository root. Below is a detailed guide on what each script does and when to use it:

### 1. `deploy-extension.js` — Local Extension Development
* **Command:** `pnpm deploy` or `node deploy-extension.js`
* **What it does:**
  1. Runs `pnpm build` (which compiles `extension/web-ui` with Vite and compiles extension TypeScript with `tsc`).
  2. Runs `node package-extension.js` to create the `.vsix` installer package.
  3. Automatically executes `code --install-extension <path-to-vsix> --force` to install the newly built extension directly into your local VS Code.
* **When to use:** Whenever you make changes to the VS Code extension or webview and want to test them immediately in VS Code.

### 2. `package-extension.js` — VSIX Package Bundler
* **Command:** `pnpm package` or `node package-extension.js`
* **What it does:**
  1. Copies the built webview bundle from `extension/web-ui/dist/` into `extension/web/`.
  2. Invokes `@vscode/vsce package` with clean exclusions (ignoring node_modules, source TS files, and web-ui source code) to produce a lean `.vsix` file in the `build/` directory.
* **When to use:** When you need a clean `.vsix` bundle for distribution without installing it locally.

### 3. `update-version.js` — Monorepo Version Synchronizer
* **Command:** `node update-version.js <new-version>` (e.g. `node update-version.js 0.4.0`)
* **What it does:**
  * Synchronizes the semantic version across all four project manifests in one pass:
    - Root `package.json`
    - Rust crate `pcbfapi/Cargo.toml` (`[package]` section)
    - Extension manifest `extension/package.json`
    - Frontend manifest `extension/web-ui/package.json`
* **When to use:** Before making a new release or when bumping version numbers.

### 4. `generate-docs.js` — Automated Markdown Documentation Generator
* **Command:** `pnpm docs` or `node generate-docs.js`
* **What it does:**
  1. Scans all Rust source files in `pcbfapi/src/` (`main.rs`, `forge.rs`, `definitions.rs`, `tray.rs`), extracting module-level documentation (`//!`) and item doc comments (`///`).
  2. Scans TypeScript and JavaScript scripts for JSDoc comment blocks (`/** ... */`).
  3. Writes comprehensive Markdown documentation files into the `docs/` folder:
     - `docs/pcbfapi.md`: Rust API handlers, endpoints, structs, and compiler world methods.
     - `docs/scripts.md`: Script functions and packaging documentation.
     - `docs/README.md`: Technical documentation index.
* **When to use:** Whenever you add or modify Rust functions, API endpoints, or scripts, or automatically as part of the release workflow.

### 5. `release.sh` — Unified End-to-End Release Pipeline
* **Command:** `./release.sh [new-version]` (e.g. `./release.sh 0.4.0` or `./release.sh`)
* **What it does:**
  1. Updates the version across all manifests if a version parameter is passed.
  2. Runs `node generate-docs.js` to update documentation in `docs/`.
  3. Builds frontend webview and compiles extension TypeScript (`pnpm run build`).
  4. Packages the `.vsix` extension bundle.
  5. Publishes the extension to the VS Code Marketplace if `VSCE_PAT` is defined.
  6. Commits modified files and pushes to `origin main`.
  7. Cleans up any existing GitHub release/tag for this version (using `gh release delete`).
  8. Creates and pushes a fresh Git tag (`vX.Y.Z`), triggering the GitHub Actions `cargo-dist` workflow to build cross-platform binaries, installers, and update the Homebrew tap.
* **When to use:** When you are ready to publish a new official release of PCB Forge.

### 6. `retag.sh` — CI Re-trigger Utility
* **Command:** `./retag.sh`
* **What it does:**
  * Reads the current version from `pcbfapi/Cargo.toml`.
  * Deletes any existing GitHub Release for that tag using the `gh` CLI.
  * Deletes the local Git tag and the remote Git tag on GitHub.
  * Recreates the tag at the current commit and pushes it to GitHub.
* **When to use:** If a GitHub Actions build failed on a tag or you need to re-trigger CI for the same version without bumping the version number.

### 7. `wipe-tag.sh` — Tag Teardown Utility
* **Command:** `./wipe-tag.sh`
* **What it does:**
  * Extracts the current version from `pcbfapi/Cargo.toml` and permanently deletes the tag locally and on GitHub remote without recreating it.
* **When to use:** When canceling a release or removing an accidental tag.

### 8. `install-api.sh` — Local CLI Daemon Installation
* **Command:** `./install-api.sh`
* **What it does:**
  * Runs `cargo build --release` inside `pcbfapi/` and copies the compiled binary to `~/.pcb-forge/bin/pcbfapi`.
* **When to use:** When developing or testing the Rust daemon natively on your system.

### 9. `generate_icons.py` — Icon Generator
* **Command:** `python3 generate_icons.py`
* **What it does:**
  * Uses Pillow / Python to generate multi-resolution PNG and icon assets from the master icon files for the system tray and extension.

---

## 🚀 Quick Start for Development

### Prerequisites
- **Node.js** (v18+) & **pnpm**
- **Rust** & **Cargo** (latest stable)
- **KiCad 7+ or 8+** (with `kicad-cli` on PATH)
- **GitHub CLI (`gh`)** (optional, for release management)

### Local Build & Test Loop

1. **Install Dependencies:**
   ```bash
   pnpm install
   ```

2. **Check & Build Rust API Backend:**
   ```bash
   cargo check --manifest-path pcbfapi/Cargo.toml
   cargo build --manifest-path pcbfapi/Cargo.toml
   ```

3. **Develop the VS Code Extension & Webview:**
   ```bash
   # Build frontend and install extension into VS Code:
   pnpm deploy
   ```

4. **Generate Documentation:**
   ```bash
   pnpm docs
   ```

---

## 📄 License

This project is licensed under the ISC License.
