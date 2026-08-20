#!/bin/bash

# Exit immediately if any command fails
set -e

# 1. Extract full GitHub repository URL from package.json or git remote
REPO_URL=""

if [ -f "package.json" ]; then
  echo "🔍 Parsing repository URL from package.json..."
  REPO_URL=$(grep -m 1 '"repository"' package.json | grep -oE 'https://github.com/[a-zA-Z0-9_.-]+/[a-zA-Z0-9_.-]+|git@[a-zA-Z0-9_.-]+:[a-zA-Z0-9_.-]+/[a-zA-Z0-9_.-]+' || true)
fi

# Fallback to git remote if package.json didn't provide a valid URL
if [ -z "$REPO_URL" ]; then
  echo "🔍 Parsing repository URL from git remote..."
  REPO_URL=$(git config --get remote.origin.url || true)
fi

if [ -z "$REPO_URL" ]; then
    echo "❌ Could not automatically determine the repository URL from package.json or git remote."
    exit 1
fi

# Clean up trailing .git if present
REPO_URL=$(echo "$REPO_URL" | sed 's/\.git$//')

# Convert SSH URLs (git@github.com:user/repo) to HTTPS format if needed
if [[ "$REPO_URL" == git@* ]]; then
  REPO_URL=$(echo "$REPO_URL" | sed 's/:/\//' | sed 's/git@/https:\/\//')
fi

# Extract components for Homebrew/Formula use
CLEAN_PATH=$(echo "$REPO_URL" | sed -E 's/https:\/\/github.com\///')
GITHUB_USER=$(echo "$CLEAN_PATH" | cut -d '/' -f 1)
GITHUB_REPO=$(echo "$CLEAN_PATH" | cut -d '/' -f 2)
FORMULA_NAME="$GITHUB_REPO"
TAP_NAME="homebrew-tap"

echo "🎯 Target Repository URL: $REPO_URL"

# 2. Homebrew setup and installation script execution
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