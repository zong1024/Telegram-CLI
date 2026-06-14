//! `tg login` — interactive login via tgcd.

use anyhow::Result;
use serde_json::json;
use tg_core::config::TgConfig;
use tg_ipc::client::IpcClient;
use tg_ipc::protocol::ServerMessage;

pub async fn run() -> Result<()> {
    let config = TgConfig::load()?;
    let socket = &config.ipc.socket_path;

    if !socket.exists() {
        anyhow::bail!("Daemon not running. Start: tgcd");
    }

    println!("🔐  Logging in…\n");

    let mut client = IpcClient::connect(socket).await?;

    // Trigger auth state machine
    client.send_raw(serde_json::json!({
        "id": "login-trigger",
        "method": "auth_trigger",
        "params": {}
    })).await?;

    println!("⏳  Waiting for auth events…\n");

    // Single event loop — handles auth events and user input
    loop {
        let msg = client.read_message().await?;
        match msg {
            ServerMessage::AuthState(auth) => match auth.state.as_str() {
                "ready" => {
                    println!("✅  Logged in!");
                    break;
                }
                "wait_phone" => {
                    let phone = input("📱  Phone number")?;
                    client.send_raw(serde_json::json!({
                        "id": "login-phone",
                        "method": "auth_phone",
                        "params": { "phone": phone }
                    })).await?;
                }
                "wait_code" => {
                    let code = input("🔑  Code")?;
                    client.send_raw(serde_json::json!({
                        "id": "login-code",
                        "method": "auth_code",
                        "params": { "code": code }
                    })).await?;
                }
                "wait_password" => {
                    let pw = input("🔒  2FA password")?;
                    client.send_raw(serde_json::json!({
                        "id": "login-pw",
                        "method": "auth_password",
                        "params": { "password": pw }
                    })).await?;
                }
                other => {
                    println!("   Auth state: {other}");
                }
            },
            ServerMessage::Response(resp) => {
                // Ignore login ack responses
                if let Some(err) = resp.error {
                    println!("❌  Error: {}", err.message);
                }
            }
            ServerMessage::Event(_) => {
                // Ignore other events during login
            }
        }
    }

    Ok(())
}

fn input(prompt: &str) -> Result<String> {
    let mut buf = String::new();
    print!("{prompt}: ");
    std::io::Write::flush(&mut std::io::stdout())?;
    std::io::BufRead::read_line(&mut std::io::stdin().lock(), &mut buf)?;
    Ok(buf.trim().to_string())
}
