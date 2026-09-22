#!/bin/bash
# Packages the release binary into a portable .tar.gz (no installers required).
# Usage: ./package-tar.sh   ->  dist/MFileManager-<version>-linux-amd64.tar.gz
set -euo pipefail

cd "$(dirname "$0")"
VERSION="$(grep -m1 '^version' Cargo.toml | cut -d'"' -f2)"
TAG="MFileManager-${VERSION}-linux-amd64"
DIST="dist"

echo "==> Building release binary..."
cargo build --release

echo "==> Staging package..."
rm -rf "${DIST}/${TAG}"
mkdir -p "${DIST}/${TAG}"

cp target/release/file-manager "${DIST}/${TAG}/"
cp README.md LICENSE "${DIST}/${TAG}/" 2>/dev/null || true
cp icons/128x128.png "${DIST}/${TAG}/file-manager.png"

cat > "${DIST}/${TAG}/file-manager.desktop" <<EOF
[Desktop Entry]
Name=File Manager
Comment=A fast, modern file manager for Ubuntu/Linux
Exec=$(pwd)/${DIST}/${TAG}/file-manager
Icon=$(pwd)/${DIST}/${TAG}/file-manager.png
Terminal=false
Type=Application
Categories=Utility;FileManager;System;
EOF
chmod +x "${DIST}/${TAG}/file-manager"

echo "==> Creating ${DIST}/${TAG}.tar.gz..."
tar -czf "${DIST}/${TAG}.tar.gz" -C "${DIST}" "${TAG}"
ls -lh "${DIST}/${TAG}.tar.gz"

echo "Done. Extract & run:"
echo "  tar -xzf ${DIST}/${TAG}.tar.gz"
echo "  cd ${TAG} && ./file-manager"
echo "To add to GNOME launcher, install the .desktop file with:"
echo "  cp ${TAG}/file-manager.desktop ~/.local/share/applications/  (then fix the Exec= path)"