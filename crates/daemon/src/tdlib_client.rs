//! Thin wrapper around `tdlib::Client` that runs the receive loop
//! and dispatches TDLib updates via a broadcast channel.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use anyhow::Result;
use serde_json::Value as JsonValue;
use tokio::sync::{broadcast, oneshot};
use tracing::{debug, error, warn};
use tdlib::enums::Update;
use tdlib::functions;
use tdlib::types::UpdateAuthorizationState;

use tg_common::config::TgConfig;

/// Identifier for a pending TDLib request so we can match the response.
type RequestId = u64;

/// A shared handle to the running TDLib client.
/// Methods are synchronous TDLib calls wrapped in `tokio::task::spawn_blocking`.
#[derive(Clone)]
pub struct TdClient {
    inner: Arc<TdInner>,
}

struct TdInner {
    client: tdlib::Client,
    /// Broadcast channel for forwarding TDLib updates to the rest of the daemon.
    event_tx: broadcast::Sender<JsonValue>,
    /// Set when authorization is ready.
    authorized: AtomicBool,
}

impl TdClient {
    /// Create a new TDLib client, set database parameters, start the receive loop.
    pub fn new(config: &TgConfig, event_tx: broadcast::Sender<JsonValue>) -> Result<Self> {
        let client = tdlib::Client::builder()
            .database_directory(config.database_dir.to_string_lossy())
            .use_test_dc(config.test)
            .verbosity(config.verbosity)
            .build()?;

        let inner = Arc::new(TdInner {
            client,
            event_tx,
            authorized: AtomicBool::new(false),
        });

        // Spawn the TDLib receive loop
        let inner_clone = inner.clone();
        std::thread::spawn(move || {
            receive_loop(inner_clone);
        });

        // Set TDLib database encryption key (empty = no extra encryption)
        let td = Self { inner };
        td.send(functions::set_database_encryption_key(
            tdlib::types::SetDatabaseEncryptionKey {
                new_encryption_key: Vec::new(),
            },
        ));

        Ok(td)
    }

    /// Send a TDLib function and return the raw JSON response.
    /// This is blocking — callers must wrap in `spawn_blocking` or call from sync context.
    pub fn send(&self, func: tdlib::enums::Function) -> JsonValue {
        let raw = serde_json::to_value(&func).unwrap_or_default();
        let resp = self.inner.client.send(raw);
        serde_json::from_str(&resp).unwrap_or(JsonValue::Null)
    }

    /// Async wrapper around `send`.
    pub async fn send_async(&self, func: tdlib::enums::Function) -> JsonValue {
        let td = self.clone();
        tokio::task::spawn_blocking(move || td.send(func))
            .await
            .unwrap_or(JsonValue::Null)
    }

    /// Execute a TDLib function synchronously (rarely used, for simple queries).
    pub fn execute(&self, func: tdlib::enums::Function) -> Option<JsonValue> {
        let raw = serde_json::to_value(&func).ok()?;
        let resp = self.inner.client.execute(Some(raw))?;
        serde_json::from_str(&resp).ok()
    }

    /// Set the authorized flag.
    pub fn set_authorized(&self, v: bool) {
        self.inner.authorized.store(v, Ordering::SeqCst);
    }

    pub fn is_authorized(&self) -> bool {
        self.inner.authorized.load(Ordering::SeqCst)
    }
}

/// Background thread: continuously receive TDLib updates and forward them
/// to the broadcast channel.
fn receive_loop(inner: Arc<TdInner>) {
    loop {
        let raw = inner.client.receive(Duration::from_secs(1).as_secs_f64());
        let raw = match raw {
            Some(r) => r,
            None => continue,
        };

        // Parse into typed update if possible, else forward raw JSON
        if let Ok(update) = serde_json::from_str::<Update>(&raw) {
            match &update {
                Update::AuthorizationState(UpdateAuthorizationState { authorization_state }) => {
                    let state_str = match authorization_state {
                        tdlib::enums::AuthorizationState::WaitPhoneNumber => "wait_phone",
                        tdlib::enums::AuthorizationState::WaitCode(_) => "wait_code",
                        tdlib::enums::AuthorizationState::WaitPassword(_) => "wait_password",
                        tdlib::enums::AuthorizationState::Ready => {
                            inner.authorized.store(true, Ordering::SeqCst);
                            "ready"
                        }
                        tdlib::enums::AuthorizationState::Closing => "closing",
                        tdlib::enums::AuthorizationState::Closed => "closed",
                        tdlib::enums::AuthorizationState::WaitRegistration(_) => "wait_registration",
                        tdlib::enums::AuthorizationState::WaitOtherDeviceConfirmation(_) => {
                            "wait_other_device"
                        }
                        tdlib::enums::AuthorizationState::LoggingOut => "logging_out",
                    };
                    let ev = serde_json::json!({
                        "type": "event",
                        "name": "auth_update",
                        "data": { "state": state_str }
                    });
                    let _ = inner.event_tx.send(ev);
                }
                Update::NewMessage(_) => {
                    if let Ok(v) = serde_json::to_value(&update) {
                        let ev = serde_json::json!({
                            "type": "event",
                            "name": "new_message",
                            "data": v
                        });
                        let _ = inner.event_tx.send(ev);
                    }
                }
                _ => {
                    debug!("tdlib update: {:?}", update);
                }
            }
        } else {
            debug!("tdlib raw: {}", raw);
        }
    }
}
