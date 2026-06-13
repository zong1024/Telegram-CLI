//! Raw FFI wrapper around `libtdjson` — the TDLib shared library.
//!
//! This crate provides safe(r) Rust wrappers around the three C functions:
//!   - `td_json_client_create`
//!   - `td_json_client_send`
//!   - `td_json_client_receive`
//!   - `td_json_client_execute`
//!   - `td_json_client_destroy`
//!
//! All communication is raw JSON strings. Typed wrappers can be layered on top later.

use std::ffi::{CStr, CString};
use std::os::raw::{c_char, c_double, c_int};
use std::sync::Arc;

// ── FFI declarations ───────────────────────────────────────────────

type TdJsonClientPtr = *mut std::ffi::c_void;

#[cfg_attr(
    all(not(target_env = "msvc"), not(target_os = "macos")),
    link(name = "tdjson")
)]
extern "C" {
    fn td_json_client_create() -> TdJsonClientPtr;
    fn td_json_client_send(client: TdJsonClientPtr, request: *const c_char);
    fn td_json_client_receive(client: TdJsonClientPtr, timeout: c_double) -> *const c_char;
    fn td_json_client_execute(client: TdJsonClientPtr, request: *const c_char) -> *const c_char;
    fn td_json_client_destroy(client: TdJsonClientPtr);

    // TDLib >= 1.8: log verbosity
    fn td_set_log_verbosity_level(level: c_int);
    fn td_set_log_fatal_error_callback(callback: Option<unsafe extern "C" fn(*const c_char)>);
}

// ── Public API ─────────────────────────────────────────────────────

/// A handle to the TDLib JSON client.
///
/// Internally holds a raw pointer to the C client. `Send + Sync` because
/// the C client is thread-safe (it uses internal locks).
pub struct TdJson {
    ptr: TdJsonClientPtr,
}

// SAFETY: libtdjson's client is internally thread-safe.
unsafe impl Send for TdJson {}
unsafe impl Sync for TdJson {}

impl TdJson {
    /// Create a new TDLib JSON client.
    pub fn new() -> Self {
        let ptr = unsafe { td_json_client_create() };
        assert!(!ptr.is_null(), "td_json_client_create returned null");
        Self { ptr }
    }

    /// Create with log level. Call once before creating clients.
    pub fn set_log_verbosity(level: i32) {
        unsafe {
            td_set_log_verbosity_level(level);
        }
    }

    /// Set a fatal error callback (for logging).
    pub fn set_fatal_error_callback(cb: Option<unsafe extern "C" fn(*const c_char)>) {
        unsafe {
            td_set_log_fatal_error_callback(cb);
        }
    }

    /// Send a JSON request to TDLib. Does not return a response.
    ///
    /// `query` must be a valid JSON string.
    pub fn send(&self, query: &str) {
        let c_query = CString::new(query).expect("query contains null byte");
        unsafe {
            td_json_client_send(self.ptr, c_query.as_ptr());
        }
    }

    /// Receive a JSON response/update from TDLib.
    /// Returns `None` if the timeout expires with no data.
    ///
    /// `timeout` is in seconds.
    pub fn receive(&self, timeout: f64) -> Option<String> {
        let result = unsafe { td_json_client_receive(self.ptr, timeout) };
        if result.is_null() {
            None
        } else {
            // SAFETY: td_json_client_receive returns a null-terminated C string
            // that remains valid until the next call to receive/execute/send.
            Some(unsafe { CStr::from_ptr(result) }.to_string_lossy().into_owned())
        }
    }

    /// Synchronously execute a TDLib function (only for certain simple queries).
    /// Returns `None` if the function is not supported in synchronous mode.
    ///
    /// `query` must be a valid JSON string.
    pub fn execute(&self, query: &str) -> Option<String> {
        let c_query = CString::new(query).expect("query contains null byte");
        let result = unsafe { td_json_client_execute(self.ptr, c_query.as_ptr()) };
        if result.is_null() {
            None
        } else {
            Some(unsafe { CStr::from_ptr(result) }.to_string_lossy().into_owned())
        }
    }
}

impl Drop for TdJson {
    fn drop(&mut self) {
        unsafe {
            td_json_client_destroy(self.ptr);
        }
    }
}

/// Thread-safe wrapper that can be cloned and shared across threads.
/// Uses `Arc` internally — the underlying C client is the same pointer.
#[derive(Clone)]
pub struct SharedTdJson {
    inner: Arc<TdJson>,
}

impl SharedTdJson {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(TdJson::new()),
        }
    }

    /// Send a JSON request.
    pub fn send(&self, query: &str) {
        self.inner.send(query);
    }

    /// Receive a JSON response/update with timeout in seconds.
    pub fn receive(&self, timeout: f64) -> Option<String> {
        self.inner.receive(timeout)
    }

    /// Execute a synchronous TDLib function.
    pub fn execute(&self, query: &str) -> Option<String> {
        self.inner.execute(query)
    }

    /// Convenience: send a JSON value.
    pub fn send_json(&self, query: &serde_json::Value) {
        self.send(&query.to_string());
    }

    /// Convenience: send and return raw string.
    pub fn send_query(&self, query: &str) -> String {
        self.send(query);
        // TDLib responses arrive asynchronously via receive()
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_destroy() {
        // This test only works if libtdjson.so is available at link time.
        let client = TdJson::new();
        // execute is safe to call with a trivial query
        let result = client.execute(r#"{"@type": "getOption", "name": "version"}"#);
        // May return None if synchronous execute is not supported for this query
        if let Some(r) = result {
            assert!(r.contains("version") || r.contains("@type"));
        }
    }
}
