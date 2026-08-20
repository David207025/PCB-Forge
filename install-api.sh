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

# 2. Extract GitHub User / Repository name from package.json
if [ -f "package.json" ]; then
  echo "🔍 Parsing repository info from package.json..."
  # Tries to extract user/repo from package.json repository field
  REPO_STRING=$(grep -m 1 '"repository"' package.json | grep -oE '[a-zA-Z0-9_-]+/[a-zA-Z0-9_-]+' || true)

  if [ -n "$REPO_STRING" ]; then
    GITHUB_USER=$(echo "$REPO_STRING" | cut -d '/' -f 1)
    GITHUB_REPO=$(echo "$REPO_STRING" | cut -d '/' -f 2)
  fi
fi

# Fallback to git remote if package.json didn't have it
if [ -z "$GITHUB_USER" ] || [ -z "$GITHUB_REPO" ]; then
  echo "🔍 Parsing repository info from git remote..."
  GIT_URL=$(git config --get remote.origin.url || true)
  if [ -n "$GIT_URL" ]; then
    # Handles both HTTPS and SSH git URLs
    CLEAN_URL=$(echo "$GIT_URL" | sed -E 's/^(git@github.com:|https:\/\/github.com\/)//' | sed 's/\.git$//')
    GITHUB_USER=$(echo "$CLEAN_URL" | cut -d '/' -f 1)
    GITHUB_REPO=$(echo "$CLEAN_URL" | cut -d '/' -f 2)
  fi
fi

if [ -z "$GITHUB_USER" ] || [ -z "$GITHUB_REPO" ]; then
  echo "❌ Could not automatically determine GitHub user and repo. Please check package.json or git remote."
  exit 1
fi

TAP_NAME="homebrew-tap"
FORMULA_NAME="$GITHUB_REPO"

echo "🎯 Target GitHub Repository: $GITHUB_USER/$GITHUB_REPO"

# 3. Ensure cargo-dist is initialized for GitHub Actions (safe to run multiple times)
if [ ! -f ".github/workflows/release.yml" ]; then
  echo "⚙️ Initializing cargo-dist workflow for GitHub Actions..."
  cargo install cargo-dist --locked 2>/dev/null || true
  cargo dist init --yes --no-auto-releases
fi

# 4. Push code & manage git tags
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

# 5. Homebrew setup and installation script generation
echo -e "\n🚀 Checking for Homebrew..."

# Install Homebrew if it isn't already installed on macOS/Linux
if ! command -v brew &> /dev/null; then
    echo "📦 Homebrew not found. Installing Homebrew..."
    /bin/bash -c "$(curl -fsSL https://raw.githubusercontent.com/Homebrew/install/HEAD/install.sh)"

    # Configure Homebrew path for Apple Silicon or Linux if necessary
    if [[ "$OSTYPE" == "darwin"* ]] && [[ $(uname -m) == "arm64" ]]; then
        echo 'eval "$(/opt/homebrew/bin/brew shellenv)"' >> ~/.zshrc
        eval "$(/opt/homebrew/bin/brew shellenv)"
    elif [[ -d /home/linuxbrew/.linuxbrew ]]; then
        echo 'eval "$(/home/linuxbrew/.linuxbrew/bin/brew shellenv)"' >> ~/.bashrc
        eval "$(/home/linuxbrew/.linuxbrew/bin/brew shellenv)"
    fi
else
    echo "✅ Homebrew is already installed."
fi

echo -e "\n📡 Tapping custom repository: ${GITHUB_USER}/${TAP_NAME}..."
brew tap "${GITHUB_USER}/${TAP_NAME}"

echo -e "\n📥 Pulling and installing ${FORMULA_NAME} from GitHub..."
# If it's already installed, upgrade it; otherwise, install it fresh
if brew list --formula | grep -q "^${FORMULA_NAME}$"; then
    brew upgrade "${FORMULA_NAME}"
else
    brew install "${GITHUB_USER}/${TAP_NAME}/${FORMULA_NAME}"
fi

echo -e "\n✨ SUCCESS: ${FORMULA_NAME} is successfully installed and ready to use!"