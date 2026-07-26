<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { api } from '@/api/client'
import type { TraceDetail, TraceSummary } from '@/api/types'
import Badge from '@/components/Badge.vue'
import N1Box from '@/components/N1Box.vue'
import SpanDetail from '@/components/SpanDetail.vue'
import Waterfall from '@/components/Waterfall.vue'
import type { SpanRow } from '@/api/types'
import { useHours } from '@/composables/useHours'
import { fmtMs, fmtTime } from '@/utils/format'

const route = useRoute()
const router = useRouter()
const { hours } = useHours()

function decodeResourceParam(raw: unknown): string {
  const s = String(raw || '')
  try {
    return decodeURIComponent(s)
  } catch {
    return s
  }
}

const resource = ref(decodeResourceParam(route.params.resource))
const kind = ref(typeof route.query.kind === 'string' ? route.query.kind : undefined)
const traces = ref<TraceSummary[]>([])
const detail = ref<TraceDetail | null>(null)
const selectedSpan = ref<SpanRow | null>(null)
const sourceOnly = ref<{ path: string; line: number } | null>(null)
const error = ref<string | null>(null)

async function loadList() {
  error.value = null
  const q = new URLSearchParams({
    hours: String(hours.value),
    resource: resource.value,
    limit: '40',
  })
  if (kind.value) q.set('kind', kind.value)
  try {
    const data = await api<{ traces: TraceSummary[] }>(`/api/v1/traces?${q}`)
    traces.value = data.traces || []
    if (traces.value.length) {
      await loadTrace(traces.value[0].trace_id)
    } else {
      detail.value = null
    }
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
    traces.value = []
  }
}

async function loadTrace(traceId: string) {
  selectedSpan.value = null
  sourceOnly.value = null
  try {
    detail.value = await api<TraceDetail>(`/api/v1/traces/${traceId}`)
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  }
}

function back() {
  void router.push({ path: kind.value === 'job' ? '/jobs' : '/', query: { hours: String(hours.value) } })
}

function openSource(path: string, line: number) {
  selectedSpan.value = null
  sourceOnly.value = { path, line }
}

watch(
  () => [route.params.resource, route.query.kind, hours.value] as const,
  () => {
    resource.value = decodeResourceParam(route.params.resource)
    kind.value = typeof route.query.kind === 'string' ? route.query.kind : undefined
    void loadList()
  },
)

onMounted(() => void loadList())
</script>

<template>
  <div>
    <p><a class="back" @click.prevent="back">← Back</a></p>
    <div v-if="error" class="error-banner">{{ error }}</div>
    <div class="panel">
      <h2>{{ detail?.trace.root_resource || resource }}</h2>
      <div v-if="detail" class="meta">
        <span class="num">{{ detail.trace.trace_id }}</span>
        · {{ fmtMs(detail.trace.duration_ms) }} ·
        <Badge v-if="detail.trace.is_error" variant="err">error</Badge>
        <template v-else>ok</template>
        ·
        <Badge v-if="detail.trace.has_n_plus_one" variant="n1">N+1</Badge>
        · {{ detail.trace.span_count }} spans · {{ fmtTime(detail.trace.start_time_ns) }}
      </div>
      <N1Box
        v-for="e in detail?.n_plus_one || []"
        :key="e.id"
        :event="e"
        @source="openSource"
      />
      <div style="margin-top: 1rem">
        <Waterfall
          v-if="detail"
          :spans="detail.spans"
          :selected-id="selectedSpan?.span_id"
          @select="
            (s) => {
              selectedSpan = s
              sourceOnly = null
            }
          "
        />
        <div v-else class="meta">No traces</div>
      </div>
      <SpanDetail :span="selectedSpan" :source-only="sourceOnly" />
    </div>

    <div class="panel">
      <h2>Traces</h2>
      <table>
        <thead>
          <tr>
            <th>ID</th>
            <th>Duration</th>
            <th>Status</th>
            <th>N+1</th>
            <th>Spans</th>
            <th>When</th>
          </tr>
        </thead>
        <tbody>
          <tr
            v-for="t in traces"
            :key="t.trace_id"
            :class="{ active: detail?.trace.trace_id === t.trace_id }"
            @click="loadTrace(t.trace_id)"
          >
            <td class="num">{{ t.trace_id.slice(0, 12) }}…</td>
            <td class="num">{{ fmtMs(t.duration_ms) }}</td>
            <td>
              <Badge v-if="t.is_error" variant="err">error</Badge>
              <Badge v-else>ok</Badge>
            </td>
            <td>
              <Badge v-if="t.has_n_plus_one" variant="n1">N+1</Badge>
              <span v-else>—</span>
            </td>
            <td class="num">{{ t.span_count }}</td>
            <td class="meta">{{ fmtTime(t.start_time_ns) }}</td>
          </tr>
        </tbody>
      </table>
    </div>
  </div>
</template>
