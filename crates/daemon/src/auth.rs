//! Authentication flow: wait for user to log in via CLI/TUI before
//! the daemon starts accepting general requests.

use anyhow::Result;
use tdlib::enums::{AuthorizationState, InputPhoneNumber, Function};
use tdlib::functions;
use tracing::info;

use crate::handler::AppState;

/// Block until TDLib reaches `AuthorizationState::Ready`.
/// If a session already exists the transition is instant.
pub async fn ensure_authorized(state: &AppState) -> Result<()> {
    // Trigger the auth state machine
    state
        .td
        .send_async(Function::SetAuthenticationPhoneNumber(
            tdlib::functions::SetAuthenticationPhoneNumber {
                phone_number: state.config.phone.clone(),
                settings: tdlib::types::PhoneNumberAuthenticationSettings {
                    allow_flash_call: false,
                    allow_missed_call: false,
                    allow_sms_retriever_api: false,
                    is_current_phone_number: true,
                    authentication_tokens: Vec::new(),
                },
            },
        ))
        .await;

    info!(
        "Auth: if this is a fresh session, use `tg login` or TUI to enter phone/code."
    );

    // Poll until authorized — the auth state updates arrive via broadcast
    // and are also visible via tdlib receive loop.
    // For now, just wait until the flag flips (set by receive_loop).
    let mut interval = tokio::time::interval(std::time::Duration::from_millis(500));
    loop {
        interval.tick().await;
        if state.td.is_authorized() {
            info!("✅  Authorized");
            break;
        }
    }
    Ok(())
}
