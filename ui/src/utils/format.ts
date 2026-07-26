import type { CodeLocation, SpanRow } from '@/api/types'

export function fmtMs(ms: number | null | undefined): string {
  if (ms == null || Number.isNaN(ms)) return '—'
  if (ms < 1) return `${ms.toFixed(2)} ms`
  if (ms < 100) return `${ms.toFixed(1)} ms`
  return `${Math.round(ms)} ms`
}

export function fmtTime(ns: number | null | undefined): string {
  if (!ns) return '—'
  return new Date(ns / 1e6).toLocaleString()
}

export function trunc(s: string | null | undefined, n: number): string {
  const v = String(s || '')
  return v.length > n ? `${v.slice(0, n - 1)}…` : v
}

export function formatCodeLoc(
  path?: string | null,
  line?: number | null,
  fn?: string | null,
): string {
  if (!path) return ''
  let s = path + (line ? `:${line}` : '')
  if (fn) s += ` in ${fn}`
  return s
}

export function codeAttrs(span: SpanRow): CodeLocation {
  const a = span.attributes || {}
  const path = (a['code.filepath'] ?? a.code_filepath) as string | undefined
  const lineRaw = a['code.lineno'] ?? a.code_lineno
  const fn = (a['code.function'] ?? a.code_function) as string | undefined
  const line =
    typeof lineRaw === 'number'
      ? lineRaw
      : typeof lineRaw === 'string'
        ? Number(lineRaw)
        : undefined
  return { path, line: Number.isFinite(line) ? line : undefined, fn }
}

export function kindClass(kind: string | null | undefined): string {
  const k = (kind || 'custom').replace(/[^a-z0-9.]+/gi, '')
  return `k-${k}`
}
