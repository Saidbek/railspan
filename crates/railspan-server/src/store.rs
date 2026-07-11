//! SQLite persistence for traces, spans, N+1 events, and deploys.

use anyhow::{Context, Result};
use railspan_protocol::{Span, TraceBatch};
use serde::{Deserialize, Serialize};
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::debug;

const DEFAULT_N1_THRESHOLD: u32 = 5;

#[derive(Clone)]
pub struct Store {
    pool: SqlitePool,
    n1_threshold: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct EndpointRow {
    pub resource: String,
    pub count: u64,
    pub error_count: u64,
    pub error_rate: f64,
    pub p50_ms: f64,
    pub p95_ms: f64,
    pub p99_ms: f64,
    pub avg_ms: f64,
    pub n_plus_one_count: u64,
    pub kind: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct TraceSummary {
    pub trace_id: String,
    pub root_resource: Option<String>,
    pub duration_ns: i64,
    pub duration_ms: f64,
    pub is_error: bool,
    pub status_code: Option<i64>,
    pub start_time_ns: i64,
    pub span_count: i64,
    pub has_n_plus_one: bool,
    pub root_kind: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct SpanRow {
    pub span_id: String,
    pub trace_id: String,
    pub parent_span_id: Option<String>,
    pub name: String,
    pub kind: String,
    pub resource: Option<String>,
    pub start_ns: i64,
    pub duration_ns: i64,
    pub duration_ms: f64,
    pub status: String,
    pub attributes: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct NPlusOneEvent {
    pub id: String,
    pub trace_id: String,
    pub root_resource: Option<String>,
    pub sql_fingerprint: String,
    pub repeat_count: u32,
    pub total_duration_ns: i64,
    pub total_duration_ms: f64,
    pub detected_at_ns: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DeployMarker {
    pub id: String,
    pub git_sha: Option<String>,
    pub version: Option<String>,
    pub deployed_at_ns: i64,
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize)]
pub struct TraceDetail {
    pub trace: TraceSummary,
    pub spans: Vec<SpanRow>,
    pub n_plus_one: Vec<NPlusOneEvent>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CreateDeploy {
    pub git_sha: Option<String>,
    pub version: Option<String>,
    pub deployed_at_ns: Option<i64>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

impl Store {
    pub async fn open(path: impl AsRef<Path>) -> Result<Self> {
        if let Some(parent) = path.as_ref().parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("create data dir {}", parent.display()))?;
        }
        let url = format!("sqlite://{}?mode=rwc", path.as_ref().display());
        let options = SqliteConnectOptions::from_str(&url)?.create_if_missing(true);
        let pool = SqlitePoolOptions::new()
            .max_connections(5)
            .connect_with(options)
            .await
            .with_context(|| format!("open sqlite {}", path.as_ref().display()))?;
        let store = Self {
            pool,
            n1_threshold: DEFAULT_N1_THRESHOLD,
        };
        store.migrate().await?;
        Ok(store)
    }

    pub async fn open_in_memory() -> Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        let store = Self {
            pool,
            n1_threshold: DEFAULT_N1_THRESHOLD,
        };
        store.migrate().await?;
        Ok(store)
    }

    pub fn with_n1_threshold(mut self, threshold: u32) -> Self {
        self.n1_threshold = threshold.max(2);
        self
    }

    async fn migrate(&self) -> Result<()> {
        let stmts = [
            r#"CREATE TABLE IF NOT EXISTS traces (
                trace_id TEXT PRIMARY KEY,
                root_resource TEXT,
                http_method TEXT,
                status_code INTEGER,
                duration_ns INTEGER NOT NULL,
                service TEXT,
                is_error INTEGER NOT NULL DEFAULT 0,
                start_time_ns INTEGER NOT NULL,
                received_at_ns INTEGER NOT NULL,
                has_n_plus_one INTEGER NOT NULL DEFAULT 0,
                root_kind TEXT
            )"#,
            r#"CREATE INDEX IF NOT EXISTS idx_traces_time ON traces(start_time_ns DESC)"#,
            r#"CREATE INDEX IF NOT EXISTS idx_traces_resource ON traces(root_resource, start_time_ns DESC)"#,
            r#"CREATE TABLE IF NOT EXISTS spans (
                span_id TEXT PRIMARY KEY,
                trace_id TEXT NOT NULL,
                parent_span_id TEXT,
                name TEXT NOT NULL,
                kind TEXT NOT NULL,
                resource TEXT,
                start_ns INTEGER NOT NULL,
                duration_ns INTEGER NOT NULL,
                status TEXT,
                attributes_json TEXT,
                FOREIGN KEY (trace_id) REFERENCES traces(trace_id) ON DELETE CASCADE
            )"#,
            r#"CREATE INDEX IF NOT EXISTS idx_spans_trace ON spans(trace_id)"#,
            r#"CREATE TABLE IF NOT EXISTS n_plus_one_events (
                id TEXT PRIMARY KEY,
                project_id TEXT,
                trace_id TEXT NOT NULL,
                root_resource TEXT,
                sql_fingerprint TEXT NOT NULL,
                repeat_count INTEGER NOT NULL,
                total_duration_ns INTEGER NOT NULL,
                detected_at_ns INTEGER NOT NULL
            )"#,
            r#"CREATE INDEX IF NOT EXISTS idx_n1_time ON n_plus_one_events(detected_at_ns DESC)"#,
            r#"CREATE INDEX IF NOT EXISTS idx_n1_trace ON n_plus_one_events(trace_id)"#,
            r#"CREATE TABLE IF NOT EXISTS deploy_markers (
                id TEXT PRIMARY KEY,
                git_sha TEXT,
                version TEXT,
                deployed_at_ns INTEGER NOT NULL,
                metadata_json TEXT
            )"#,
            r#"CREATE INDEX IF NOT EXISTS idx_deploys_time ON deploy_markers(deployed_at_ns DESC)"#,
        ];
        for stmt in stmts {
            sqlx::query(stmt).execute(&self.pool).await?;
        }
        // Best-effort column adds for upgrades
        let _ = sqlx::query("ALTER TABLE traces ADD COLUMN has_n_plus_one INTEGER NOT NULL DEFAULT 0")
            .execute(&self.pool)
            .await;
        let _ = sqlx::query("ALTER TABLE traces ADD COLUMN root_kind TEXT")
            .execute(&self.pool)
            .await;
        Ok(())
    }

    pub async fn ingest_batch(&self, batch: &TraceBatch) -> Result<usize> {
        if batch.spans.is_empty() {
            return Ok(0);
        }

        let received_at = now_ns() as i64;
        let service = batch
            .resource
            .get("service.name")
            .and_then(|v| v.as_str())
            .map(|s| s.to_string());

        let mut by_trace: HashMap<String, Vec<&Span>> = HashMap::new();
        for span in &batch.spans {
            // Cardinality guard: skip absurd attribute blobs later; cap resource length
            by_trace
                .entry(span.trace_id.clone())
                .or_default()
                .push(span);
        }

        let mut tx = self.pool.begin().await?;
        let mut accepted = 0usize;

        for (trace_id, spans) in by_trace {
            let root = select_root(&spans);
            let duration_ns = root
                .map(|s| s.end_time_unix_ns.saturating_sub(s.start_time_unix_ns) as i64)
                .unwrap_or(0);
            let start_time_ns = root
                .map(|s| s.start_time_unix_ns as i64)
                .or_else(|| spans.iter().map(|s| s.start_time_unix_ns as i64).min())
                .unwrap_or(received_at);
            let root_resource = root
                .and_then(|s| s.resource.clone())
                .map(|r| truncate(&r, 512));
            let root_kind = root.map(|s| s.kind.clone());
            let is_error = root.map(|s| s.status == "error").unwrap_or(false)
                || spans.iter().any(|s| s.status == "error");
            let status_code = root
                .and_then(|s| s.attributes.get("http.status_code"))
                .and_then(|v| v.as_i64().or_else(|| v.as_u64().map(|u| u as i64)));
            let http_method = root
                .and_then(|s| s.attributes.get("http.method"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            // Detect N+1 from sql spans in this batch + would ideally merge with stored;
            // for MVP detect within the batch's sql spans for this trace.
            let n1_events = detect_n_plus_one(&trace_id, &root_resource, &spans, self.n1_threshold, received_at);
            let has_n1 = !n1_events.is_empty();

            sqlx::query(
                r#"
                INSERT INTO traces (
                    trace_id, root_resource, http_method, status_code,
                    duration_ns, service, is_error, start_time_ns, received_at_ns,
                    has_n_plus_one, root_kind
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(trace_id) DO UPDATE SET
                    root_resource = excluded.root_resource,
                    http_method = excluded.http_method,
                    status_code = excluded.status_code,
                    duration_ns = excluded.duration_ns,
                    service = excluded.service,
                    is_error = excluded.is_error,
                    start_time_ns = excluded.start_time_ns,
                    received_at_ns = excluded.received_at_ns,
                    has_n_plus_one = CASE WHEN excluded.has_n_plus_one = 1 THEN 1 ELSE traces.has_n_plus_one END,
                    root_kind = excluded.root_kind
                "#,
            )
            .bind(&trace_id)
            .bind(&root_resource)
            .bind(&http_method)
            .bind(status_code)
            .bind(duration_ns)
            .bind(&service)
            .bind(if is_error { 1 } else { 0 })
            .bind(start_time_ns)
            .bind(received_at)
            .bind(if has_n1 { 1 } else { 0 })
            .bind(&root_kind)
            .execute(&mut *tx)
            .await?;

            for span in spans {
                let duration = span.end_time_unix_ns.saturating_sub(span.start_time_unix_ns) as i64;
                let resource = span.resource.as_ref().map(|r| truncate(r, 1024));
                let attrs = serde_json::to_string(&span.attributes).unwrap_or_else(|_| "{}".into());
                let attrs = truncate(&attrs, 16_384);
                sqlx::query(
                    r#"
                    INSERT INTO spans (
                        span_id, trace_id, parent_span_id, name, kind, resource,
                        start_ns, duration_ns, status, attributes_json
                    ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
                    ON CONFLICT(span_id) DO UPDATE SET
                        parent_span_id = excluded.parent_span_id,
                        name = excluded.name,
                        kind = excluded.kind,
                        resource = excluded.resource,
                        start_ns = excluded.start_ns,
                        duration_ns = excluded.duration_ns,
                        status = excluded.status,
                        attributes_json = excluded.attributes_json
                    "#,
                )
                .bind(&span.span_id)
                .bind(&span.trace_id)
                .bind(&span.parent_span_id)
                .bind(truncate(&span.name, 256))
                .bind(truncate(&span.kind, 64))
                .bind(resource)
                .bind(span.start_time_unix_ns as i64)
                .bind(duration)
                .bind(&span.status)
                .bind(attrs)
                .execute(&mut *tx)
                .await?;
                accepted += 1;
            }

            for ev in n1_events {
                sqlx::query(
                    r#"
                    INSERT INTO n_plus_one_events (
                        id, trace_id, root_resource, sql_fingerprint,
                        repeat_count, total_duration_ns, detected_at_ns
                    ) VALUES (?, ?, ?, ?, ?, ?, ?)
                    ON CONFLICT(id) DO NOTHING
                    "#,
                )
                .bind(&ev.id)
                .bind(&ev.trace_id)
                .bind(&ev.root_resource)
                .bind(&ev.sql_fingerprint)
                .bind(ev.repeat_count as i64)
                .bind(ev.total_duration_ns)
                .bind(ev.detected_at_ns)
                .execute(&mut *tx)
                .await?;
            }
        }

        tx.commit().await?;
        debug!(accepted, "persisted span batch");
        Ok(accepted)
    }

    pub async fn list_endpoints(&self, from_ns: i64, to_ns: i64) -> Result<Vec<EndpointRow>> {
        let rows = sqlx::query(
            r#"
            SELECT root_resource, duration_ns, is_error, has_n_plus_one, root_kind
            FROM traces
            WHERE start_time_ns >= ? AND start_time_ns <= ?
              AND root_resource IS NOT NULL
            "#,
        )
        .bind(from_ns)
        .bind(to_ns)
        .fetch_all(&self.pool)
        .await?;

        let mut groups: HashMap<String, Vec<i64>> = HashMap::new();
        let mut errors: HashMap<String, u64> = HashMap::new();
        let mut n1s: HashMap<String, u64> = HashMap::new();
        let mut kinds: HashMap<String, String> = HashMap::new();
        for row in rows {
            let resource: String = row.get("root_resource");
            let duration: i64 = row.get("duration_ns");
            let is_error: i64 = row.get("is_error");
            let has_n1: i64 = row.try_get("has_n_plus_one").unwrap_or(0);
            let kind: Option<String> = row.try_get("root_kind").ok();
            groups.entry(resource.clone()).or_default().push(duration);
            if is_error != 0 {
                *errors.entry(resource.clone()).or_default() += 1;
            }
            if has_n1 != 0 {
                *n1s.entry(resource.clone()).or_default() += 1;
            }
            if let Some(k) = kind {
                kinds.entry(resource).or_insert(k);
            }
        }

        let mut out: Vec<EndpointRow> = groups
            .into_iter()
            .map(|(resource, mut durations)| {
                let count = durations.len() as u64;
                let error_count = *errors.get(&resource).unwrap_or(&0);
                let n_plus_one_count = *n1s.get(&resource).unwrap_or(&0);
                let kind = kinds
                    .get(&resource)
                    .cloned()
                    .unwrap_or_else(|| "http.server".into());
                durations.sort_unstable();
                let avg = if count == 0 {
                    0.0
                } else {
                    durations.iter().sum::<i64>() as f64 / count as f64
                };
                EndpointRow {
                    resource,
                    count,
                    error_count,
                    error_rate: if count == 0 {
                        0.0
                    } else {
                        error_count as f64 / count as f64
                    },
                    p50_ms: ns_to_ms(percentile(&durations, 0.50)),
                    p95_ms: ns_to_ms(percentile(&durations, 0.95)),
                    p99_ms: ns_to_ms(percentile(&durations, 0.99)),
                    avg_ms: ns_to_ms(avg as i64),
                    n_plus_one_count,
                    kind,
                }
            })
            .collect();

        out.sort_by(|a, b| {
            b.p95_ms
                .partial_cmp(&a.p95_ms)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        // fix Equal
        Ok(out)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn list_traces(
        &self,
        from_ns: i64,
        to_ns: i64,
        resource: Option<&str>,
        errors_only: bool,
        n1_only: bool,
        min_duration_ms: Option<f64>,
        kind: Option<&str>,
        limit: i64,
    ) -> Result<Vec<TraceSummary>> {
        let min_ns = min_duration_ms.map(|ms| (ms * 1_000_000.0) as i64);

        let mut sql = String::from(
            r#"
            SELECT t.trace_id, t.root_resource, t.duration_ns, t.is_error, t.status_code,
                   t.start_time_ns, t.has_n_plus_one, t.root_kind,
                   COUNT(s.span_id) as span_count
            FROM traces t
            LEFT JOIN spans s ON s.trace_id = t.trace_id
            WHERE t.start_time_ns >= ? AND t.start_time_ns <= ?
            "#,
        );
        if resource.is_some() {
            sql.push_str(" AND t.root_resource = ? ");
        }
        if errors_only {
            sql.push_str(" AND t.is_error = 1 ");
        }
        if n1_only {
            sql.push_str(" AND t.has_n_plus_one = 1 ");
        }
        if min_ns.is_some() {
            sql.push_str(" AND t.duration_ns >= ? ");
        }
        if kind.is_some() {
            sql.push_str(" AND t.root_kind = ? ");
        }
        sql.push_str(" GROUP BY t.trace_id ORDER BY t.start_time_ns DESC LIMIT ? ");

        let mut q = sqlx::query(&sql).bind(from_ns).bind(to_ns);
        if let Some(r) = resource {
            q = q.bind(r);
        }
        if let Some(min) = min_ns {
            q = q.bind(min);
        }
        if let Some(k) = kind {
            q = q.bind(k);
        }
        q = q.bind(limit);

        let rows = q.fetch_all(&self.pool).await?;
        Ok(rows.into_iter().map(map_trace_summary).collect())
    }

    pub async fn get_trace(&self, trace_id: &str) -> Result<Option<TraceDetail>> {
        let row = sqlx::query(
            r#"
            SELECT t.trace_id, t.root_resource, t.duration_ns, t.is_error, t.status_code,
                   t.start_time_ns, t.has_n_plus_one, t.root_kind,
                   (SELECT COUNT(*) FROM spans s WHERE s.trace_id = t.trace_id) as span_count
            FROM traces t
            WHERE t.trace_id = ?
            "#,
        )
        .bind(trace_id)
        .fetch_optional(&self.pool)
        .await?;

        let Some(row) = row else {
            return Ok(None);
        };
        let trace = map_trace_summary(row);

        let span_rows = sqlx::query(
            r#"
            SELECT span_id, trace_id, parent_span_id, name, kind, resource,
                   start_ns, duration_ns, status, attributes_json
            FROM spans
            WHERE trace_id = ?
            ORDER BY start_ns ASC
            "#,
        )
        .bind(trace_id)
        .fetch_all(&self.pool)
        .await?;

        let spans = span_rows
            .into_iter()
            .map(|row| {
                let duration_ns: i64 = row.get("duration_ns");
                let attrs_raw: String = row.get("attributes_json");
                let attributes =
                    serde_json::from_str(&attrs_raw).unwrap_or_else(|_| serde_json::json!({}));
                SpanRow {
                    span_id: row.get("span_id"),
                    trace_id: row.get("trace_id"),
                    parent_span_id: row.get("parent_span_id"),
                    name: row.get("name"),
                    kind: row.get("kind"),
                    resource: row.get("resource"),
                    start_ns: row.get("start_ns"),
                    duration_ns,
                    duration_ms: ns_to_ms(duration_ns),
                    status: row.get("status"),
                    attributes,
                }
            })
            .collect();

        let n1 = self.list_n_plus_one_for_trace(trace_id).await?;
        Ok(Some(TraceDetail {
            trace,
            spans,
            n_plus_one: n1,
        }))
    }

    async fn list_n_plus_one_for_trace(&self, trace_id: &str) -> Result<Vec<NPlusOneEvent>> {
        let rows = sqlx::query(
            r#"
            SELECT id, trace_id, root_resource, sql_fingerprint, repeat_count,
                   total_duration_ns, detected_at_ns
            FROM n_plus_one_events
            WHERE trace_id = ?
            ORDER BY repeat_count DESC
            "#,
        )
        .bind(trace_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(map_n1).collect())
    }

    pub async fn list_n_plus_one(&self, from_ns: i64, to_ns: i64, limit: i64) -> Result<Vec<NPlusOneEvent>> {
        let rows = sqlx::query(
            r#"
            SELECT id, trace_id, root_resource, sql_fingerprint, repeat_count,
                   total_duration_ns, detected_at_ns
            FROM n_plus_one_events
            WHERE detected_at_ns >= ? AND detected_at_ns <= ?
            ORDER BY detected_at_ns DESC
            LIMIT ?
            "#,
        )
        .bind(from_ns)
        .bind(to_ns)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows.into_iter().map(map_n1).collect())
    }

    pub async fn create_deploy(&self, req: CreateDeploy) -> Result<DeployMarker> {
        let id = format!("dep_{}", hex_id());
        let deployed_at = req.deployed_at_ns.unwrap_or(now_ns() as i64);
        let metadata = if req.metadata.is_null() {
            serde_json::json!({})
        } else {
            req.metadata
        };
        let meta_str = serde_json::to_string(&metadata).unwrap_or_else(|_| "{}".into());
        sqlx::query(
            r#"
            INSERT INTO deploy_markers (id, git_sha, version, deployed_at_ns, metadata_json)
            VALUES (?, ?, ?, ?, ?)
            "#,
        )
        .bind(&id)
        .bind(&req.git_sha)
        .bind(&req.version)
        .bind(deployed_at)
        .bind(meta_str)
        .execute(&self.pool)
        .await?;
        Ok(DeployMarker {
            id,
            git_sha: req.git_sha,
            version: req.version,
            deployed_at_ns: deployed_at,
            metadata,
        })
    }

    pub async fn list_deploys(&self, from_ns: i64, to_ns: i64, limit: i64) -> Result<Vec<DeployMarker>> {
        let rows = sqlx::query(
            r#"
            SELECT id, git_sha, version, deployed_at_ns, metadata_json
            FROM deploy_markers
            WHERE deployed_at_ns >= ? AND deployed_at_ns <= ?
            ORDER BY deployed_at_ns DESC
            LIMIT ?
            "#,
        )
        .bind(from_ns)
        .bind(to_ns)
        .bind(limit)
        .fetch_all(&self.pool)
        .await?;
        Ok(rows
            .into_iter()
            .map(|row| {
                let meta: String = row.get("metadata_json");
                DeployMarker {
                    id: row.get("id"),
                    git_sha: row.get("git_sha"),
                    version: row.get("version"),
                    deployed_at_ns: row.get("deployed_at_ns"),
                    metadata: serde_json::from_str(&meta).unwrap_or_else(|_| serde_json::json!({})),
                }
            })
            .collect())
    }

    pub async fn retain(&self, traces_older_than_ns: i64) -> Result<(u64, u64)> {
        let del_spans = sqlx::query(
            r#"
            DELETE FROM spans WHERE trace_id IN (
              SELECT trace_id FROM traces WHERE start_time_ns < ?
            )
            "#,
        )
        .bind(traces_older_than_ns)
        .execute(&self.pool)
        .await?
        .rows_affected();

        let del_n1 = sqlx::query("DELETE FROM n_plus_one_events WHERE detected_at_ns < ?")
            .bind(traces_older_than_ns)
            .execute(&self.pool)
            .await?
            .rows_affected();

        let del_traces = sqlx::query("DELETE FROM traces WHERE start_time_ns < ?")
            .bind(traces_older_than_ns)
            .execute(&self.pool)
            .await?
            .rows_affected();

        // deploys keep longer; prune very old
        let _ = sqlx::query("DELETE FROM deploy_markers WHERE deployed_at_ns < ?")
            .bind(traces_older_than_ns.saturating_sub(90 * 24 * 3600 * 1_000_000_000))
            .execute(&self.pool)
            .await;

        Ok((del_traces + del_n1, del_spans))
    }

    pub async fn stats(&self) -> Result<(i64, i64, i64)> {
        let traces: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM traces")
            .fetch_one(&self.pool)
            .await?;
        let spans: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM spans")
            .fetch_one(&self.pool)
            .await?;
        let n1: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM n_plus_one_events")
            .fetch_one(&self.pool)
            .await?;
        Ok((traces, spans, n1))
    }
}

fn map_trace_summary(row: sqlx::sqlite::SqliteRow) -> TraceSummary {
    let duration_ns: i64 = row.get("duration_ns");
    TraceSummary {
        trace_id: row.get("trace_id"),
        root_resource: row.get("root_resource"),
        duration_ns,
        duration_ms: ns_to_ms(duration_ns),
        is_error: row.get::<i64, _>("is_error") != 0,
        status_code: row.get("status_code"),
        start_time_ns: row.get("start_time_ns"),
        span_count: row.get("span_count"),
        has_n_plus_one: row.try_get::<i64, _>("has_n_plus_one").unwrap_or(0) != 0,
        root_kind: row.try_get("root_kind").ok(),
    }
}

fn map_n1(row: sqlx::sqlite::SqliteRow) -> NPlusOneEvent {
    let total_duration_ns: i64 = row.get("total_duration_ns");
    NPlusOneEvent {
        id: row.get("id"),
        trace_id: row.get("trace_id"),
        root_resource: row.get("root_resource"),
        sql_fingerprint: row.get("sql_fingerprint"),
        repeat_count: row.get::<i64, _>("repeat_count") as u32,
        total_duration_ns,
        total_duration_ms: ns_to_ms(total_duration_ns),
        detected_at_ns: row.get("detected_at_ns"),
    }
}

fn detect_n_plus_one(
    trace_id: &str,
    root_resource: &Option<String>,
    spans: &[&Span],
    threshold: u32,
    detected_at: i64,
) -> Vec<NPlusOneEvent> {
    let mut by_fp: HashMap<String, (u32, i64)> = HashMap::new();
    for s in spans {
        if s.kind != "sql" {
            continue;
        }
        let fp = s
            .resource
            .clone()
            .or_else(|| {
                s.attributes
                    .get("db.statement")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string())
            })
            .unwrap_or_else(|| s.name.clone());
        let dur = s.end_time_unix_ns.saturating_sub(s.start_time_unix_ns) as i64;
        let e = by_fp.entry(fp).or_insert((0, 0));
        e.0 += 1;
        e.1 += dur;
    }
    by_fp
        .into_iter()
        .filter(|(_, (c, _))| *c >= threshold)
        .map(|(fp, (count, total))| NPlusOneEvent {
            id: format!("n1_{}_{}", &trace_id[..trace_id.len().min(12)], simple_hash(&fp)),
            trace_id: trace_id.to_string(),
            root_resource: root_resource.clone(),
            sql_fingerprint: truncate(&fp, 1024),
            repeat_count: count,
            total_duration_ns: total,
            total_duration_ms: ns_to_ms(total),
            detected_at_ns: detected_at,
        })
        .collect()
}

fn select_root<'a>(spans: &[&'a Span]) -> Option<&'a Span> {
    spans
        .iter()
        .copied()
        .find(|s| s.parent_span_id.is_none())
        .or_else(|| {
            spans
                .iter()
                .copied()
                .find(|s| s.kind == "http.server" || s.kind == "job")
        })
        .or_else(|| {
            spans
                .iter()
                .copied()
                .max_by_key(|s| s.end_time_unix_ns.saturating_sub(s.start_time_unix_ns))
        })
}

fn percentile(sorted: &[i64], p: f64) -> i64 {
    if sorted.is_empty() {
        return 0;
    }
    if sorted.len() == 1 {
        return sorted[0];
    }
    let idx = ((sorted.len() as f64 - 1.0) * p).round() as usize;
    sorted[idx.min(sorted.len() - 1)]
}

fn ns_to_ms(ns: i64) -> f64 {
    ns as f64 / 1_000_000.0
}

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_nanos() as u64)
        .unwrap_or(0)
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        s.chars().take(max).collect()
    }
}

