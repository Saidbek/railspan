//! SQLite persistence for traces and spans.

use anyhow::{Context, Result};
use railspan_protocol::{Span, TraceBatch};
use serde::Serialize;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePoolOptions};
use sqlx::{Row, SqlitePool};
use std::collections::HashMap;
use std::path::Path;
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::debug;

#[derive(Clone)]
pub struct Store {
    pool: SqlitePool,
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
pub struct TraceDetail {
    pub trace: TraceSummary,
    pub spans: Vec<SpanRow>,
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
        let store = Self { pool };
        store.migrate().await?;
        Ok(store)
    }

    pub async fn open_in_memory() -> Result<Self> {
        let pool = SqlitePoolOptions::new()
            .max_connections(1)
            .connect("sqlite::memory:")
            .await?;
        let store = Self { pool };
        store.migrate().await?;
        Ok(store)
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
                received_at_ns INTEGER NOT NULL
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
        ];
        for stmt in stmts {
            sqlx::query(stmt).execute(&self.pool).await?;
        }
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
            let root_resource = root.and_then(|s| s.resource.clone());
            let is_error = root.map(|s| s.status == "error").unwrap_or(false)
                || spans.iter().any(|s| s.status == "error");
            let status_code = root
                .and_then(|s| s.attributes.get("http.status_code"))
                .and_then(|v| v.as_i64().or_else(|| v.as_u64().map(|u| u as i64)));
            let http_method = root
                .and_then(|s| s.attributes.get("http.method"))
                .and_then(|v| v.as_str())
                .map(|s| s.to_string());

            sqlx::query(
                r#"
                INSERT INTO traces (
                    trace_id, root_resource, http_method, status_code,
                    duration_ns, service, is_error, start_time_ns, received_at_ns
                ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
                ON CONFLICT(trace_id) DO UPDATE SET
                    root_resource = excluded.root_resource,
                    http_method = excluded.http_method,
                    status_code = excluded.status_code,
                    duration_ns = excluded.duration_ns,
                    service = excluded.service,
                    is_error = excluded.is_error,
                    start_time_ns = excluded.start_time_ns,
                    received_at_ns = excluded.received_at_ns
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
            .execute(&mut *tx)
            .await?;

            for span in spans {
                let duration = span.end_time_unix_ns.saturating_sub(span.start_time_unix_ns) as i64;
                let attrs = serde_json::to_string(&span.attributes).unwrap_or_else(|_| "{}".into());
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
                .bind(&span.name)
                .bind(&span.kind)
                .bind(&span.resource)
                .bind(span.start_time_unix_ns as i64)
                .bind(duration)
                .bind(&span.status)
                .bind(attrs)
                .execute(&mut *tx)
                .await?;
                accepted += 1;
            }
        }

        tx.commit().await?;
        debug!(accepted, "persisted span batch");
        Ok(accepted)
    }

