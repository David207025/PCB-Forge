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

# 2. Push current code changes to main first
echo "📤 Staging and pushing latest code changes to GitHub..."
git add .

# Check if there are actually changes to commit
if git diff-index --quiet HEAD --; then
  echo "ℹ️  No uncommitted changes found, skipping commit."
else
  echo "💬 Enter your commit message (or press Enter for default):"
  read -r COMMIT_MSG
  if [ -z "$COMMIT_MSG" ]; then
    COMMIT_MSG="chore: release $TAG"
  fi
  git commit -m "$COMMIT_MSG"
fi

git push origin main

# 3. Delete old tags (ignores errors if the tag doesn't exist yet)
echo "🗑️  Deleting old local tag..."
git tag -d "$TAG" 2>/dev/null || true

echo "☁️  Deleting old remote tag..."
git push --delete origin "$TAG" 2>/dev/null || true

# 4. Create and push new tag on the current commit
echo "✨ Creating new tag $TAG on current commit..."
git tag "$TAG"
git push origin "$TAG"

# 5. Build targets locally using cargo-zigbuild
echo "🔨 Building targets with cargo-zigbuild..."
TARGETS=(
  "aarch64-apple-darwin"
  "x86_64-apple-darwin"
  "aarch64-unknown-linux-gnu"
  "x86_64-unknown-linux-gnu"
  "x86_64-pc-windows-gnu" # Using -gnu for windows via zigbuild avoids MSVC toolchain requirements on Mac
)

for target in "${TARGETS[@]}"; do
  echo "🚀 Compiling for $target..."
  cargo zigbuild --release --target "$target"
done

# 6. Package artifacts into a dist folder
DIST_DIR="target/distrib"
mkdir -p "$DIST_DIR"

echo "📦 Packaging artifacts..."
for target in "${TARGETS[@]}"; do
  # Adjust binary name if needed (e.g., your binary executable name)
  BIN_NAME="pcbfapi"
  EXT=""
  if [[ "$target" == *"windows"* ]]; then
    EXT=".exe"
  fi

  SRC_PATH="target/$target/release/$BIN_NAME$EXT"
  if [ -f "$SRC_PATH" ]; then
    ARCHIVE_NAME="${BIN_NAME}-${TAG}-${target}"
    if [[ "$target" == *"windows"* ]]; then
      zip -j "$DIST_DIR/${ARCHIVE_NAME}.zip" "$SRC_PATH"
    else
      tar -czf "$DIST_DIR/${ARCHIVE_NAME}.tar.gz" -C "target/$target/release" "$BIN_NAME"
    fi
  fi
done

# 7. Create GitHub Release and upload built assets
echo "☁️ Publishing release to GitHub..."
gh release create "$TAG" "$DIST_DIR"/* --title "$TAG" --notes "Automated local release for $TAG"

echo "✅ Done! All binaries built via zigbuild and published to GitHub release $TAG."