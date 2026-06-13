#!/usr/bin/env bash
# Install tg and tgcd, set up systemd user service.
set -euo pipefail

PREFIX="${PREFIX:-$HOME/.cargo/bin}"
SERVICE_DIR="${XDG_CONFIG_HOME:-$HOME/.config}/systemd/user"

echo "==> Building release binaries…"
cargo build --release

echo "==> Installing binaries to ${PREFIX}…"
mkdir -p "${PREFIX}"
cp -f target/release/tg   "${PREFIX}/tg"
cp -f target/release/tgcd "${PREFIX}/tgcd"
chmod +x "${PREFIX}/tg" "${PREFIX}/tgcd"

echo "==> Installing systemd user service…"
mkdir -p "${SERVICE_DIR}"
cp -f scripts/tgcd.service "${SERVICE_DIR}/tgcd.service"

systemctl --user daemon-reload
systemctl --user enable tgcd.service

echo ""
echo "✅  Installation complete!"
echo ""
echo "  Binaries: ${PREFIX}/tg, ${PREFIX}/tgcd"
echo "  Service:  ${SERVICE_DIR}/tgcd.service"
echo ""
echo "Next steps:"
echo "  1. Run 'tg init' to create config (set api_id + api_hash)"
echo "  2. Start daemon:  systemctl --user start tgcd"
echo "     Or manually:   tgcd &"
echo "  3. Login:         tg login"
echo "  4. Use:           tg chats / tg send <chat> 'hello' / tg tui"
