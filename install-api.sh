#!/bin/bash
set -e

echo "📦 Preparing pcbfapi installation..."

# Stop any running service instances
brew services stop pcbfapi 2>/dev/null || true
if command -v pcbfapi &>/dev/null; then
  pcbfapi service stop 2>/dev/null || true
fi

# Uninstall existing formula
brew uninstall pcbfapi 2>/dev/null || true

echo "🍺 Installing pcbfapi from Homebrew tap..."
brew install david207025/pcb-forge/pcbfapi

echo "🚀 Starting pcbfapi background service daemon..."
brew services start pcbfapi 2>/dev/null || pcbfapi service install

echo "🔑 Checking KiCad application permissions and access..."
pcbfapi check-access || true

echo ""
echo "✅ pcbfapi is installed and running as a persistent background daemon service!"
echo "   Endpoint: http://127.0.0.1:47210"
echo "   Service commands: pcbfapi service [status|start|stop|restart]"
