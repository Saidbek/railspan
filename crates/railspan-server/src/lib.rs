//! Railspan server: ingest, SQLite storage, query API, and embedded UI.

mod store;

pub use store::{
    CreateDeploy, DeployMarker, EndpointRow, NPlusOneEvent, SpanRow, Store, TraceDetail,
    TraceSummary,
};

use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use railspan_protocol::{IngestResponse, Span, TraceBatch, PROTOCOL_VERSION};
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tower_http::cors::CorsLayer;
use tower_http::trace::TraceLayer;
use tracing::{info, warn};

#[derive(Debug, Default)]
pub struct ServerMetrics {
    pub spans_received: AtomicU64,
    pub spans_accepted: AtomicU64,
    pub spans_dropped_sample: AtomicU64,
    pub batches_received: AtomicU64,
}

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<Store>,
    pub api_key: Option<String>,
    pub metrics: Arc<ServerMetrics>,
    pub sample_rate: f64,
    pub slow_ms: u64,
}

#[derive(Debug, Clone)]
pub struct ServeConfig {
    pub addr: SocketAddr,
    pub data_dir: PathBuf,
    pub api_key: Option<String>,
    pub sample_rate: f64,
    pub slow_ms: u64,
    pub retention_days: u64,
    pub n1_threshold: u32,
}

impl Default for ServeConfig {
    fn default() -> Self {
        Self {
            addr: "127.0.0.1:7421".parse().unwrap(),
            data_dir: PathBuf::from("./data"),
            api_key: None,
            sample_rate: 1.0,
            slow_ms: 500,
            retention_days: 7,
            n1_threshold: 5,
        }
    }
}

pub fn app(state: AppState) -> Router {
    Router::new()
        .route("/", get(ui_index))
        .route("/healthz", get(healthz))
        .route("/v1/traces", post(ingest_traces))
        .route("/v1/deploys", post(create_deploy))
        .route("/api/v1/endpoints", get(list_endpoints))
        .route("/api/v1/traces", get(list_traces))
        .route("/api/v1/traces/{trace_id}", get(get_trace))
        .route("/api/v1/n-plus-one", get(list_n_plus_one))
        .route("/api/v1/deploys", get(list_deploys))
        .route("/api/v1/stats", get(stats))
        .layer(CorsLayer::permissive())
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}

pub async fn serve(config: ServeConfig) -> anyhow::Result<()> {
    let db_path = config.data_dir.join("railspan.db");
    let store = Store::open(&db_path)
        .await?
        .with_n1_threshold(config.n1_threshold);
    let store = Arc::new(store);
    let state = AppState {
        store: store.clone(),
        api_key: config.api_key.clone(),
        metrics: Arc::new(ServerMetrics::default()),
        sample_rate: config.sample_rate.clamp(0.0, 1.0),
        slow_ms: config.slow_ms,
    };

    // Retention worker
    let retention_days = config.retention_days.max(1);
    tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(3600));
        loop {
            interval.tick().await;
            let cutoff = now_ns() as i64 - (retention_days as i64 * 86_400 * 1_000_000_000);
            match store.retain(cutoff).await {
                Ok((t, s)) if t > 0 || s > 0 => {
                    info!(traces_deleted = t, spans_deleted = s, "retention pass")
                }
                Ok(_) => {}
                Err(e) => warn!(error = %e, "retention failed"),
            }
        }
    });

    let listener = tokio::net::TcpListener::bind(config.addr).await?;
    info!(
        %config.addr,
        db = %db_path.display(),
        sample_rate = state.sample_rate,
        retention_days,
        "railspan serve listening"
    );
    axum::serve(listener, app(state)).await?;
    Ok(())
}

async fn ui_index() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

