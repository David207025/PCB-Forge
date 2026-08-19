#!/bin/bash

# Exit immediately if a command fails
set -e

# Check if a tag argument was provided
if [ -z "$1" ]; then
  echo "❌ Error: No tag provided."
  echo "Usage: ./wipe-tag.sh <tag-name>"
  echo "Example: ./wipe-tag.sh v1.0.0"
  exit 1
fi

TAG="$1"
echo "🔍 Targeting tag: $TAG"

# 1. Delete the tag locally if it exists
if git rev-parse --verify "refs/tags/$TAG" >/dev/null 2>&1; then
  echo "🗑️  Deleting local tag $TAG..."
  git tag -d "$TAG"
else
  echo "ℹ️  Local tag $TAG does not exist."
fi

# 2. Delete the tag from the remote repository (GitHub)
echo "☁️  Deleting remote tag $TAG from origin..."
git push origin --delete "$TAG" 2>/dev/null || echo "ℹ️  Remote tag $TAG did not exist on GitHub."

# 3. Clean up any leftover packed refs or cached remote references just in case
git gc --prune=now --quiet 2>/dev/null || true

echo "✅ Success! Tag $TAG has been fully erased locally and remotely."