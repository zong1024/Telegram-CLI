//! Event dispatcher — currently unused as event routing is handled
//! directly in server.rs via broadcast channels.
//! This module is reserved for future per-client event filtering.

#[allow(dead_code)]
pub struct ClientFilter {
    /// Only forward events matching these chat IDs (empty = all)
    pub chat_ids: Vec<i64>,
    /// Only forward these event names (empty = all)
    pub event_names: Vec<String>,
}