async fn healthz(State(state): State<AppState>) -> Json<serde_json::Value> {
    let (traces, spans, n1) = state.store.stats().await.unwrap_or((0, 0, 0));
    Json(serde_json::json!({
        "ok": true,
        "spans_received": state.metrics.spans_received.load(Ordering::Relaxed),
        "spans_accepted": state.metrics.spans_accepted.load(Ordering::Relaxed),
        "spans_dropped_sample": state.metrics.spans_dropped_sample.load(Ordering::Relaxed),
        "batches_received": state.metrics.batches_received.load(Ordering::Relaxed),
        "traces_stored": traces,
        "spans_stored": spans,
        "n_plus_one_events": n1,
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

fn sample_batch(batch: TraceBatch, sample_rate: f64, slow_ms: u64) -> (TraceBatch, usize) {
    if sample_rate >= 1.0 {
        return (batch, 0);
    }
    // Group by trace_id; keep whole traces if error/slow or sampled
    let mut by_trace: HashMap<String, Vec<Span>> = HashMap::new();
    for span in batch.spans {
        by_trace.entry(span.trace_id.clone()).or_default().push(span);
    }
    let mut kept = Vec::new();
    let mut dropped = 0usize;
    for (_tid, spans) in by_trace {
        let root = spans.iter().find(|s| s.parent_span_id.is_none());
        let is_error = spans.iter().any(|s| s.status == "error");
        let duration_ms = root
            .map(|s| {
                s.end_time_unix_ns
                    .saturating_sub(s.start_time_unix_ns) as f64
                    / 1_000_000.0
            })
            .unwrap_or(0.0);
        let keep = is_error
            || duration_ms >= slow_ms as f64
            || rand_keep(sample_rate, root.map(|s| s.trace_id.as_str()).unwrap_or(""));
        if keep {
            kept.extend(spans);
        } else {
            dropped += spans.len();
        }
    }
    (
        TraceBatch {
            protocol_version: batch.protocol_version,
            sdk: batch.sdk,
            resource: batch.resource,
            spans: kept,
        },
        dropped,
    )
}

fn rand_keep(rate: f64, seed: &str) -> bool {
    if rate <= 0.0 {
        return false;
    }
    // Deterministic per-trace pseudo-random from hash
    let mut h: u64 = 0xcbf29ce484222325;
    for b in seed.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x100000001b3);
    }
    let r = (h % 10_000) as f64 / 10_000.0;
    r < rate
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

    let incoming = batch.spans.len();
    state
        .metrics
        .batches_received
        .fetch_add(1, Ordering::Relaxed);
    state
        .metrics
        .spans_received
        .fetch_add(incoming as u64, Ordering::Relaxed);

    let (batch, dropped) = sample_batch(batch, state.sample_rate, state.slow_ms);
    if dropped > 0 {
        state
            .metrics
            .spans_dropped_sample
            .fetch_add(dropped as u64, Ordering::Relaxed);
    }

    // Drop health-like resources at root if present alone
    let batch = filter_noise(batch);

    match state.store.ingest_batch(&batch).await {
        Ok(accepted) => {
            state
                .metrics
                .spans_accepted
                .fetch_add(accepted as u64, Ordering::Relaxed);
            info!(
                spans = accepted,
                dropped_sample = dropped,
                sdk = %batch.sdk.name,
                "persisted trace batch"
            );
            Ok(Json(IngestResponse {
                ok: true,
                accepted_spans: accepted,
                dropped_spans: incoming.saturating_sub(accepted),
            }))
        }
        Err(err) => {
            warn!(error = %err, "failed to persist batch");
            Err(StatusCode::INTERNAL_SERVER_ERROR)
        }
    }
}

fn filter_noise(mut batch: TraceBatch) -> TraceBatch {
    let noise: HashSet<&str> = ["HealthController#show"].into_iter().collect();
    // Keep spans unless the entire trace is only a health root with no children worth keeping
    // Simple approach: drop roots whose resource is HealthController#show and their children
    let mut drop_traces = HashSet::new();
    for s in &batch.spans {
        if s.parent_span_id.is_none() {
            if let Some(r) = &s.resource {
                if noise.contains(r.as_str()) {
                    drop_traces.insert(s.trace_id.clone());
                }
            }
        }
    }
    if drop_traces.is_empty() {
        return batch;
    }
    batch.spans.retain(|s| !drop_traces.contains(&s.trace_id));
    batch
}

async fn create_deploy(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(body): Json<CreateDeploy>,
) -> Result<Json<DeployMarker>, StatusCode> {
    if !authorize(&headers, &state.api_key) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    state
        .store
        .create_deploy(body)
        .await
        .map(Json)
        .map_err(|e| {
            warn!(error = %e, "create_deploy failed");
            StatusCode::INTERNAL_SERVER_ERROR
        })
}

#[derive(Debug, Deserialize)]
pub struct RangeQuery {
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
        .unwrap_or_else(|| to.saturating_sub((q.hours * 3_600_000_000_000.0) as i64));
    (from, to)
}

async fn list_endpoints(
    State(state): State<AppState>,
    Query(q): Query<RangeQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let (from, to) = resolve_range(&q);
    let endpoints = state.store.list_endpoints(from, to).await.map_err(|e| {
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
    #[serde(default)]
    pub n1_only: bool,
    pub min_duration_ms: Option<f64>,
    pub kind: Option<String>,
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
            q.n1_only,
            q.min_duration_ms,
            q.kind.as_deref(),
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

async fn list_n_plus_one(
    State(state): State<AppState>,
    Query(q): Query<RangeQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let (from, to) = resolve_range(&q);
    let events = state
        .store
        .list_n_plus_one(from, to, 100)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({ "events": events })))
}

async fn list_deploys(
    State(state): State<AppState>,
    Query(q): Query<RangeQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let (from, to) = resolve_range(&q);
    let deploys = state
        .store
        .list_deploys(from, to, 100)
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({ "deploys": deploys })))
}

async fn stats(State(state): State<AppState>) -> Result<Json<serde_json::Value>, StatusCode> {
    let (traces, spans, n1) = state
        .store
        .stats()
        .await
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    Ok(Json(serde_json::json!({
        "traces": traces,
        "spans": spans,
        "n_plus_one_events": n1
    })))
}

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

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
            store: Arc::new(
                Store::open_in_memory()
                    .await
                    .unwrap()
                    .with_n1_threshold(5),
            ),
            api_key: Some("secret".into()),
            metrics: Arc::new(ServerMetrics::default()),
            sample_rate: 1.0,
            slow_ms: 500,
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
    async fn ingest_list_and_n1() {
        let state = test_state().await;
        let mut spans = vec![serde_json::json!({
            "trace_id": "aa",
            "span_id": "root",
            "name": "http.server",
            "kind": "http.server",
            "resource": "UsersController#with_posts",
            "start_time_unix_ns": 1000,
            "end_time_unix_ns": 50000,
            "status": "ok",
            "attributes": {"http.status_code": 200}
        })];
        for i in 0..6 {
            spans.push(serde_json::json!({
                "trace_id": "aa",
                "span_id": format!("sql{i}"),
                "parent_span_id": "root",
                "name": "sql",
                "kind": "sql",
                "resource": "SELECT posts WHERE user_id = ?",
                "start_time_unix_ns": 2000 + i * 10,
                "end_time_unix_ns": 2010 + i * 10,
                "status": "ok",
                "attributes": {}
            }));
        }
        let body = serde_json::json!({
            "protocol_version": 1,
            "sdk": {"name": "t", "version": "0", "language": "ruby"},
            "spans": spans
        })
        .to_string();

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

        let res = app(state.clone())
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
        assert_eq!(json["endpoints"][0]["n_plus_one_count"], 1);

        let res = app(state)
            .oneshot(
                Request::builder()
                    .uri("/api/v1/n-plus-one?from_ns=0&to_ns=999999999999999999")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }
}
