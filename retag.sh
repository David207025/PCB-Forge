#!/bin/bash

# Exit immediately if any command fails
set -e

# 1. Extract the version from pcbfapi/Cargo.toml
VERSION=$(grep -m 1 '^version = ' pcbfapi/Cargo.toml | cut -d '"' -f 2)

if [ -z "$VERSION" ]; then
  echo "❌ Could not find version in pcbfapi/Cargo.toml"
  exit 1
fi

TAG="v$VERSION"
echo "📦 Found version: $VERSION"
echo "🏷️  Managing tag: $TAG"

# 2. Push code & manage git tags
echo "📤 Pushing latest code changes to GitHub..."
git add .
if ! git diff-index --quiet HEAD --; then
  git commit -m "chore: release $TAG"
fi
git push origin main

git tag -d "$TAG" 2>/dev/null || true
git push --delete origin "$TAG" 2>/dev/null || true

git tag "$TAG"
git push origin "$TAG"

# 3. Prepare distribution folder
DIST_DIR="target/distrib"
mkdir -p "$DIST_DIR"
BIN_NAME="pcbfapi"

# 4. Build Linux targets natively inside Docker
echo "🐳 Building Linux targets inside Docker container..."
docker build -f Dockerfile.build -t pcb-builder-linux .

# Build x86_64 Linux
docker run --rm -v "$(pwd)":/app -e PKG_CONFIG_ALLOW_CROSS=1 pcb-builder-linux cargo build --release --target x86_64-unknown-linux-gnu
tar -czf "$DIST_DIR/${BIN_NAME}-${TAG}-x86_64-unknown-linux-gnu.tar.gz" -C target/x86_64-unknown-linux-gnu/release "$BIN_NAME"

# Build aarch64 Linux
docker run --rm -v "$(pwd)":/app -e PKG_CONFIG_ALLOW_CROSS=1 pcb-builder-linux cargo build --release --target aarch64-unknown-linux-gnu
tar -czf "$DIST_DIR/${BIN_NAME}-${TAG}-aarch64-unknown-linux-gnu.tar.gz" -C target/aarch64-unknown-linux-gnu/release "$BIN_NAME"


# 5. Build macOS targets natively on your Mac
echo "🍏 Building macOS targets natively..."
cargo build --release --target x86_64-apple-darwin
cargo build --release --target aarch64-apple-darwin

tar -czf "$DIST_DIR/${BIN_NAME}-${TAG}-x86_64-apple-darwin.tar.gz" -C target/x86_64-apple-darwin/release "$BIN_NAME"
tar -czf "$DIST_DIR/${BIN_NAME}-${TAG}-aarch64-apple-darwin.tar.gz" -C target/aarch64-apple-darwin/release "$BIN_NAME"

# 6. Build Windows target via cargo-zigbuild or standard Windows toolchain
echo "🪟 Building Windows target..."
cargo zigbuild --release --target x86_64-pc-windows-gnu
zip -j "$DIST_DIR/${BIN_NAME}-${TAG}-x86_64-pc-windows-gnu.zip" "target/x86_64-pc-windows-gnu/release/$BIN_NAME.exe"

# 7. Publish to GitHub Release
echo "☁️ Publishing release to GitHub..."
gh release create "$TAG" "$DIST_DIR"/* --title "$TAG" --notes "Local release for $TAG built via Docker & macOS native."

echo "✅ Done! All platforms successfully built and published."