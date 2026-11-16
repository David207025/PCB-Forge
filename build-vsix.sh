#!/bin/bash
# ==============================================================================
# build-vsix.sh — Local VS Code / Antigravity extension build & VSIX packager
# ==============================================================================
#
# Usage:
#   ./build-vsix.sh            # Builds webview, compiles TS, and outputs .vsix
#   ./build-vsix.sh --install  # Installs the built .vsix into VS Code & Antigravity
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

echo "🔨 Building frontend webview and extension..."
pnpm run build

echo "📦 Packaging VS Code extension VSIX..."
node package-extension.js

VERSION=$(node -p "require('./extension/package.json').version")
VSIX_PATH="build/pcb-forge-${VERSION}.vsix"

if [ "${1:-}" = "--install" ]; then
  INSTALLED=0

  for cli in code agy-ide; do
    if command -v "$cli" &> /dev/null; then
      echo "📥 Installing $VSIX_PATH into $cli..."
      if "$cli" --install-extension "$VSIX_PATH" --force; then
        INSTALLED=$((INSTALLED + 1))
      else
        echo "⚠️  Failed to install into $cli."
      fi
    fi
  done

  if [ "$INSTALLED" -gt 0 ]; then
    echo "✅ Installed! Reload your IDE window (Cmd+Shift+P -> Developer: Reload Window) to test."
  else
    echo "⚠️  Neither 'code' nor 'agy-ide' CLI was found in PATH. Install $VSIX_PATH manually via Extensions -> Install from VSIX..."
  fi
else
  echo "✅ Ready at: $VSIX_PATH"
  echo "   Run './build-vsix.sh --install' to install it directly into VS Code and Antigravity."
fi>