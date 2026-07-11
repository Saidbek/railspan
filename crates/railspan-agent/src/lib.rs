//! Railspan agent HTTP ingest service.

use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use railspan_protocol::{IngestResponse, TraceBatch, PROTOCOL_VERSION};
use std::net::SocketAddr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

#[derive(Debug, Default)]
pub struct AgentMetrics {
    pub spans_received: AtomicU64,
    pub spans_accepted: AtomicU64,
    pub batches_received: AtomicU64,
}

#[derive(Clone)]
pub struct AgentState {
    pub api_key: Option<String>,
    pub metrics: Arc<AgentMetrics>,
}

pub fn app(state: AgentState) -> Router {
    Router::new()
        .route("/healthz", get(healthz))
        .route("/v1/traces", post(ingest_traces))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

async fn healthz(State(state): State<AgentState>) -> Json<serde_json::Value> {
    Json(serde_json::json!({
        "ok": true,
        "spans_received": state.metrics.spans_received.load(Ordering::Relaxed),
        "spans_accepted": state.metrics.spans_accepted.load(Ordering::Relaxed),
        "batches_received": state.metrics.batches_received.load(Ordering::Relaxed),
    }))
}

fn authorize(headers: &HeaderMap, expected: &Option<String>) -> bool {
    let Some(expected) = expected else {
        return true;
    };
    if expected.is_empty() {
        return true;
    }
    let Some(auth) = headers.get(axum::http::header::AUTHORIZATION) else {
        return false;
    };
    let Ok(auth) = auth.to_str() else {
        return false;
    };
    auth.strip_prefix("Bearer ")
        .map(|token| token == expected)
        .unwrap_or(false)
}

async fn ingest_traces(
    State(state): State<AgentState>,
    headers: HeaderMap,
    body: bytes::Bytes,
) -> Result<Json<IngestResponse>, StatusCode> {
    if !authorize(&headers, &state.api_key) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let batch: TraceBatch = serde_json::from_slice(&body).map_err(|err| {
        warn!(error = %err, "invalid trace batch JSON");
        StatusCode::BAD_REQUEST
    })?;

    if batch.protocol_version != PROTOCOL_VERSION {
        warn!(
            version = batch.protocol_version,
            "unsupported protocol version"
        );
        return Err(StatusCode::BAD_REQUEST);
    }

    let count = batch.spans.len();
    state.metrics.batches_received.fetch_add(1, Ordering::Relaxed);
    state
        .metrics
        .spans_received
        .fetch_add(count as u64, Ordering::Relaxed);
    state
        .metrics
        .spans_accepted
        .fetch_add(count as u64, Ordering::Relaxed);

    // MVP: log summary; persistence lands in E3.
    let resources: Vec<_> = batch
        .spans
        .iter()
        .filter_map(|s| s.resource.as_deref())
        .take(5)
        .collect();
    info!(
        spans = count,
        sdk = %batch.sdk.name,
        resources = ?resources,
        "accepted trace batch"
    );

    Ok(Json(IngestResponse {
        ok: true,
        accepted_spans: count,
        dropped_spans: 0,
    }))
}

pub async fn serve(addr: SocketAddr, state: AgentState) -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind(addr).await?;
    info!(%addr, "railspan agent listening");
    axum::serve(listener, app(state)).await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::{Request, StatusCode};
    use tower::ServiceExt;

    #[tokio::test]
    async fn healthz_ok() {
        let state = AgentState {
            api_key: None,
            metrics: Arc::new(AgentMetrics::default()),
        };
        let app = app(state);
        let res = app
            .oneshot(
                Request::builder()
                    .uri("/healthz")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn ingest_requires_key_when_configured() {
        let state = AgentState {
            api_key: Some("secret".into()),
            metrics: Arc::new(AgentMetrics::default()),
        };
        let app = app(state);
        let body = r#"{"protocol_version":1,"spans":[]}"#;
        let res = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/traces")
                    .header("content-type", "application/json")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn ingest_accepts_valid_batch() {
        let state = AgentState {
            api_key: Some("secret".into()),
            metrics: Arc::new(AgentMetrics::default()),
        };
        let app = app(state);
        let body = r#"{
            "protocol_version": 1,
            "sdk": {"name": "railspan-ruby", "version": "0.1.0", "language": "ruby"},
            "spans": [{
                "trace_id": "aa",
                "span_id": "bb",
                "name": "http.server",
                "kind": "http.server",
                "resource": "GET /",
                "start_time_unix_ns": 1,
                "end_time_unix_ns": 2,
                "status": "ok",
                "attributes": {}
            }]
        }"#;
        let res = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/v1/traces")
                    .header("content-type", "application/json")
                    .header("authorization", "Bearer secret")
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }
}
