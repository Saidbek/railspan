//! Railspan server: ingest, SQLite storage, query API, and embedded UI.

mod store;

pub use store::{EndpointRow, SpanRow, Store, TraceDetail, TraceSummary};

use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use railspan_protocol::{IngestResponse, TraceBatch, PROTOCOL_VERSION};
use serde::Deserialize;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

#[derive(Debug, Default)]
pub struct ServerMetrics {
    pub spans_received: AtomicU64,
    pub spans_accepted: AtomicU64,
    pub batches_received: AtomicU64,
}

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<Store>,
    pub api_key: Option<String>,
    pub metrics: Arc<ServerMetrics>,
}

#[derive(Debug, Clone)]
pub struct ServeConfig {
    pub addr: SocketAddr,
    pub data_dir: PathBuf,
    pub api_key: Option<String>,
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/", get(ui_index))
        .route("/healthz", get(healthz))
        .route("/v1/traces", post(ingest_traces))
        .route("/api/v1/endpoints", get(list_endpoints))
        .route("/api/v1/traces", get(list_traces))
        .route("/api/v1/traces/{trace_id}", get(get_trace))
        .route("/api/v1/stats", get(stats))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

pub async fn serve(config: ServeConfig) -> anyhow::Result<()> {
    let db_path = config.data_dir.join("railspan.db");
    let store = Store::open(&db_path).await?;
    let state = AppState {
        store: Arc::new(store),
        api_key: config.api_key,
        metrics: Arc::new(ServerMetrics::default()),
    };
    let listener = tokio::net::TcpListener::bind(config.addr).await?;
    info!(%config.addr, db = %db_path.display(), "railspan serve listening");
    axum::serve(listener, app(state)).await?;
    Ok(())
}

async fn ui_index() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

async fn healthz(State(state): State<AppState>) -> Json<serde_json::Value> {
    let (traces, spans) = state.store.stats().await.unwrap_or((0, 0));
    Json(serde_json::json!({
        "ok": true,
        "spans_received": state.metrics.spans_received.load(Ordering::Relaxed),
        "spans_accepted": state.metrics.spans_accepted.load(Ordering::Relaxed),
        "batches_received": state.metrics.batches_received.load(Ordering::Relaxed),
        "traces_stored": traces,
        "spans_stored": spans,
    }))
}

fn authorize(headers: &HeaderMap, expected: &Option<String>) -> bool {
    let Some(expected) = expected else {
        return true;
    };
    if expected.is_empty() {
        return true;
    }
    let Some(auth) = headers.get(header::AUTHORIZATION) else {
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
    State(state): State<AppState>,
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
        warn!(version = batch.protocol_version, "unsupported protocol version");
        return Err(StatusCode::BAD_REQUEST);
    }

    let count = batch.spans.len();
    state
        .metrics
        .batches_received
        .fetch_add(1, Ordering::Relaxed);
    state
        .metrics
        .spans_received
        .fetch_add(count as u64, Ordering::Relaxed);

    match state.store.ingest_batch(&batch).await {
        Ok(accepted) => {
            state
                .metrics
                .spans_accepted
                .fetch_add(accepted as u64, Ordering::Relaxed);
            let resources: Vec<_> = batch
                .spans
                .iter()
                .filter_map(|s| s.resource.as_deref())
                .take(5)
                .collect();
            info!(
                spans = accepted,
                sdk = %batch.sdk.name,
                resources = ?resources,
                "persisted trace batch"
            );
            Ok(Json(IngestResponse {
                ok: true,
                accepted_spans: accepted,
                dropped_spans: count.saturating_sub(accepted),
            }))
        }
        Err(err) => {
            warn!(error = %err, "failed to persist batch");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

#[derive(Debug, Deserialize)]
pub struct RangeQuery {
    /// Hours lookback (default 24)
    #[serde(default = "default_hours")]
    pub hours: f64,
    pub from_ns: Option<i64>,
    pub to_ns: Option<i64>,
}

fn default_hours() -> f64 {
    24.0
}

fn resolve_range(q: &RangeQuery) -> (i64, i64) {
    let now = now_ns() as i64;
    let to = q.to_ns.unwrap_or(now);
    let from = q
        .from_ns
        .unwrap_or_else(|| to - (q.hours * 3_600_000_000_000.0) as i64);
    (from, to)
}

async fn list_endpoints(
    State(state): State<AppState>,
    Query(q): Query<RangeQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let (from, to) = resolve_range(&q);
    let endpoints = state
        .store
        .list_endpoints(from, to)
        .await
        .map_err(|e| {
            warn!(error = %e, "list_endpoints failed");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(serde_json::json!({ "endpoints": endpoints, "from_ns": from, "to_ns": to })))
}

#[derive(Debug, Deserialize)]
pub struct TracesQuery {
    #[serde(default = "default_hours")]
    pub hours: f64,
    pub from_ns: Option<i64>,
    pub to_ns: Option<i64>,
    pub resource: Option<String>,
    #[serde(default)]
    pub errors_only: bool,
    pub min_duration_ms: Option<f64>,
    #[serde(default = "default_limit")]
    pub limit: i64,
}

fn default_limit() -> i64 {
    50
}

async fn list_traces(
    State(state): State<AppState>,
    Query(q): Query<TracesQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let range = RangeQuery {
        hours: q.hours,
        from_ns: q.from_ns,
        to_ns: q.to_ns,
    };
    let (from, to) = resolve_range(&range);
    let traces = state
        .store
        .list_traces(
            from,
            to,
            q.resource.as_deref(),
            q.errors_only,
            q.min_duration_ms,
            q.limit.clamp(1, 500),
        )
        .await
        .map_err(|e| {
            warn!(error = %e, "list_traces failed");
            StatusCode::INTERNAL_SERVER_ERROR
        })?;
    Ok(Json(serde_json::json!({ "traces": traces, "from_ns": from, "to_ns": to })))
}

async fn get_trace(
    State(state): State<AppState>,
    Path(trace_id): Path<String>,
) -> Result<Response, StatusCode> {
    match state.store.get_trace(&trace_id).await {
        Ok(Some(detail)) => Ok(Json(detail).into_response()),
        Ok(None) => Err(StatusCode::NOT_FOUND),
        Err(e) => {
            warn!(error = %e, "get_trace failed");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

async fn stats(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let (traces, spans) = state.store.stats().await.map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({ "traces": traces, "spans": spans })))
}

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

/// Kept for older call sites / docs.
pub fn placeholder() -> &'static str {
    "railspan-server"
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    async fn test_state() -> AppState {
        AppState {
            store: Arc::new(Store::open_in_memory().await.unwrap()),
            api_key: Some("secret".into()),
            metrics: Arc::new(ServerMetrics::default()),
        }
    }

    #[tokio::test]
    async fn healthz_ok() {
        let app = app(test_state().await);
        let res = app
            .oneshot(Request::builder().uri("/healthz").body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn ingest_and_list_endpoints() {
        let state = test_state().await;
        let body = r#"{
            "protocol_version": 1,
            "sdk": {"name": "t", "version": "0", "language": "ruby"},
            "resource": {"service.name": "demo"},
            "spans": [{
                "trace_id": "aa",
                "span_id": "bb",
                "name": "http.server",
                "kind": "http.server",
                "resource": "UsersController#show",
                "start_time_unix_ns": 1000000000,
                "end_time_unix_ns": 1120000000,
                "status": "ok",
                "attributes": {"http.status_code": 200}
            }]
        }"#;
        let res = app(state.clone())
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

        let res = app(state)
            .oneshot(
                Request::builder()
                    .uri("/api/v1/endpoints?from_ns=0&to_ns=999999999999999999")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(json["endpoints"][0]["resource"], "UsersController#show");
        assert_eq!(json["endpoints"][0]["count"], 1);
    }
}