fn simple_hash(s: &str) -> u64 {
    let mut h: u64 = 0xcbf29ce484222325;
    for b in s.as_bytes() {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x100000001b3);
    }
    h
}

fn hex_id() -> String {
    format!("{:016x}", now_ns())
}

#[cfg(test)]
mod tests {
    use super::*;
    use railspan_protocol::SdkInfo;
    use std::collections::HashMap;

    fn n1_batch() -> TraceBatch {
        let mut spans = vec![Span {
            trace_id: "t1".into(),
            span_id: "root".into(),
            parent_span_id: None,
            name: "http.server".into(),
            kind: "http.server".into(),
            resource: Some("UsersController#with_posts".into()),
            start_time_unix_ns: 1_000,
            end_time_unix_ns: 50_000,
            status: "ok".into(),
            attributes: HashMap::from([("http.status_code".into(), serde_json::json!(200))]),
            events: vec![],
        }];
        for i in 0..6 {
            spans.push(Span {
                trace_id: "t1".into(),
                span_id: format!("sql{i}"),
                parent_span_id: Some("root".into()),
                name: "sql".into(),
                kind: "sql".into(),
                resource: Some("SELECT posts WHERE user_id = ?".into()),
                start_time_unix_ns: 2_000 + i * 100,
                end_time_unix_ns: 2_050 + i * 100,
                status: "ok".into(),
                attributes: HashMap::new(),
                events: vec![],
            });
        }
        TraceBatch {
            protocol_version: 1,
            sdk: SdkInfo {
                name: "test".into(),
                version: "0".into(),
                language: "ruby".into(),
                runtime: None,
            },
            resource: HashMap::new(),
            spans,
        }
    }

