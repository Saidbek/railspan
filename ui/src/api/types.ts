export interface Endpoint {
  resource: string
  count: number
  error_count: number
  error_rate: number
  p50_ms: number
  p95_ms: number
  p99_ms: number
  avg_ms: number
  n_plus_one_count: number
  kind: string
}

export interface TraceSummary {
  trace_id: string
  root_resource: string | null
  duration_ns: number
  duration_ms: number
  is_error: boolean
  status_code: number | null
  start_time_ns: number
  span_count: number
  has_n_plus_one: boolean
  root_kind: string | null
}

export interface SpanRow {
  span_id: string
  trace_id: string
  parent_span_id: string | null
  name: string
  kind: string
  resource: string | null
  start_ns: number
  duration_ns: number
  duration_ms: number
  status: string
  attributes: Record<string, unknown>
}

export interface NPlusOneEvent {
  id: string
  trace_id: string
  root_resource: string | null
  sql_fingerprint: string
  repeat_count: number
  total_duration_ns: number
  total_duration_ms: number
  detected_at_ns: number
  code_filepath?: string | null
  code_lineno?: number | null
  code_function?: string | null
}

export interface DeployMarker {
  id: string
  git_sha: string | null
  version: string | null
  deployed_at_ns: number
  metadata: Record<string, unknown>
}

export interface TraceDetail {
  trace: TraceSummary
  spans: SpanRow[]
  n_plus_one: NPlusOneEvent[]
}

export interface Stats {
  traces: number
  spans: number
  n_plus_one_events: number
}

export interface Healthz {
  ok: boolean
  batches_received?: number
  spans_received?: number
  spans_accepted?: number
  advised_sample_rate?: number
  [key: string]: unknown
}

export interface SourceSnippet {
  path: string
  line: number
  start_line: number
  language: string
  lines: string[]
  source_root_configured?: boolean
}

export interface CodeLocation {
  path?: string
  line?: number
  fn?: string
}