    pub async fn list_endpoints(&self, from_ns: i64, to_ns: i64) -> Result<Vec<EndpointRow>> {
        let rows = sqlx::query(
            r#"
            SELECT root_resource, duration_ns, is_error
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
        for row in rows {
            let resource: String = row.get("root_resource");
            let duration: i64 = row.get("duration_ns");
            let is_error: i64 = row.get("is_error");
            groups.entry(resource.clone()).or_default().push(duration);
            if is_error != 0 {
                *errors.entry(resource).or_default() += 1;
            }
        }

        let mut out: Vec<EndpointRow> = groups
            .into_iter()
            .map(|(resource, mut durations)| {
                let count = durations.len() as u64;
                let error_count = *errors.get(&resource).unwrap_or(&0);
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
                }
            })
            .collect();

        out.sort_by(|a, b| {
            b.p95_ms
                .partial_cmp(&a.p95_ms)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        Ok(out)
    }

    pub async fn list_traces(
        &self,
        from_ns: i64,
        to_ns: i64,
        resource: Option<&str>,
        errors_only: bool,
        min_duration_ms: Option<f64>,
        limit: i64,
    ) -> Result<Vec<TraceSummary>> {
        let min_ns = min_duration_ms.map(|ms| (ms * 1_000_000.0) as i64);

        let mut sql = String::from(
            r#"
            SELECT t.trace_id, t.root_resource, t.duration_ns, t.is_error, t.status_code,
                   t.start_time_ns, COUNT(s.span_id) as span_count
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
        if min_ns.is_some() {
            sql.push_str(" AND t.duration_ns >= ? ");
        }
        sql.push_str(" GROUP BY t.trace_id ORDER BY t.start_time_ns DESC LIMIT ? ");

        let mut q = sqlx::query(&sql).bind(from_ns).bind(to_ns);
        if let Some(r) = resource {
            q = q.bind(r);
        }
        if let Some(min) = min_ns {
            q = q.bind(min);
        }
        q = q.bind(limit);

        let rows = q.fetch_all(&self.pool).await?;
        Ok(rows
            .into_iter()
            .map(|row| {
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
                }
            })
            .collect())
    }

    pub async fn get_trace(&self, trace_id: &str) -> Result<Option<TraceDetail>> {
        let row = sqlx::query(
            r#"
            SELECT t.trace_id, t.root_resource, t.duration_ns, t.is_error, t.status_code,
                   t.start_time_ns,
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

        let duration_ns: i64 = row.get("duration_ns");
        let trace = TraceSummary {
            trace_id: row.get("trace_id"),
            root_resource: row.get("root_resource"),
            duration_ns,
            duration_ms: ns_to_ms(duration_ns),
            is_error: row.get::<i64, _>("is_error") != 0,
            status_code: row.get("status_code"),
            start_time_ns: row.get("start_time_ns"),
            span_count: row.get("span_count"),
        };

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

        Ok(Some(TraceDetail { trace, spans }))
    }

    pub async fn stats(&self) -> Result<(i64, i64)> {
        let traces: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM traces")
            .fetch_one(&self.pool)
            .await?;
        let spans: i64 = sqlx::query_scalar("SELECT COUNT(*) FROM spans")
            .fetch_one(&self.pool)
            .await?;
        Ok((traces, spans))
    }
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
            spans.iter().copied().max_by_key(|s| {
                s.end_time_unix_ns
                    .saturating_sub(s.start_time_unix_ns)
            })
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

#[cfg(test)]
mod tests {
    use super::*;
    use railspan_protocol::{SdkInfo, Span};
    use std::collections::HashMap;

    fn sample_batch() -> TraceBatch {
        TraceBatch {
            protocol_version: 1,
            sdk: SdkInfo {
                name: "test".into(),
                version: "0".into(),
                language: "ruby".into(),
                runtime: None,
            },
            resource: HashMap::from([(
                "service.name".into(),
                serde_json::Value::String("demo".into()),
            )]),
            spans: vec![
                Span {
                    trace_id: "t1".into(),
                    span_id: "s1".into(),
                    parent_span_id: None,
                    name: "http.server".into(),
                    kind: "http.server".into(),
                    resource: Some("UsersController#index".into()),
                    start_time_unix_ns: 1_000_000_000,
                    end_time_unix_ns: 1_050_000_000,
                    status: "ok".into(),
                    attributes: HashMap::from([(
                        "http.status_code".into(),
                        serde_json::json!(200),
                    )]),
                    events: vec![],
                },
                Span {
                    trace_id: "t1".into(),
                    span_id: "s2".into(),
                    parent_span_id: Some("s1".into()),
                    name: "sql".into(),
                    kind: "sql".into(),
                    resource: Some("SELECT 1".into()),
                    start_time_unix_ns: 1_010_000_000,
                    end_time_unix_ns: 1_020_000_000,
                    status: "ok".into(),
                    attributes: HashMap::new(),
                    events: vec![],
                },
            ],
        }
    }

    #[tokio::test]
    async fn ingest_and_query() {
        let store = Store::open_in_memory().await.unwrap();
        let n = store.ingest_batch(&sample_batch()).await.unwrap();
        assert_eq!(n, 2);

        let endpoints = store.list_endpoints(0, i64::MAX).await.unwrap();
        assert_eq!(endpoints.len(), 1);
        assert_eq!(endpoints[0].resource, "UsersController#index");
        assert_eq!(endpoints[0].count, 1);

        let detail = store.get_trace("t1").await.unwrap().unwrap();
        assert_eq!(detail.spans.len(), 2);
        assert_eq!(detail.trace.duration_ms, 50.0);
    }
}
