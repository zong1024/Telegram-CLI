# Telegram-CLI

**TDLib + Rust daemon + TUI/CLI** — a full-featured Telegram client for your terminal.

```
CLI / TUI 前端
       │
       │ Unix Socket / JSON-RPC
       ▼
Rust Core Daemon  (tg-daemon)
       │
       │ FFI / tdjson
       ▼
TDLib / libtdjson
       │
       ▼
Telegram Network + Local TDLib Database
```

## Architecture

| Component | Binary | Description |
|-----------|--------|-------------|
| **tg-daemon** | `tg-daemon` | Background daemon — owns the TDLib connection, stores session, accepts clients via Unix socket |
| **tg-cli** | `tg` | Command-line client — send one-shot commands to the daemon |
| **tg-tui** | `tg-tui` | Full TUI chat client — ratatui-powered terminal interface |
| **tg-common** | (lib) | Shared protocol, config, and error types |

## Quick Start

### Prerequisites

1. **Telegram API credentials** — go to https://my.telegram.org/apps and create an app to get `api_id` and `api_hash`.
2. **TDLib** — either install system-wide or let the build script compile it.

### Install

```bash
# Clone
git clone https://github.com/zong1024/Telegram-CLI.git
cd Telegram-CLI

# Option A: install system TDLib
sudo pacman -S tdlib        # Arch
sudo apt install libtd-dev  # Debian/Ubuntu
brew install tdlib           # macOS

# Option B: build TDLib from source
./scripts/build-tdlib.sh

# Build all Rust binaries
cargo build --release

# Binaries are now in target/release/
ls target/release/tg target/release/tg-daemon target/release/tg-tui
```

### Configure

```bash
# Interactive setup wizard
./target/release/tg init
# This creates ~/.config/tg-cli/config.toml
```

Or create `~/.config/tg-cli/config.toml` manually:

```toml
api_id      = 12345678
api_hash    = "your_api_hash_here"
phone       = "+8613800138000"   # optional, prompted at login
socket_path = "/run/user/1000/tg-cli.sock"
database_dir = "~/.local/share/tg-cli"
verbosity   = 0
test        = false
```

### Run

```bash
# 1. Start daemon (background)
tg-daemon &

# 2. Login (interactive: phone → code → 2FA)
tg login

# 3. Use CLI commands
tg ls                     # list chats
tg messages 123456789     # show messages in a chat
tg send 123456789 "Hello" # send a message
tg search 123456789 "key" # search messages
tg read 123456789         # mark as read
tg status                 # daemon status

# 4. Or launch the TUI
tg-tui
```

## TUI Keybindings

| Key | Action |
|-----|--------|
| `j` / `↓` | Next dialog |
| `k` / `↑` | Previous dialog |
| `Enter` / `i` | Focus input |
| `Esc` | Back to dialog list |
| `Enter` (in input) | Send message |
| `/search <query>` | Search in current chat |
| `/read` | Mark current chat as read |
| `/q` | Quit |
| `q` | Quit |

## IPC Protocol

Clients and daemon communicate over a Unix socket with **newline-delimited JSON**:

```
→  {"id":1, "method":"send_message", "params":{"chat_id":123, "text":"Hello"}}
←  {"type":"response", "id":1, "result":{"id":456, ...}}
←  {"type":"event", "name":"new_message", "data":{...}}
←  {"type":"auth_state", "state":"ready"}
```

## Project Structure

```
Telegram-CLI/
├── Cargo.toml              # workspace root
├── crates/
│   ├── common/             # shared types, config, protocol
│   │   └── src/
│   │       ├── lib.rs
│   │       ├── config.rs   # TgConfig, XDG paths
│   │       ├── error.rs    # TgError
│   │       └── protocol.rs # JSON-RPC wire format
│   ├── daemon/             # background daemon
│   │   └── src/
│   │       ├── main.rs     # entry, CLI args
│   │       ├── tdlib_client.rs  # TDLib wrapper + receive loop
│   │       ├── auth.rs     # auth state machine
│   │       ├── handler.rs  # request → TDLib dispatch
│   │       ├── server.rs   # Unix socket accept loop
│   │       └── dispatcher.rs
│   ├── cli/                # one-shot CLI
│   │   └── src/
│   │       ├── main.rs     # clap commands → socket
│   │       ├── init.rs     # config wizard
│   │       ├── login.rs    # interactive login
│   │       └── output.rs   # pretty-print responses
│   └── tui/                # ratatui TUI
│       └── src/
│           └── main.rs     # full TUI app
└── scripts/
    └── build-tdlib.sh      # build TDLib from source
```

## License

MIT
