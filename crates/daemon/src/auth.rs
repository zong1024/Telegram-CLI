//! Authentication flow.
//!
//! Sends the initial TDLib authorization trigger and waits until
//! `AuthorizationState::Ready` is received via the event channel.

use anyhow::Result;
use tracing::info;

use crate::handler::AppState;

/// Wait until TDLib reaches authorized state.
///
/// If a session already exists, this returns immediately.
/// Otherwise, the daemon sends auth state events that clients
/// (tg login / tg-tui) can respond to with phone/code/password.
pub async fn ensure_authorized(state: &AppState) -> Result<()> {
    // If already authorized from a previous session, we're done
    if state.td.is_authorized() {
        info!("✅  Already authorized (existing session)");
        return Ok(());
    }

    // Trigger the auth state machine by sending getAuthorizationState
    state
        .td
        .send(serde_json::json!({
            "@type": "getAuthorizationState"
        }))
        .await;

    info!("⏳  Waiting for authorization…");
    info!("    Use `tg login` or `tg-tui` to complete the login flow.");

    // Wait for the authorized flag to flip
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
