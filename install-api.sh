#!/bin/bash

# Configuration - Change these to your actual GitHub username and repository name
GITHUB_USER="David207025"
TAP_NAME="homebrew-tap"
FORMULA_NAME="pcbfapi" # or whatever you name your formula

echo "🚀 Checking for Homebrew..."

# 1. Install Homebrew if it isn't already installed on macOS/Linux
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