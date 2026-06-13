//! `tg login` — interactive login via TDLib through the daemon.

use anyhow::Result;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

use tg_common::config::TgConfig;

pub async fn run() -> Result<()> {
    let config = TgConfig::load()?;
    let socket = &config.socket_path;

    if !socket.exists() {
        anyhow::bail!(
            "Daemon not running. Start it first: tg-daemon\n\
             (Ensure TG_PHONE is set in config or env for auto-login.)"
        );
    }

    println!("🔐  Logging in…\n");
    println!("The daemon handles authentication via TDLib.");
    println!("If a session already exists, you're already logged in.");
    println!("Otherwise, follow the prompts below.\n");

    let stream = UnixStream::connect(socket).await?;
    let (reader, mut writer) = stream.split();
    let mut reader = BufReader::new(reader);

    // Request status
    let req = serde_json::json!({
        "id": 1,
        "method": "status",
        "params": {}
    });
    let line = serde_json::to_string(&req)?;
    writer.write_all(line.as_bytes()).await?;
    writer.write_all(b"\n").await?;

    let mut resp_line = String::new();
    reader.read_line(&mut resp_line).await?;
    let resp: serde_json::Value = serde_json::from_str(&resp_line)?;

    if let Some(result) = resp.get("result") {
        if result["authorized"].as_bool().unwrap_or(false) {
            println!("✅  Already logged in!");
            return Ok(());
        }
    }

    println!("⏳  Waiting for auth state updates from daemon…");
    println!("   (Make sure the daemon has your phone number in config)\n");

    // Listen for auth_state events
    let mut line_buf = String::new();
    loop {
        line_buf.clear();
        let n = reader.read_line(&mut line_buf).await?;
        if n == 0 {
            println!("Connection closed.");
            break;
        }

        if let Ok(msg) = serde_json::from_str::<serde_json::Value>(&line_buf) {
            if msg["type"].as_str() == Some("auth_state") {
                let state = msg["state"].as_str().unwrap_or("unknown");
                match state {
                    "ready" => {
                        println!("✅  Logged in successfully!");
                        break;
                    }
                    "wait_code" => {
                        let mut code = String::new();
                        print!("📱  Enter the code you received: ");
                        std::io::Write::flush(&mut std::io::stdout())?;
                        std::io::BufRead::read_line(
                            &mut std::io::stdin().lock(),
                            &mut code,
                        )?;
                        // Send code back via daemon
                        let auth_req = serde_json::json!({
                            "id": 2,
                            "method": "auth_code",
                            "params": { "code": code.trim() }
                        });
                        let l = serde_json::to_string(&auth_req)?;
                        writer.write_all(l.as_bytes()).await?;
                        writer.write_all(b"\n").await?;
                    }
                    "wait_password" => {
                        let mut pw = String::new();
                        print!("🔒  Enter 2FA password: ");
                        std::io::Write::flush(&mut std::io::stdout())?;
                        std::io::BufRead::read_line(
                            &mut std::io::stdin().lock(),
                            &mut pw,
                        )?;
                        let auth_req = serde_json::json!({
                            "id": 3,
                            "method": "auth_password",
                            "params": { "password": pw.trim() }
                        });
                        let l = serde_json::to_string(&auth_req)?;
                        writer.write_all(l.as_bytes()).await?;
                        writer.write_all(b"\n").await?;
                    }
                    "wait_phone" => {
                        let mut phone = String::new();
                        print!("📱  Enter phone number: ");
                        std::io::Write::flush(&mut std::io::stdout())?;
                        std::io::BufRead::read_line(
                            &mut std::io::stdin().lock(),
                            &mut phone,
                        )?;
                        let auth_req = serde_json::json!({
                            "id": 4,
                            "method": "auth_phone",
                            "params": { "phone": phone.trim() }
                        });
                        let l = serde_json::to_string(&auth_req)?;
                        writer.write_all(l.as_bytes()).await?;
                        writer.write_all(b"\n").await?;
                    }
                    other => {
                        println!("   Auth state: {other}");
                    }
                }
            }
        }
    }

    Ok(())
}
