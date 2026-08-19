#!/bin/bash

# Exit immediately if any command fails
set -e

# 1. Extract the version from pcbapi/Cargo.toml
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

echo "🚀 Pushing new tag to GitHub..."
git push origin "$TAG"

echo "✅ Done! All code pushed and release tag $TAG created. Check the Actions tab in GitHub!"