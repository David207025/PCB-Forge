#!/bin/bash
# ==============================================================================
# retag.sh — Delete existing release + tag and recreate to re-trigger CI
# ==============================================================================
#
# This script is intended for use when you need to re-trigger the cargo-dist
# GitHub Actions release workflow without changing the version number.
#
# It:
#   1. Extracts the current version from pcbfapi/Cargo.toml
#   2. Commits and pushes any uncommitted changes
#   3. Deletes the existing GitHub Release (via `gh`) so cargo-dist can
#      recreate it cleanly
#   4. Deletes the old git tag locally and on GitHub
#   5. Creates a fresh tag and pushes it (triggering release.yml)
#
# Prerequisites:
#   - gh CLI installed and authenticated (`gh auth login`)
#   - cargo-dist release workflow at .github/workflows/release.yml
#
# Usage:
#   ./retag.sh

set -e  # Exit immediately on any command failure

# ── Step 1: Extract version from Cargo.toml ────────────────────────────────────
VERSION=$(grep -m 1 '^version = ' pcbfapi/Cargo.toml | cut -d '"' -f 2)

if [ -z "$VERSION" ]; then
  echo "❌ Could not find version in pcbfapi/Cargo.toml"
  exit 1
fi

TAG="v$VERSION"
echo "📦 Found version: $VERSION"
echo "🏷️  Managing tag: $TAG"

# ── Step 2: Ensure cargo-dist workflow exists ──────────────────────────────────
if [ ! -f ".github/workflows/release.yml" ]; then
  echo "⚙️ Initializing cargo-dist workflow for GitHub Actions..."
  cargo install cargo-dist --locked 2>/dev/null || true
  dist init --yes --no-auto-releases
fi

# ── Step 3: Commit and push any pending changes ────────────────────────────────
echo "📤 Pushing latest code changes to GitHub..."
git add .
if ! git diff-index --quiet HEAD --; then
  git commit -m "chore: release $TAG"
fi
git push origin main

# ── Step 4: Delete the existing GitHub Release (if any) ───────────────────────
# This must happen before deleting the tag; otherwise vsce / cargo-dist may
# fail trying to recreate a release that already exists for that tag.
if gh release view "$TAG" &>/dev/null; then
  echo "🗑️  Deleting existing GitHub Release $TAG..."
  gh release delete "$TAG" --yes
else
  echo "ℹ️  No existing GitHub Release for $TAG — nothing to delete."
fi

# ── Step 5: Delete old tag locally and remotely ────────────────────────────────
git tag -d "$TAG" 2>/dev/null || true
git push --delete origin "$TAG" 2>/dev/null || true

# ── Step 6: Create fresh tag and push to trigger CI ───────────────────────────
git tag "$TAG"
git push origin "$TAG"

echo "✅ Code and tag pushed successfully!"
echo "☁️  GitHub Actions is now handling the multi-platform build and release workflow automatically."