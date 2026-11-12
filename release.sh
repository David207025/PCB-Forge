#!/bin/bash
# ==============================================================================
# release.sh — Hybrid local/Docker release automation script for PCB Forge
# ==============================================================================
#
# Automates the complete end-to-end release process:
#   1. (Optional) Syncs version across all package manifests (update-version.js)
#   2. Regenerates Markdown API docs in docs/ (generate-docs.js)
#   3. Builds web-ui and compiles the VS Code extension
#   4. Packages the VSIX extension bundle (package-extension.js)
#   5. Publishes VSIX to VS Code Marketplace (if VSCE_PAT is available)
#   6. Commits and pushes changes to main branch
#   7. Compiles macOS binaries natively and Windows binary via Docker
#   8. Uploads prebuilt artifacts to a draft GitHub release and pushes the tag
#      to trigger lightweight Linux CI (global installers + Homebrew tap)
#
# Usage:
#   ./release.sh           # Uses current version in pcbfapi/Cargo.toml
#   ./release.sh 0.4.0     # Bumps to 0.4.0 first, then runs release
# ==============================================================================

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

NEW_VERSION="${1:-}"
VSCE_PAT="${VSCE_PAT:-}"
WIN_BUILDER_IMAGE="pcbfapi-win-builder"

# Preflight checks
for cmd in gh docker dist rustup; do
  if ! command -v "$cmd" &> /dev/null; then
    echo "❌ Required tool '$cmd' is not installed or not in PATH."
    exit 1
  fi
done
gh auth status >/dev/null

# Step 1: Version update if argument provided
if [ -n "$NEW_VERSION" ]; then
  echo "🔄 Updating version to $NEW_VERSION across all manifests..."
  node update-version.js "$NEW_VERSION"
fi

# Extract current version
VERSION=$(grep -m 1 '^version = ' pcbfapi/Cargo.toml | cut -d '"' -f 2)
if [ -z "$VERSION" ]; then
  echo "❌ Could not find version in pcbfapi/Cargo.toml"
  exit 1
fi

TAG="v$VERSION"
echo "🚀 Starting full release workflow for $TAG"

# Step 2: Regenerate documentation
echo "📚 Generating documentation in docs/..."
node generate-docs.js

# Step 3: Build webview & compile extension
echo "🔨 Building frontend webview and extension..."
pnpm run build

# Step 4: Package VSIX
echo "📦 Packaging VS Code extension VSIX..."
node package-extension.js

# Step 5: Publish extension to VS Code Marketplace (if token is available)
if [ -n "$VSCE_PAT" ]; then
  echo "🚀 Publishing extension to VS Code Marketplace..."
  (cd extension && npx @vscode/vsce publish -p "$VSCE_PAT")
else
  echo "ℹ️  VSCE_PAT is not set. Skipping marketplace upload."
  echo "   (To publish to the VS Code Marketplace, set VSCE_PAT=<your-pat>)"
fi

# Step 6: Commit and push changes
echo "📝 Checking git status..."
if [ -n "$(git status --porcelain)" ]; then
  git add .
  git commit -m "chore(release): $TAG"
fi

echo "⬆️ Pushing changes to origin main..."
git push origin main

# Step 7: Compile macOS natively & Windows in Docker
echo "🛠️  Building release binaries locally (macOS) and in Docker (Windows)..."
rm -rf target/distrib
mkdir -p target/distrib

TMP_MANIFEST_DIR="$(mktemp -d)"
trap 'rm -rf "$TMP_MANIFEST_DIR"' EXIT

# Ensure Intel macOS rustup target is installed
rustup target add x86_64-apple-darwin >/dev/null

# Build Docker cross-compiler image once if not cached locally
if ! docker image inspect "$WIN_BUILDER_IMAGE" >/dev/null 2>&1; then
  echo "🐳 Building $WIN_BUILDER_IMAGE Docker image..."
  docker build -f Dockerfile.win -t "$WIN_BUILDER_IMAGE" .
fi

echo "🍎 Building aarch64-apple-darwin..."
RUST_BACKTRACE=1 dist build --tag="$TAG" --output-format=json --artifacts=local --target=aarch64-apple-darwin \
  > "$TMP_MANIFEST_DIR/aarch64-apple-darwin-dist-manifest.json"
mv "$TMP_MANIFEST_DIR/aarch64-apple-darwin-dist-manifest.json" target/distrib/

echo "🍎 Building x86_64-apple-darwin..."
RUST_BACKTRACE=1 dist build --tag="$TAG" --output-format=json --artifacts=local --target=x86_64-apple-darwin \
  > "$TMP_MANIFEST_DIR/x86_64-apple-darwin-dist-manifest.json"
mv "$TMP_MANIFEST_DIR/x86_64-apple-darwin-dist-manifest.json" target/distrib/

echo "🪟 Building x86_64-pc-windows-msvc in Docker..."
docker run --rm \
  -v "$SCRIPT_DIR:/work" \
  -v pcbfapi-win-target:/target-win \
  -e CARGO_TARGET_DIR=/target-win \
  "$WIN_BUILDER_IMAGE" \
  bash -c "set -euo pipefail; dist build --tag='$TAG' --output-format=json --artifacts=local --target=x86_64-pc-windows-msvc > /tmp/x86_64-pc-windows-msvc-dist-manifest.json && cp /target-win/distrib/* /work/target/distrib/ && mv /tmp/x86_64-pc-windows-msvc-dist-manifest.json /work/target/distrib/"

# Step 8: Clean up old release/tags, stage prebuilt artifacts in a draft release, and push tag
echo "🔍 Checking for existing GitHub Release for $TAG..."
if gh release view "$TAG" &> /dev/null; then
  echo "🗑️  Deleting existing GitHub Release $TAG..."
  gh release delete "$TAG" --yes --cleanup-tag 2>/dev/null || true
  echo "✅ Existing release deleted."
fi

if git rev-parse "$TAG" >/dev/null 2>&1; then
  echo "🗑️  Deleting local tag $TAG..."
  git tag -d "$TAG" 2>/dev/null || true
fi

if git ls-remote --tags origin | grep -q "refs/tags/$TAG"; then
  echo "🗑️  Deleting remote tag $TAG..."
  git push origin ":refs/tags/$TAG" 2>/dev/null || true
fi

echo "📤 Uploading prebuilt artifacts to draft release $TAG..."
gh release create "$TAG" \
  --draft \
  --title "Release $TAG (Staging)" \
  --notes "Staging prebuilt local artifacts for CI packaging." \
  target/distrib/*

echo "🏷️  Creating fresh tag $TAG..."
git tag -a "$TAG" -m "Release $TAG"
echo "⬆️ Pushing tag $TAG to origin (triggers CI packaging & Homebrew tap update)..."
git push origin "$TAG"

echo ""
echo "🎉 Local builds uploaded and tag $TAG pushed!"
echo "   GitHub Actions (ubuntu-22.04) will now generate installers, publish the release, and update Homebrew."