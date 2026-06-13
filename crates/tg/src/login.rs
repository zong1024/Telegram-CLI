//! `tg login` — interactive login via tgcd.

use anyhow::Result;
use serde_json::json;
use tg_core::config::TgConfig;
use tg_ipc::client::IpcClient;
use tg_ipc::protocol::{methods, ServerMessage};

pub async fn run() -> Result<()> {
    let config = TgConfig::load()?;
    let socket = &config.ipc.socket_path;

    if !socket.exists() {
        anyhow::bail!("Daemon not running. Start: tgcd");
    }

    println!("🔐  Logging in…\n");
    let mut client = IpcClient::connect(socket).await?;

    // Check status
    let _status = client.call(methods::GET_STATUS, json!({})).await?;

    // Trigger auth state machine — TDLib will respond with current auth state
    client.send_request(&tg_ipc::protocol::Request {
        id: uuid::Uuid::new_v4().to_string(),
        method: "auth_trigger".to_string(),
        params: json!({}),
    }).await?;

    println!("⏳  Waiting for auth events…\n");

    loop {
        let msg = client.read_message().await?;
        match msg {
            ServerMessage::AuthState(auth) => match auth.state.as_str() {
                "ready" => { println!("✅  Logged in!"); break; }
                "wait_phone" => {
                    let phone = input("📱  Phone number")?;
                    client.call(methods::AUTH_PHONE, json!({"phone": phone})).await?;
                }
                "wait_code" => {
                    let code = input("🔑  Code")?;
                    client.call(methods::AUTH_CODE, json!({"code": code})).await?;
                }
                "wait_password" => {
                    let pw = input("🔒  2FA password")?;
                    client.call(methods::AUTH_PASSWORD, json!({"password": pw})).await?;
                }
                other => println!("   Auth: {other}"),
            },
            _ => {}
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
