#!/bin/bash
# ==============================================================================
# release.sh — Unified release automation script for PCB Forge
# ==============================================================================
#
# Automates the complete end-to-end release process:
#   1. (Optional) Syncs version across all package manifests (update-version.js)
#   2. Regenerates Markdown API docs in docs/ (generate-docs.js)
#   3. Builds web-ui and compiles the VS Code extension
#   4. Packages the VSIX extension bundle (package-extension.js)
#   5. Publishes VSIX to VS Code Marketplace (if VSCE_PAT is available)
#   6. Commits and pushes changes to main branch
#   7. Deletes any pre-existing release/tag for this version on GitHub
#   8. Creates a fresh git tag and pushes it to trigger cargo-dist CI
#
# Usage:
#   ./release.sh           # Uses current version in pcbfapi/Cargo.toml
#   ./release.sh 0.4.0     # Bumps to 0.4.0 first, then runs release
# ==============================================================================

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
cd "$SCRIPT_DIR"

NEW_VERSION="$1"

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
  echo "⬆️ Pushing changes to origin..."
  git push origin main
else
  echo "✅ Working tree is clean."
fi

# Step 7: Delete pre-existing GitHub release and tag if any
if command -v gh &> /dev/null; then
  echo "🔍 Checking for existing GitHub Release for $TAG..."
  if gh release view "$TAG" &> /dev/null; then
    echo "🗑️  Deleting existing GitHub Release $TAG..."
    gh release delete "$TAG" --yes --cleanup-tag
    echo "✅ Existing release deleted."
  fi
fi

if git rev-parse "$TAG" >/dev/null 2>&1; then
  echo "🗑️  Deleting local tag $TAG..."
  git tag -d "$TAG"
fi

if git ls-remote --tags origin | grep -q "refs/tags/$TAG"; then
  echo "🗑️  Deleting remote tag $TAG..."
  git push origin ":refs/tags/$TAG"
fi

# Step 8: Create and push fresh tag
echo "🏷️  Creating fresh tag $TAG..."
git tag -a "$TAG" -m "Release $TAG"
echo "⬆️ Pushing tag $TAG to origin (triggers CI cargo-dist workflow)..."
git push origin "$TAG"

echo ""
echo "🎉 Release process complete for $TAG!"
echo "   GitHub Actions will now build binaries, installers, and update the Homebrew tap."
