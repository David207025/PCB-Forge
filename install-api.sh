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

# 2. Ensure cargo-dist is initialized for GitHub Actions (safe to run multiple times)
if [ ! -f ".github/workflows/release.yml" ]; then
  echo "⚙️ Initializing cargo-dist workflow for GitHub Actions..."
  cargo install cargo-dist --locked 2>/dev/null || true
  cargo dist init --yes --no-auto-releases
fi

# 3. Push code & manage git tags
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

echo "✅ Code and tag pushed successfully!"
echo "☁️ GitHub Actions is now handling the multi-platform build and release workflow automatically."