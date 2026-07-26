//! Railspan server: ingest, SQLite storage, query API, and embedded UI.

mod store;

pub use store::{
    CreateDeploy, DeployMarker, EndpointRow, NPlusOneEvent, SpanRow, Store, TraceDetail,
    TraceSummary,
};

use axum::extract::{Path, Query, State};
use axum::http::{header, HeaderMap, Request, StatusCode};
use axum::middleware::{from_fn_with_state, Next};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use railspan_protocol::{IngestAdvice, IngestResponse, Span, TraceBatch, PROTOCOL_VERSION};
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

/// Reject batches larger than this (cardinality / DoS guard).
pub const MAX_SPANS_PER_BATCH: usize = 5_000;
/// Max attribute keys retained per span.
pub const MAX_ATTRIBUTES_PER_SPAN: usize = 64;
/// Max events retained per span.
pub const MAX_EVENTS_PER_SPAN: usize = 16;
/// Soft load thresholds (spans / rolling minute) for adaptive sampling advice.
const LOAD_SOFT: u64 = 20_000;
const LOAD_HARD: u64 = 50_000;
const LOAD_CRITICAL: u64 = 100_000;

#[derive(Debug, Default)]
pub struct ServerMetrics {
    pub spans_received: AtomicU64,
    pub spans_accepted: AtomicU64,
    pub spans_dropped_sample: AtomicU64,
    pub spans_dropped_cardinality: AtomicU64,
    pub batches_received: AtomicU64,
    pub batches_rejected: AtomicU64,
    /// Spans counted in the current adaptive-sampling window.
    pub window_spans: AtomicU64,
    /// Unix seconds when the current load window started.
    pub window_start_secs: AtomicU64,
}

#[derive(Clone)]
pub struct AppState {
    pub store: Arc<Store>,
    /// Bearer token required for `/v1/*` ingest.
    pub api_key: Option<String>,
    /// Bearer token required for `/api/*` query routes. Falls back to `api_key`.
    pub ui_token: Option<String>,
    pub metrics: Arc<ServerMetrics>,
    pub sample_rate: f64,
    pub slow_ms: u64,
    /// Optional app checkout root for `GET /api/v1/source` code highlight.
    pub source_root: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct ServeConfig {
    pub addr: SocketAddr,
    pub data_dir: PathBuf,
    pub api_key: Option<String>,
    pub ui_token: Option<String>,
    pub sample_rate: f64,
    pub slow_ms: u64,
    pub retention_days: u64,
    pub n1_threshold: u32,
    /// Local path to application source for UI code snippets.
    pub source_root: Option<PathBuf>,
}

impl Default for ServeConfig {
    fn default() -> Self {
        Self {
            addr: "127.0.0.1:7421".parse().unwrap(),
            data_dir: PathBuf::from("./data"),
            api_key: None,
            ui_token: None,
            sample_rate: 1.0,
            slow_ms: 500,
            retention_days: 7,
            n1_threshold: 5,
            source_root: None,
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
        .route("/api/v1/source", get(get_source))
        .layer(from_fn_with_state(state.clone(), ui_auth_middleware))
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
    let ui_token = config
        .ui_token
        .clone()
        .or_else(|| config.api_key.clone())
        .filter(|s| !s.is_empty());
    let source_root = config.source_root.and_then(|p| {
        let canon = std::fs::canonicalize(&p).unwrap_or(p);
        if canon.is_dir() {
            Some(canon)
        } else {
            warn!(path = %canon.display(), "source_root is not a directory; source API disabled");
            None
        }
    });
    let state = AppState {
        store: store.clone(),
        api_key: config.api_key.clone().filter(|s| !s.is_empty()),
        ui_token,
        metrics: Arc::new(ServerMetrics {
            window_start_secs: AtomicU64::new(now_secs()),
            ..ServerMetrics::default()
        }),
        sample_rate: config.sample_rate.clamp(0.0, 1.0),
        slow_ms: config.slow_ms,
        source_root,
    };

    // Retention worker — hourly TTL so disk stays bounded.
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
        ui_auth = state.ui_token.is_some(),
        ingest_auth = state.api_key.is_some(),
        source_root = state
            .source_root
            .as_ref()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| "(none)".into()),
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
        "spans_dropped_cardinality": state.metrics.spans_dropped_cardinality.load(Ordering::Relaxed),
        "batches_received": state.metrics.batches_received.load(Ordering::Relaxed),
        "batches_rejected": state.metrics.batches_rejected.load(Ordering::Relaxed),
        "traces_stored": traces,
        "spans_stored": spans,
        "n_plus_one_events": n1,
        "advised_sample_rate": compute_advice_rate(&state),
    }))
}

