#!/usr/bin/env bash
# Build TDLib from source and install to /usr/local.
# Requires: cmake, gperf, zlib, openssl (or gnutls)
set -euo pipefail

TD_VERSION="1.8.0"
BUILD_DIR="${BUILD_DIR:-/tmp/tdlib-build}"
PREFIX="${PREFIX:-/usr/local}"

echo "==> Building TDLib ${TD_VERSION}"
echo "    Build dir: ${BUILD_DIR}"
echo "    Install to: ${PREFIX}"

sudo_needed=""
if [ ! -w "${PREFIX}" ]; then
    sudo_needed="sudo"
fi

# Install build deps (Arch / Debian / macOS)
install_deps() {
    if command -v pacman &>/dev/null; then
        sudo pacman -S --needed --noconfirm cmake gperf zlib openssl
    elif command -v apt &>/dev/null; then
        sudo apt update
        sudo apt install -y cmake gperf zlib1g-dev libssl-dev
    elif command -v brew &>/dev/null; then
        brew install cmake gperf openssl
    else
        echo "⚠️  Please install cmake, gperf, zlib, openssl manually."
    fi
}

echo "==> Checking build dependencies…"
install_deps

mkdir -p "${BUILD_DIR}"
cd "${BUILD_DIR}"

if [ ! -d td ]; then
    echo "==> Cloning TDLib…"
    git clone --depth 1 --branch "v${TD_VERSION}" https://github.com/tdlib/td.git
fi

cd td
mkdir -p build
cd build

echo "==> Configuring…"
cmake .. \
    -DCMAKE_BUILD_TYPE=Release \
    -DCMAKE_INSTALL_PREFIX="${PREFIX}" \
    -DTD_ENABLE_JNI=OFF

echo "==> Compiling (this takes a few minutes)…"
cmake --build . --parallel "$(nproc 2>/dev/null || sysctl -n hw.ncpu)"

echo "==> Installing to ${PREFIX}…"
${sudo_needed} cmake --install .

echo "==> Done!"
echo "    libtdjson: ${PREFIX}/lib/libtdjson.so"
echo "    (or libtdjson.dylib on macOS)"
echo ""
echo "Make sure ${PREFIX}/lib is in your LD_LIBRARY_PATH / DYLD_LIBRARY_PATH."
echo "  export LD_LIBRARY_PATH=${PREFIX}/lib:\$LD_LIBRARY_PATH"
