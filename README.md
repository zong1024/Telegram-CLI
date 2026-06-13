# Telegram-CLI

**TDLib + Rust daemon + TUI/CLI** — a full-featured Telegram client for your terminal.

```
tg / tg-tui  (前端)
      │
      │ Unix Socket, LengthDelimitedCodec
      ▼
    tgcd     (Rust daemon, tokio)
      │
      │ 自写 tdjson FFI (libtdjson.so)
      ▼
  TDLib C    (libtdjson)
      │
      ▼
Telegram Cloud + 本地 TDLib DB + SQLite Cache
```

## Architecture

| Component | Binary | Description |
|-----------|--------|-------------|
| **tgcd** | `tgcd` | Background daemon — owns the TDLib connection, SQLite cache, accepts clients via Unix socket |
| **tg** | `tg` | Command-line client — send one-shot commands to `tgcd` |
| **tg-tui** | `tg-tui` | Full TUI chat client — ratatui-powered terminal interface |
| **tg-common** | (lib) | Shared protocol, config, IPC client |
| **tg-tdjson** | (lib) | Self-written FFI wrapper around `libtdjson.so` |

### Tech Stack

| Layer | Technology |
|-------|-----------|
| Core | TDLib / libtdjson |
| Rust runtime | tokio |
| Serialization | serde / serde_json |
| Error handling | thiserror / anyhow |
| Logging | tracing |
| CLI | clap |
| TUI | ratatui + crossterm |
| Local cache | SQLite + sqlx |
| IPC | Unix Socket + LengthDelimitedCodec + JSON |
| Config | directories + toml + keyring |
| Distribution | tg + tgcd + libtdjson, systemd user service |

## Quick Start

### Prerequisites

1. **Telegram API credentials** — https://my.telegram.org/apps → get `api_id` and `api_hash`
2. **TDLib** — install `libtdjson.so`:

```bash
# Arch
sudo pacman -S tdlib

# Ubuntu/Debian
sudo apt install libtd-dev

# macOS
brew install tdlib

# Or build from source
./scripts/build-tdlib.sh
```

### Install

```bash
git clone https://github.com/zong1024/Telegram-CLI.git
cd Telegram-CLI

# One-click install (builds + installs binaries + systemd service)
./scripts/install.sh

# Or manual build
cargo build --release
```

### Configure

```bash
# Interactive wizard (creates ~/.config/tg/config.toml)
tg init
```

Or create `~/.config/tg/config.toml` manually:

```toml
api_id      = 12345678
api_hash    = "your_api_hash_here"
phone       = "+8613800138000"   # optional
socket_path = "/run/user/1000/tg/tgcd.sock"
database_path = "/home/user/.local/share/tg/tg.db"
tdlib_dir   = "/home/user/.local/share/tg/tdlib/"
verbosity   = 0
test        = false
```

Credentials are stored in your system keyring (GNOME Keyring / KWallet / macOS Keychain) and take priority over the config file.

### Run

```bash
# Option A: systemd (recommended)
systemctl --user start tgcd
tg login        # interactive: phone → code → 2FA

# Option B: manual
tgcd &
tg login

# Use
tg ls                       # list chats
tg messages 123456789       # show messages
tg send 123456789 "Hello"   # send a message
tg search 123456789 "key"   # search (hits SQLite cache first)
tg read 123456789           # mark as read
tg status                   # daemon status
tg-tui                      # launch TUI
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

Length-delimited JSON over Unix socket (4-byte big-endian length prefix + JSON payload).

```
Client → Daemon:
  ┌──────────┬──────────────────────────────────────────┐
  │ len (4B) │ {"id":1,"method":"send_message",...}     │
  └──────────┴──────────────────────────────────────────┘

Daemon → Client:
  ┌──────────┬──────────────────────────────────────────┐
  │ len (4B) │ {"type":"response","id":1,"result":{…}} │
  └──────────┴──────────────────────────────────────────┘
  ┌──────────┬──────────────────────────────────────────┐
  │ len (4B) │ {"type":"event","name":"new_message",…}  │
  └──────────┴──────────────────────────────────────────┘
```

## Project Structure

```
Telegram-CLI/
├── Cargo.toml                  # workspace root
├── crates/
│   ├── tdjson/                 # self-written libtdjson FFI
│   │   ├── build.rs            # link script (pkg-config / env / fallback)
│   │   └── src/lib.rs          # TdJson, SharedTdJson (send/receive/execute)
│   ├── common/                 # shared types
│   │   └── src/
│   │       ├── config.rs       # TgConfig, keyring, directories
│   │       ├── error.rs        # TgError
│   │       ├── ipc.rs          # IpcClient (LengthDelimitedCodec)
│   │       └── protocol.rs     # Request, Response, Event, methods
│   ├── daemon/                 # tgcd binary
│   │   └── src/
│   │       ├── main.rs         # entry, build AppState, run IPC server
│   │       ├── tdlib.rs        # TdClient (SharedTdJson + receive loop)
│   │       ├── handler.rs      # method dispatch → raw TDLib JSON
│   │       ├── ipc.rs          # LengthDelimitedCodec server
│   │       ├── cache.rs        # SQLite (messages + dialogs tables)
│   │       ├── auth.rs         # auth state machine
│   │       └── dispatcher.rs   # event → cache updater
│   ├── cli/                    # tg binary
│   │   └── src/
│   │       ├── main.rs         # clap commands → IpcClient
│   │       ├── init.rs         # config wizard
│   │       ├── login.rs        # interactive login flow
│   │       └── output.rs       # pretty-print responses
│   └── tui/                    # tg-tui binary
│       └── src/main.rs         # ratatui TUI
└── scripts/
    ├── build-tdlib.sh          # build TDLib from source
    ├── tgcd.service            # systemd user service
    └── install.sh              # one-click install
```

## License

MIT
