//! TDLib interaction layer.
//!
//! Uses `tg_tdjson::TdClient` (multi-client API with @extra tracking).

use serde_json::Value as JsonValue;

/// Send a TDLib query and wait for the response.
pub async fn query(td: &tg_tdjson::TdClient, query: JsonValue) -> anyhow::Result<JsonValue> {
    td.send(query).await
}

/// Fire-and-forget TDLib query.
pub fn notify(td: &tg_tdjson::TdClient, query: JsonValue) {
    td.send_no_wait(query);
}
