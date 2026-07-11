//! Shared protocol types for Railspan ingest.

use serde::{Deserialize, Serialize};
use std::collections::HashMap;

pub const PROTOCOL_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceBatch {
    pub protocol_version: u32,
    #[serde(default)]
    pub sdk: SdkInfo,
    #[serde(default)]
    pub resource: HashMap<String, serde_json::Value>,
    pub spans: Vec<Span>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct SdkInfo {
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub language: String,
    #[serde(default)]
    pub runtime: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Span {
    pub trace_id: String,
    pub span_id: String,
    #[serde(default)]
    pub parent_span_id: Option<String>,
    pub name: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub resource: Option<String>,
    pub start_time_unix_ns: u64,
    pub end_time_unix_ns: u64,
    #[serde(default = "default_status")]
    pub status: String,
    #[serde(default)]
    pub attributes: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub events: Vec<SpanEvent>,
}

fn default_status() -> String {
    "ok".to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SpanEvent {
    pub time_unix_ns: u64,
    pub name: String,
    #[serde(default)]
    pub attributes: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IngestResponse {
    pub ok: bool,
    pub accepted_spans: usize,
    #[serde(default)]
    pub dropped_spans: usize,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_minimal_batch() {
        let json = r#"{
            "protocol_version": 1,
            "spans": [{
                "trace_id": "abc",
                "span_id": "def",
                "name": "http.server",
                "start_time_unix_ns": 1,
                "end_time_unix_ns": 2
            }]
        }"#;
        let batch: TraceBatch = serde_json::from_str(json).unwrap();
        assert_eq!(batch.protocol_version, 1);
        assert_eq!(batch.spans.len(), 1);
        assert_eq!(batch.spans[0].status, "ok");
    }
}
