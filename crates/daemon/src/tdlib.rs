//! TDLib client — wraps `tg_tdjson::SharedTdJson` with an async-friendly API.
//!
//! Runs a background receive loop in a dedicated thread and broadcasts
//! TDLib updates via a `tokio::sync::broadcast` channel.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use anyhow::Result;
use serde_json::Value as JsonValue;
use tokio::sync::broadcast;
use tracing::{debug, info, warn};

use tg_common::config::TgConfig;

/// Async handle to the TDLib client.
#[derive(Clone)]
pub struct TdClient {
    inner: Arc<TdInner>,
}

struct TdInner {
    td: tg_tdjson::SharedTdJson,
    event_tx: broadcast::Sender<JsonValue>,
    authorized: AtomicBool,
}

impl TdClient {
    /// Create a new TDLib client and start the background receive loop.
    pub fn new(config: &TgConfig, event_tx: broadcast::Sender<JsonValue>) -> Result<Self> {
        // Set log verbosity before creating client
        tg_tdjson::TdJson::set_log_verbosity(config.verbosity);

        let td = tg_tdjson::SharedTdJson::new();

        let inner = Arc::new(TdInner {
            td,
            event_tx,
            authorized: AtomicBool::new(false),
        });

        // Set database encryption key (empty = no extra encryption)
        let set_key = serde_json::json!({
            "@type": "setDatabaseEncryptionKey",
            "new_encryption_key": ""
        });
        inner.td.send_json(&set_key);

        // Spawn the receive loop in a dedicated thread
        let inner_clone = inner.clone();
        std::thread::Builder::new()
            .name("tdlib-recv".into())
            .spawn(move || receive_loop(inner_clone))?;

        // Configure TDLib with database parameters
        let set_params = serde_json::json!({
            "@type": "setTdlibParameters",
            "database_directory": config.tdlib_dir.to_string_lossy(),
            "use_test_dc": config.test,
            "api_id": config.api_id,
            "api_hash": config.api_hash,
            "system_language_code": "en",
            "device_model": "Telegram-CLI",
            "system_version": std::env::consts::OS,
            "application_version": env!("CARGO_PKG_VERSION"),
            "enable_storage_optimizer": true
        });
        inner.td.send_json(&set_params);

        Ok(Self { inner })
    }

    /// Send a JSON query to TDLib (non-blocking via spawn_blocking).
    pub async fn send(&self, query: JsonValue) {
        let td = self.inner.td.clone();
        let qs = query.to_string();
        tokio::task::spawn_blocking(move || td.send(&qs)).await.ok();
    }

    /// Check if TDLib is authorized.
    pub fn is_authorized(&self) -> bool {
        self.inner.authorized.load(Ordering::SeqCst)
    }

    /// Set the authorized flag.
    pub fn set_authorized(&self, v: bool) {
        self.inner.authorized.store(v, Ordering::SeqCst);
    }

    /// Access the broadcast event channel.
    pub fn subscribe(&self) -> broadcast::Receiver<JsonValue> {
        self.inner.event_tx.subscribe()
    }
}

/// Background receive loop — runs in a dedicated OS thread.
fn receive_loop(inner: Arc<TdInner>) {
    info!("TDLib receive loop started");
    loop {
        let raw = match inner.td.receive(1.0) {
            Some(r) => r,
            None => continue, // timeout, try again
        };

        let val: JsonValue = match serde_json::from_str(&raw) {
            Ok(v) => v,
            Err(e) => {
                warn!("failed to parse TDLib response: {e}");
                continue;
            }
        };

        let update_type = val["@type"].as_str().unwrap_or("");

        match update_type {
            "updateAuthorizationState" => {
                let auth_state = &val["authorization_state"];
                let state_type = auth_state["@type"].as_str().unwrap_or("unknown");

                let state_str = match state_type {
                    "authorizationStateWaitPhoneNumber" => "wait_phone",
                    "authorizationStateWaitCode" => "wait_code",
                    "authorizationStateWaitPassword" => "wait_password",
                    "authorizationStateReady" => {
                        inner.authorized.store(true, Ordering::SeqCst);
                        info!("✅  TDLib authorized");
                        "ready"
                    }
                    "authorizationStateClosing" => "closing",
                    "authorizationStateClosed" => {
                        info!("TDLib connection closed");
                        "closed"
                    }
                    "authorizationStateWaitRegistration" => "wait_registration",
                    "authorizationStateWaitOtherDeviceConfirmation" => "wait_other_device",
                    "authorizationStateLoggingOut" => "logging_out",
                    _ => state_type,
                };

                let ev = serde_json::json!({
                    "type": "auth_state",
                    "state": state_str
                });
                let _ = inner.event_tx.send(ev);
            }
            "updateNewMessage" => {
                let ev = serde_json::json!({
                    "type": "event",
                    "name": "new_message",
                    "data": val
                });
                let _ = inner.event_tx.send(ev);
            }
            _ => {
                debug!("tdlib update: {}", update_type);
            }
        }
    }
}