fn authorize(headers: &HeaderMap, expected: &Option<String>) -> bool {
    let Some(expected) = expected else {
        return true;
    };
    if expected.is_empty() {
        return true;
    }
    bearer_token(headers).is_some_and(|token| token == expected)
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let auth = headers.get(header::AUTHORIZATION)?.to_str().ok()?;
    auth.strip_prefix("Bearer ")
}

/// Protect query API when a UI token (or shared API key) is configured.
/// Ingest (`/v1/*`) and health stay on their own auth rules; static UI is public
/// but JS must send the Bearer token for `/api/*`.
async fn ui_auth_middleware(
    State(state): State<AppState>,
    req: Request<axum::body::Body>,
    next: Next,
) -> Response {
    let path = req.uri().path();
    let needs_ui_auth = path.starts_with("/api/");
    if needs_ui_auth && !authorize(req.headers(), &state.ui_token) {
        return (
            StatusCode::UNAUTHORIZED,
            [(header::WWW_AUTHENTICATE, "Bearer realm=\"railspan\"")],
            "unauthorized",
        )
            .into_response();
    }
    next.run(req).await
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

fn record_load(metrics: &ServerMetrics, span_count: usize) {
    let now = now_secs();
    let start = metrics.window_start_secs.load(Ordering::Relaxed);
    if now.saturating_sub(start) >= 60 {
        metrics.window_start_secs.store(now, Ordering::Relaxed);
        metrics.window_spans.store(0, Ordering::Relaxed);
    }
    metrics
        .window_spans
        .fetch_add(span_count as u64, Ordering::Relaxed);
}

/// Adaptive sample-rate advice from recent ingest pressure + configured floor.
fn compute_advice_rate(state: &AppState) -> f64 {
    let configured = state.sample_rate;
    let window = state.metrics.window_spans.load(Ordering::Relaxed);
    let pressure_cap = if window >= LOAD_CRITICAL {
        0.05
    } else if window >= LOAD_HARD {
        0.1
    } else if window >= LOAD_SOFT {
        0.25
    } else {
        1.0
    };
    configured.min(pressure_cap).clamp(0.0, 1.0)
}

fn ingest_advice(state: &AppState) -> IngestAdvice {
    IngestAdvice {
        sample_rate: Some(compute_advice_rate(state)),
    }
}

/// Truncate high-cardinality fields before persistence.
fn sanitize_batch(mut batch: TraceBatch) -> (TraceBatch, usize) {
    let mut dropped = 0usize;
    let original = batch.spans.len();
    if batch.spans.len() > MAX_SPANS_PER_BATCH {
        batch.spans.truncate(MAX_SPANS_PER_BATCH);
        dropped += original.saturating_sub(batch.spans.len());
    }
    for span in &mut batch.spans {
        if span.name.len() > 256 {
            span.name = span.name.chars().take(256).collect();
        }
        if let Some(ref mut r) = span.resource {
            if r.len() > 1024 {
                *r = r.chars().take(1024).collect();
            }
        }
        if span.kind.len() > 64 {
            span.kind = span.kind.chars().take(64).collect();
        }
        if span.attributes.len() > MAX_ATTRIBUTES_PER_SPAN {
            let keys: Vec<_> = span.attributes.keys().cloned().collect();
            for k in keys.into_iter().skip(MAX_ATTRIBUTES_PER_SPAN) {
                span.attributes.remove(&k);
            }
        }
        // Cap string attribute values
        for (_k, v) in span.attributes.iter_mut() {
            if let Some(s) = v.as_str() {
                if s.len() > 2048 {
                    *v = serde_json::Value::String(s.chars().take(2048).collect());
                }
            }
        }
        if span.events.len() > MAX_EVENTS_PER_SPAN {
            span.events.truncate(MAX_EVENTS_PER_SPAN);
        }
        for ev in &mut span.events {
            if ev.name.len() > 128 {
                ev.name = ev.name.chars().take(128).collect();
            }
            if ev.attributes.len() > MAX_ATTRIBUTES_PER_SPAN {
                let keys: Vec<_> = ev.attributes.keys().cloned().collect();
                for k in keys.into_iter().skip(MAX_ATTRIBUTES_PER_SPAN) {
                    ev.attributes.remove(&k);
                }
            }
        }
    }
    (batch, dropped)
}

fn sample_batch(batch: TraceBatch, sample_rate: f64, slow_ms: u64) -> (TraceBatch, usize) {
    if sample_rate >= 1.0 {
        return (batch, 0);
    }
    // Group by trace_id; keep whole traces if error/slow or sampled
    let mut by_trace: HashMap<String, Vec<Span>> = HashMap::new();
    for span in batch.spans {
        by_trace
            .entry(span.trace_id.clone())
            .or_default()
            .push(span);
    }
    let mut kept = Vec::new();
    let mut dropped = 0usize;
    for (_tid, spans) in by_trace {
        let root = spans.iter().find(|s| s.parent_span_id.is_none());
        let is_error = spans.iter().any(|s| s.status == "error");
        let duration_ms = root
            .map(|s| s.end_time_unix_ns.saturating_sub(s.start_time_unix_ns) as f64 / 1_000_000.0)
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

    // Hard size guard before JSON parse allocates huge vectors of bad data.
    if body.len() > 16 * 1024 * 1024 {
        state
            .metrics
            .batches_rejected
            .fetch_add(1, Ordering::Relaxed);
        warn!(bytes = body.len(), "batch body too large");
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }

    let batch: TraceBatch = serde_json::from_slice(&body).map_err(|err| {
        warn!(error = %err, "invalid trace batch JSON");
        state
            .metrics
            .batches_rejected
            .fetch_add(1, Ordering::Relaxed);
        StatusCode::BAD_REQUEST
    })?;

    if batch.protocol_version != PROTOCOL_VERSION {
        warn!(
            version = batch.protocol_version,
            "unsupported protocol version"
        );
        state
            .metrics
            .batches_rejected
            .fetch_add(1, Ordering::Relaxed);
        return Err(StatusCode::BAD_REQUEST);
    }

    if batch.spans.len() > MAX_SPANS_PER_BATCH {
        state
            .metrics
            .batches_rejected
            .fetch_add(1, Ordering::Relaxed);
        warn!(spans = batch.spans.len(), "batch span count exceeds limit");
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
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
    record_load(&state.metrics, incoming);

    let (batch, card_dropped) = sanitize_batch(batch);
    if card_dropped > 0 {
        state
            .metrics
            .spans_dropped_cardinality
            .fetch_add(card_dropped as u64, Ordering::Relaxed);
    }

    let (batch, sample_dropped) = sample_batch(batch, state.sample_rate, state.slow_ms);
    if sample_dropped > 0 {
        state
            .metrics
            .spans_dropped_sample
            .fetch_add(sample_dropped as u64, Ordering::Relaxed);
    }

    // Drop health-like resources at root if present alone
    let batch = filter_noise(batch);
    let advice = ingest_advice(&state);

    match state.store.ingest_batch(&batch).await {
        Ok(accepted) => {
            state
                .metrics
                .spans_accepted
                .fetch_add(accepted as u64, Ordering::Relaxed);
            info!(
                spans = accepted,
                dropped_sample = sample_dropped,
                dropped_cardinality = card_dropped,
                sdk = %batch.sdk.name,
                advised_sample_rate = advice.sample_rate,
                "persisted trace batch"
            );
            Ok(Json(IngestResponse {
                ok: true,
                accepted_spans: accepted,
                dropped_spans: incoming.saturating_sub(accepted),
                advice: Some(advice),
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
    Ok(Json(
        serde_json::json!({ "endpoints": endpoints, "from_ns": from, "to_ns": to }),
    ))
}

#[derive(Debug, Deserialize)]
pub struct SourceQuery {
    /// Path relative to source_root (e.g. app/controllers/users_controller.rb)
    pub path: String,
    pub line: Option<i64>,
    /// Context lines above/below (default 5, max 20)
    #[serde(default = "default_context")]
    pub context: u32,
}

fn default_context() -> u32 {
    5
}

const MAX_SOURCE_BYTES: u64 = 512 * 1024;

/// Read a snippet of application source for UI code highlight.
/// Requires `--source-root` / `RAILSPAN_SOURCE_ROOT`. Hardened against path traversal.
async fn get_source(
    State(state): State<AppState>,
    Query(q): Query<SourceQuery>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let Some(root) = state.source_root.as_ref() else {
        return Err(StatusCode::NOT_FOUND);
    };

    let rel = q.path.trim().trim_start_matches('/');
    if rel.is_empty() || rel.contains('\0') {
        return Err(StatusCode::BAD_REQUEST);
    }
    // Reject parent traversal in the logical path before join.
    if rel.split('/').any(|p| p == "..") {
        return Err(StatusCode::BAD_REQUEST);
    }

    let ext_ok = matches!(
        std::path::Path::new(rel)
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or(""),
        "rb" | "rake" | "arb" | "ruby" | "ru" | "rabl" | "jbuilder"
    );
    if !ext_ok {
        return Err(StatusCode::BAD_REQUEST);
    }

    let candidate = root.join(rel);
    let canon = match std::fs::canonicalize(&candidate) {
        Ok(p) => p,
        Err(_) => return Err(StatusCode::NOT_FOUND),
    };
    if !canon.starts_with(root) {
        warn!(path = %canon.display(), "source path escaped root");
        return Err(StatusCode::BAD_REQUEST);
    }
    if !canon.is_file() {
        return Err(StatusCode::NOT_FOUND);
    }
    let meta = std::fs::metadata(&canon).map_err(|_| StatusCode::NOT_FOUND)?;
    if meta.len() > MAX_SOURCE_BYTES {
        return Err(StatusCode::PAYLOAD_TOO_LARGE);
    }

    let content = tokio::fs::read_to_string(&canon)
        .await
        .map_err(|_| StatusCode::NOT_FOUND)?;
    let all_lines: Vec<&str> = content.lines().collect();
    let total = all_lines.len();
    if total == 0 {
        return Ok(Json(serde_json::json!({
            "path": rel,
            "line": q.line,
            "start_line": 1,
            "language": "ruby",
            "lines": [],
            "source_root_configured": true,
        })));
    }

    let focus = q.line.unwrap_or(1).clamp(1, total as i64) as usize;
    let ctx = q.context.min(20) as usize;
    let start = focus.saturating_sub(1).saturating_sub(ctx);
    let end = (focus.saturating_sub(1) + ctx + 1).min(total);
    let lines: Vec<String> = all_lines[start..end]
        .iter()
        .map(|s| s.to_string())
        .collect();

    Ok(Json(serde_json::json!({
        "path": rel,
        "line": focus as i64,
        "start_line": (start + 1) as i64,
        "language": "ruby",
        "lines": lines,
        "source_root_configured": true,
    })))
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
    Ok(Json(
        serde_json::json!({ "traces": traces, "from_ns": from, "to_ns": to }),
    ))
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
            store: Arc::new(Store::open_in_memory().await.unwrap().with_n1_threshold(5)),
            api_key: Some("secret".into()),
            ui_token: Some("secret".into()),
            metrics: Arc::new(ServerMetrics {
                window_start_secs: AtomicU64::new(now_secs()),
                ..ServerMetrics::default()
            }),
            sample_rate: 1.0,
            slow_ms: 500,
            source_root: None,
        }
    }

    #[tokio::test]
    async fn healthz_ok() {
        let app = app(test_state().await);
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
    async fn api_requires_ui_token() {
        let state = test_state().await;
        let res = app(state.clone())
            .oneshot(
                Request::builder()
                    .uri("/api/v1/stats")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::UNAUTHORIZED);

        let res = app(state)
            .oneshot(
                Request::builder()
                    .uri("/api/v1/stats")
                    .header("authorization", "Bearer secret")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn rejects_oversized_batch() {
        let state = test_state().await;
        let mut spans = Vec::new();
        for i in 0..(MAX_SPANS_PER_BATCH + 1) {
            spans.push(serde_json::json!({
                "trace_id": "big",
                "span_id": format!("s{i}"),
                "name": "x",
                "start_time_unix_ns": 1,
                "end_time_unix_ns": 2
            }));
        }
        let body = serde_json::json!({
            "protocol_version": 1,
            "spans": spans
        })
        .to_string();
        let res = app(state)
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
        assert_eq!(res.status(), StatusCode::PAYLOAD_TOO_LARGE);
    }

    #[tokio::test]
    async fn ingest_returns_advice() {
        let state = test_state().await;
        let body = serde_json::json!({
            "protocol_version": 1,
            "sdk": {"name": "t", "version": "0", "language": "ruby"},
            "spans": [{
                "trace_id": "adv",
                "span_id": "r",
                "name": "http.server",
                "kind": "http.server",
                "resource": "X#y",
                "start_time_unix_ns": 1,
                "end_time_unix_ns": 2,
                "status": "ok",
                "attributes": {}
            }]
        })
        .to_string();
        let res = app(state)
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
        let bytes = axum::body::to_bytes(res.into_body(), usize::MAX)
            .await
            .unwrap();
        let json: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert!(json["advice"]["sample_rate"].as_f64().is_some());
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
                    .header("authorization", "Bearer secret")
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
                    .header("authorization", "Bearer secret")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::OK);
    }

    #[tokio::test]
    async fn source_snippet_from_root() {
        let dir = std::env::temp_dir().join(format!("railspan_src_{}", now_secs()));
        std::fs::create_dir_all(dir.join("app/controllers")).unwrap();
        let file = dir.join("app/controllers/users_controller.rb");
        std::fs::write(
            &file,
            "class UsersController\n  def with_posts\n    User.all\n  end\nend\n",
        )
        .unwrap();
        let root = std::fs::canonicalize(&dir).unwrap();

        let mut state = test_state().await;
        state.source_root = Some(root);

        let res = app(state.clone())
            .oneshot(
                Request::builder()
                    .uri("/api/v1/source?path=app/controllers/users_controller.rb&line=2&context=1")
                    .header("authorization", "Bearer secret")
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
        assert_eq!(json["line"], 2);
        assert!(json["lines"].as_array().unwrap().len() >= 2);

        // Path traversal rejected
        let res = app(state)
            .oneshot(
                Request::builder()
                    .uri("/api/v1/source?path=../../etc/passwd&line=1")
                    .header("authorization", "Bearer secret")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(res.status(), StatusCode::BAD_REQUEST);

        let _ = std::fs::remove_dir_all(&dir);
    }
}
