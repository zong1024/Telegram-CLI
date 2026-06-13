//! `tg login` — interactive login via TDLib through the daemon.

use anyhow::Result;
use serde_json::json;
use tg_common::config::TgConfig;
use tg_common::ipc::IpcClient;
use tg_common::protocol::{methods, ServerMessage};

pub async fn run() -> Result<()> {
    let config = TgConfig::load()?;
    let socket = &config.socket_path;

    if !socket.exists() {
        anyhow::bail!(
            "Daemon not running. Start it first: tgcd\n\
             (Ensure phone is set in config for auto-login.)"
        );
    }

    println!("🔐  Logging in…\n");
    println!("The daemon handles authentication via TDLib.");
    println!("If a session already exists, you're already logged in.\n");

    let mut client = IpcClient::connect(socket).await?;

    // Request status to check if already authorized
    let status = client.call(methods::GET_STATUS, json!({})).await?;
    if status["authorized"].as_bool().unwrap_or(false) {
        println!("✅  Already logged in!");
        return Ok(());
    }

    println!("⏳  Waiting for auth state updates from daemon…");
    println!("   (Make sure the daemon has your phone number in config)\n");

    // Listen for auth_state events and respond interactively
    loop {
        let msg = client.read_message().await?;
        match msg {
            ServerMessage::AuthState(auth) => {
                match auth.state.as_str() {
                    "ready" => {
                        println!("✅  Logged in successfully!");
                        break;
                    }
                    "wait_phone" => {
                        let phone = read_input("📱  Enter phone number")?;
                        client
                            .call(methods::AUTH_PHONE, json!({ "phone": phone }))
                            .await?;
                    }
                    "wait_code" => {
                        let code = read_input("🔑  Enter the code you received")?;
                        client
                            .call(methods::AUTH_CODE, json!({ "code": code }))
                            .await?;
                    }
                    "wait_password" => {
                        let pw = read_input("🔒  Enter 2FA password")?;
                        client
                            .call(methods::AUTH_PASSWORD, json!({ "password": pw }))
                            .await?;
                    }
                    other => {
                        println!("   Auth state: {other}");
                    }
                }
            }
            _ => {
                // Skip other messages during login
            }
        }
    }

    Ok(())
}

fn read_input(prompt: &str) -> Result<String> {
    let mut buf = String::new();
    print!("{prompt}: ");
    std::io::Write::flush(&mut std::io::stdout())?;
    std::io::BufRead::read_line(&mut std::io::stdin().lock(), &mut buf)?;
    Ok(buf.trim().to_string())
}
