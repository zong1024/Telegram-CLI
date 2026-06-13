//! `tg-tui` — Terminal UI for Telegram, powered by ratatui.
//!
//! Connects to `tgcd` via IPC and provides a vim-like chat interface.

use std::io;
use std::time::{Duration, Instant};

use anyhow::Result;
use crossterm::{
    event::{self, Event, KeyCode, KeyEvent, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{
    backend::CrosstermBackend,
    layout::{Constraint, Direction, Layout},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, List, ListItem, Paragraph, Wrap},
    Frame, Terminal,
};
use tokio::sync::mpsc;

use tg_common::config::TgConfig;
use tg_common::ipc::IpcClient;
use tg_common::protocol::{methods, ServerMessage};

// ── Application state ──────────────────────────────────────────────

#[derive(PartialEq)]
enum Focus {
    Dialogs,
    Input,
}

struct App {
    dialogs: Vec<DialogItem>,
    selected: usize,
    messages: Vec<ChatMessage>,
    input: String,
    input_cursor: usize,
    focus: Focus,
    status: String,
    running: bool,
}

struct DialogItem {
    id: i64,
    title: String,
    unread: i32,
}

struct ChatMessage {
    id: i64,
    sender: String,
    text: String,
    time: String,
}

impl App {
    fn new() -> Self {
        Self {
            dialogs: Vec::new(),
            selected: 0,
            messages: Vec::new(),
            input: String::new(),
            input_cursor: 0,
            focus: Focus::Dialogs,
            status: "Starting…".into(),
            running: true,
        }
    }

    fn current_chat_id(&self) -> Option<i64> {
        self.dialogs.get(self.selected).map(|d| d.id)
    }
}

// ── Entry point ────────────────────────────────────────────────────

#[tokio::main]
async fn main() -> Result<()> {
    let config = TgConfig::load()?;
    let socket = &config.socket_path;

    if !socket.exists() {
        eprintln!("❌  Daemon not running. Start it first: tgcd");
        std::process::exit(1);
    }

    // Terminal setup
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Connect to daemon
    let mut client = IpcClient::connect(socket).await?;

    // Channel for server messages → UI
    let (ev_tx, mut ev_rx) = mpsc::channel::<ServerMessage>(64);

    // Spawn background reader
    tokio::spawn(async move {
        loop {
            match client.read_message().await {
                Ok(msg) => {
                    if ev_tx.send(msg).await.is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
    });

    let mut app = App::new();

    // Request initial dialogs
    // (The background reader will pick up the response)
    let config2 = TgConfig::load()?;
    let mut writer_client = IpcClient::connect(&config2.socket_path).await?;
    writer_client
        .send_request(&tg_common::protocol::Request {
            id: 1,
            method: methods::LIST_DIALOGS.to_string(),
            params: serde_json::json!({ "limit": 50 }),
        })
        .await?;
    app.status = "Loading dialogs…".into();

    let tick_rate = Duration::from_millis(100);
    let mut last_tick = Instant::now();

    // ── Main loop ──────────────────────────────────────────────

    loop {
        terminal.draw(|f| ui(f, &app))?;

        let timeout = tick_rate
            .checked_sub(last_tick.elapsed())
            .unwrap_or(Duration::ZERO);

        if event::poll(timeout)? {
            if let Event::Key(key) = event::read()? {
                handle_key(key, &mut app, &mut writer_client).await?;
            }
        }

        if last_tick.elapsed() >= tick_rate {
            last_tick = Instant::now();
        }

        // Handle server messages
        while let Ok(msg) = ev_rx.try_recv() {
            handle_server_message(&msg, &mut app);
        }

        if !app.running {
            break;
        }
    }

    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

// ── Key handling ───────────────────────────────────────────────────

async fn handle_key(
    key: KeyEvent,
    app: &mut App,
    client: &mut IpcClient,
) -> Result<()> {
    match app.focus {
        Focus::Dialogs => match key.code {
            KeyCode::Char('q') | KeyCode::Esc => app.running = false,
            KeyCode::Char('i') | KeyCode::Enter => app.focus = Focus::Input,
            KeyCode::Char('j') | KeyCode::Down => {
                if app.selected < app.dialogs.len().saturating_sub(1) {
                    app.selected += 1;
                    load_messages(app, client).await?;
                }
            }
            KeyCode::Char('k') | KeyCode::Up => {
                if app.selected > 0 {
                    app.selected -= 1;
                    load_messages(app, client).await?;
                }
            }
            KeyCode::Char('/') => {
                app.focus = Focus::Input;
                app.input = "/".into();
                app.input_cursor = 1;
            }
            _ => {}
        },
        Focus::Input => match key.code {
            KeyCode::Esc => {
                app.input.clear();
                app.input_cursor = 0;
                app.focus = Focus::Dialogs;
            }
            KeyCode::Enter => {
                let text: String = app.input.drain(..).collect();
                app.input_cursor = 0;
                if !text.is_empty() {
                    if text.starts_with('/') {
                        handle_command(&text, app, client).await?;
                    } else if let Some(chat_id) = app.current_chat_id() {
                        client
                            .send_request(&tg_common::protocol::Request {
                                id: 0,
                                method: methods::SEND_MESSAGE.to_string(),
                                params: serde_json::json!({
                                    "chat_id": chat_id,
                                    "text": text
                                }),
                            })
                            .await?;
                    }
                }
            }
            KeyCode::Backspace => {
                if app.input_cursor > 0 {
                    app.input_cursor -= 1;
                    app.input.remove(app.input_cursor);
                }
            }
            KeyCode::Delete => {
                if app.input_cursor < app.input.len() {
                    app.input.remove(app.input_cursor);
                }
            }
            KeyCode::Left => app.input_cursor = app.input_cursor.saturating_sub(1),
            KeyCode::Right => {
                if app.input_cursor < app.input.len() {
                    app.input_cursor += 1;
                }
            }
            KeyCode::Char(c)
                if key.modifiers.is_empty() || key.modifiers == KeyModifiers::SHIFT =>
            {
                app.input.insert(app.input_cursor, c);
                app.input_cursor += 1;
            }
            _ => {}
        },
    }
    Ok(())
}

async fn handle_command(cmd: &str, app: &mut App, client: &mut IpcClient) -> Result<()> {
    let parts: Vec<&str> = cmd.splitn(2, ' ').collect();
    match parts[0] {
        "/q" => app.running = false,
        "/search" => {
            if let Some(chat_id) = app.current_chat_id() {
                let query = parts.get(1).unwrap_or(&"");
                client
                    .send_request(&tg_common::protocol::Request {
                        id: 0,
                        method: methods::SEARCH.to_string(),
                        params: serde_json::json!({ "chat_id": chat_id, "query": query }),
                    })
                    .await?;
                app.status = format!("Searching for: {query}");
            }
        }
        "/read" => {
            if let Some(chat_id) = app.current_chat_id() {
                client
                    .send_request(&tg_common::protocol::Request {
                        id: 0,
                        method: methods::MARK_READ.to_string(),
                        params: serde_json::json!({ "chat_id": chat_id }),
                    })
                    .await?;
                app.status = "Marked as read".into();
            }
        }
        _ => app.status = format!("Unknown command: {}", parts[0]),
    }
    Ok(())
}

async fn load_messages(app: &mut App, client: &mut IpcClient) -> Result<()> {
    if let Some(chat_id) = app.current_chat_id() {
        client
            .send_request(&tg_common::protocol::Request {
                id: 0,
                method: methods::GET_MESSAGES.to_string(),
                params: serde_json::json!({ "chat_id": chat_id, "limit": 50 }),
            })
            .await?;
        app.status = format!("Loading messages for chat {chat_id}…");
    }
    Ok(())
}

// ── Server message handling ────────────────────────────────────────

fn handle_server_message(msg: &ServerMessage, app: &mut App) {
    match msg {
        ServerMessage::Response(resp) => {
            if let Some(err) = &resp.error {
                app.status = format!("❌  {}", err.message);
                return;
            }
            if let Some(result) = &resp.result {
                // Detect response type by shape
                if let Some(dialogs) = result.as_array() {
                    app.dialogs.clear();
                    for (_i, item) in dialogs.iter().enumerate() {
                        let id = item["chat_id"].as_i64().unwrap_or(0);
                        let title = item["title"]
                            .as_str()
                            .unwrap_or(&format!("Chat #{id}"))
                            .to_string();
                        let unread = item["unread_count"].as_i64().unwrap_or(0) as i32;
                        app.dialogs.push(DialogItem { id, title, unread });
                    }
                    app.status = format!("{} dialogs loaded", app.dialogs.len());
                } else if let Some(messages) = result.as_array() {
                    app.messages.clear();
                    for m in messages.iter() {
                        let text = m["text"]
                            .as_str()
                            .unwrap_or("[media]");
                        let sender_id = m["sender_id"].as_i64().unwrap_or(0);
                        let ts = m["date"].as_i64().unwrap_or(0);
                        app.messages.push(ChatMessage {
                            id: m["id"].as_i64().unwrap_or(0),
                            sender: format!("user#{sender_id}"),
                            text: text.to_string(),
                            time: format_time(ts),
                        });
                    }
                }
            }
        }
        ServerMessage::Event(ev) => {
            if ev.name == "new_message" {
                app.status = "📨  New message".into();
            }
        }
        ServerMessage::AuthState(auth) => {
            app.status = format!("🔐  Auth: {}", auth.state);
        }
    }
}

// ── Rendering ──────────────────────────────────────────────────────

fn ui(f: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(1),
            Constraint::Length(3),
            Constraint::Length(1),
        ])
        .split(f.area());

    // Header
    let header = Paragraph::new(" 📱 Telegram TUI │ j/k: navigate │ i: input │ /q: quit ")
        .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD));
    f.render_widget(header, chunks[0]);

    // Body
    let body_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(30), Constraint::Percentage(70)])
        .split(chunks[1]);

    // Dialog list
    let dialog_style = if app.focus == Focus::Dialogs {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let dialogs: Vec<ListItem> = app
        .dialogs
        .iter()
        .enumerate()
        .map(|(i, d)| {
            let style = if i == app.selected {
                Style::default()
                    .fg(Color::Black)
                    .bg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
            };
            let prefix = if d.unread > 0 {
                format!("({}) ", d.unread)
            } else {
                String::new()
            };
            ListItem::new(Line::from(vec![Span::styled(
                format!("{prefix}{}", d.title),
                style,
            )]))
        })
        .collect();
    let dialog_list = List::new(dialogs).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Chats ")
            .border_style(dialog_style),
    );
    f.render_widget(dialog_list, body_chunks[0]);

    // Messages
    let messages: Vec<Line> = app
        .messages
        .iter()
        .map(|m| {
            Line::from(vec![
                Span::styled(
                    format!("[{}] ", m.time),
                    Style::default().fg(Color::DarkGray),
                ),
                Span::styled(
                    format!("{}: ", m.sender),
                    Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD),
                ),
                Span::raw(&m.text),
            ])
        })
        .collect();
    let chat = Paragraph::new(messages)
        .block(Block::default().borders(Borders::ALL).title(" Messages "))
        .wrap(Wrap { trim: false });
    f.render_widget(chat, body_chunks[1]);

    // Input
    let input_text = format!("{}█", app.input);
    let input_style = if app.focus == Focus::Input {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default()
    };
    let input = Paragraph::new(input_text).block(
        Block::default()
            .borders(Borders::ALL)
            .title(" Input (Enter: send, Esc: back) ")
            .border_style(input_style),
    );
    f.render_widget(input, chunks[2]);

    // Status bar
    let status = Paragraph::new(app.status.clone()).style(Style::default().fg(Color::DarkGray));
    f.render_widget(status, chunks[3]);
}

// ── Helpers ────────────────────────────────────────────────────────

fn format_time(ts: i64) -> String {
    use std::time::{Duration, UNIX_EPOCH};
    let dt = UNIX_EPOCH + Duration::from_secs(ts as u64);
    let datetime: chrono::DateTime<chrono::Utc> = dt.into();
    datetime.format("%H:%M").to_string()
}
