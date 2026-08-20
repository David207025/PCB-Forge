#!/bin/bash

# Exit immediately if any command fails
set -e

# 1. Extract the exact repository URL from package.json using Node.js for clean JSON parsing
REPO_URL=""

if [ -f "package.json" ]; then
  echo "🔍 Reading repository URL from package.json..."
  REPO_URL=$(node -e '
    try {
      const pkg = require("./package.json");
      let repo = pkg.repository;
      if (typeof repo === "object" && repo !== null) repo = repo.url;
      if (typeof repo === "string") console.log(repo.trim());
    } catch (e) {}
  ' || true)
fi

if [ -z "$REPO_URL" ]; then
  echo "🔍 Reading repository URL from git remote..."
  REPO_URL=$(git config --get remote.origin.url || true)
fi

if [ -z "$REPO_URL" ]; then
  echo "❌ Could not find repository URL in package.json or git remote."
  exit 1
fi

# Strip git+ prefix if present
REPO_URL=$(echo "$REPO_URL" | sed 's/^git+//')

# Clean up trailing .git if present
REPO_URL=$(echo "$REPO_URL" | sed 's/\.git$//')

# Convert SSH URLs (git@github.com:user/repo) to HTTPS format
if [[ "$REPO_URL" == git@* ]]; then
  REPO_URL=$(echo "$REPO_URL" | sed 's/:/\//' | sed 's/git@/https:\/\//')
fi

# Extract user/repo components safely
CLEAN_PATH=$(echo "$REPO_URL" | sed -E 's/https:\/\/github.com\///')
GITHUB_USER=$(echo "$CLEAN_PATH" | cut -d '/' -f 1)
GITHUB_REPO=$(echo "$CLEAN_PATH" | cut -d '/' -f 2)

# Homebrew requires formula names to be lowercase
FORMULA_NAME=$(echo "$GITHUB_REPO" | tr '[:upper:]' '[:lower:]')
TAP_NAME="homebrew-tap"

# Extract version from Cargo.toml to point to the specific release tag
VERSION=$(grep -m 1 '^version = ' pcbfapi/Cargo.toml | cut -d '"' -f 2)
TAG="v$VERSION"

echo "🎯 Repository: $REPO_URL"
echo "📦 Version Tag: $TAG"
echo "🏷️  Formula Name: $FORMULA_NAME"

# 2. Ensure Homebrew is available
if ! command -v brew &> /dev/null; then
    echo "📦 Homebrew not found. Installing Homebrew..."
    /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

    if [[ "$OSTYPE" == "darwin"* ]] && [[ $(uname -m) == "arm64" ]]; then
        eval "$(/opt/homebrew/bin/brew shellenv)"
    elif [[ -d /home/linuxbrew/.linuxbrew ]]; then
        eval "$(/home/linuxbrew/.linuxbrew/bin/brew shellenv)"
    fi
fi

# 3. Tap using the custom URL format and install the formula
echo "📡 Tapping repository via custom URL..."
brew tap "${GITHUB_USER}/${TAP_NAME}" "$REPO_URL"

echo "📥 Installing formula..."
if brew list --formula | grep -q "^${FORMULA_NAME}$"; then
    brew upgrade "${FORMULA_NAME}"
else
    brew install "${GITHUB_USER}/${TAP_NAME}/${FORMULA_NAME}"
fi

echo "✨ SUCCESS: Installed successfully!"