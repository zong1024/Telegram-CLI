//! Authentication flow.

use tracing::info;

use tg_core::config::TgConfig;
use tg_core::models::AuthState;

/// Wait for TDLib authorization.
#[allow(dead_code)]
pub async fn ensure_authorized(td: &tg_tdjson::TdClient, _config: &TgConfig) -> bool {
    let mut rx = tg_tdjson::subscribe_updates();

    // Trigger auth state machine
    td.send_no_wait(serde_json::json!({"@type": "getAuthorizationState"}));

    info!("⏳  Waiting for authorization…");

    loop {
        match rx.recv().await {
            Ok(update) => {
                if update.get("@type").and_then(|v| v.as_str()) == Some("updateAuthorizationState") {
                    let auth_state = &update["authorization_state"];
                    let state_type = auth_state.get("@type").and_then(|v| v.as_str()).unwrap_or("");

                    match AuthState::from_tdlib(state_type) {
                        AuthState::Ready => {
                            info!("✅  Authorized");
                            return true;
                        }
                        AuthState::WaitPhoneNumber => {
                            info!("📱  Need phone number — use `tg login`");
                        }
                        AuthState::WaitCode => {
                            info!("🔑  Need verification code — use `tg login`");
                        }
                        AuthState::WaitPassword => {
                            info!("🔒  Need 2FA password — use `tg login`");
                        }
                        AuthState::WaitTdlibParameters => {
                            info!("⚙️  Waiting for TDLib parameters…");
                        }
                        AuthState::WaitEncryptionKey => {
                            info!("🔑  Waiting for encryption key…");
                            td.send_no_wait(serde_json::json!({
                                "@type": "setDatabaseEncryptionKey",
                                "new_encryption_key": ""
                            }));
                        }
                        AuthState::Closed => {
                            info!("TDLib connection closed");
                            return false;
                        }
                        _ => {}
                    }
                }
            }
            Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
            Err(_) => return false,
        }
    }
}