    #[tokio::test]
    async fn detects_n_plus_one() {
        let store = Store::open_in_memory().await.unwrap().with_n1_threshold(5);
        store.ingest_batch(&n1_batch()).await.unwrap();
        let n1 = store.list_n_plus_one(0, i64::MAX, 10).await.unwrap();
        assert_eq!(n1.len(), 1);
        assert!(n1[0].repeat_count >= 5);
        let detail = store.get_trace("t1").await.unwrap().unwrap();
        assert!(detail.trace.has_n_plus_one);
        let endpoints = store.list_endpoints(0, i64::MAX).await.unwrap();
        assert_eq!(endpoints[0].n_plus_one_count, 1);
    }

    #[tokio::test]
    async fn deploy_and_retain() {
        let store = Store::open_in_memory().await.unwrap();
        store
            .create_deploy(CreateDeploy {
                git_sha: Some("abc".into()),
                version: Some("1.0".into()),
                deployed_at_ns: Some(100),
                metadata: serde_json::json!({}),
            })
            .await
            .unwrap();
        let deps = store.list_deploys(0, i64::MAX, 10).await.unwrap();
        assert_eq!(deps.len(), 1);
        store.ingest_batch(&n1_batch()).await.unwrap();
        // Delete everything older than far-future cutoff leaves recent? start times are tiny → deleted
        let (t, s) = store.retain(10_000).await.unwrap();
        assert!(t >= 1 || s >= 1);
        let (traces, spans, _) = store.stats().await.unwrap();
        assert_eq!(traces, 0);
        assert_eq!(spans, 0);
    }
}
