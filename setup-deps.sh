#!/bin/bash
# Install system dependencies required to build & run the Tauri file manager on Ubuntu/Debian
set -e

echo "==> Installing Tauri v2 system dependencies for Ubuntu/Debian"
sudo apt update
sudo apt install -y \
    libwebkit2gtk-4.1-dev \
    build-essential \
    curl \
    wget \
    file \
    libxdo-dev \
    libssl-dev \
    libayatana-appindicator3-dev \
    librsvg2-dev \
    libglib2.0-dev

echo "==> System dependencies installed."
echo "==> Next steps:"
echo "      cargo build            # build debug"
echo "      cargo tauri dev        # run in dev mode"
echo "      cargo tauri build --bundles deb,appimage,rpm  # release"