<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { api } from '@/api/client'
import type { SpanRow, TraceDetail } from '@/api/types'
import Badge from '@/components/Badge.vue'
import N1Box from '@/components/N1Box.vue'
import SpanDetail from '@/components/SpanDetail.vue'
import Waterfall from '@/components/Waterfall.vue'
import { useHours } from '@/composables/useHours'
import { fmtMs, fmtTime } from '@/utils/format'

const route = useRoute()
const router = useRouter()
const { hours } = useHours()

const detail = ref<TraceDetail | null>(null)
const selectedSpan = ref<SpanRow | null>(null)
const sourceOnly = ref<{ path: string; line: number } | null>(null)
const error = ref<string | null>(null)

async function load() {
  const id = String(route.params.traceId || '')
  error.value = null
  selectedSpan.value = null
  sourceOnly.value = null
  try {
    detail.value = await api<TraceDetail>(`/api/v1/traces/${id}`)
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
    detail.value = null
  }
}

function back() {
  void router.push({ path: '/n-plus-one', query: { hours: String(hours.value) } })
}

function openSource(path: string, line: number) {
  selectedSpan.value = null
  sourceOnly.value = { path, line }
}

onMounted(() => void load())
watch(() => route.params.traceId, () => void load())
</script>

<template>
  <div>
    <p><a class="back" @click.prevent="back">← Back</a></p>
    <div v-if="error" class="error-banner">{{ error }}</div>
    <div v-if="detail" class="panel">
      <h2>{{ detail.trace.root_resource || detail.trace.trace_id }}</h2>
      <div class="meta">
        <span class="num">{{ detail.trace.trace_id }}</span>
        · {{ fmtMs(detail.trace.duration_ms) }} ·
        <Badge v-if="detail.trace.is_error" variant="err">error</Badge>
        <template v-else>ok</template>
        ·
        <Badge v-if="detail.trace.has_n_plus_one" variant="n1">N+1</Badge>
        · {{ detail.trace.span_count }} spans · {{ fmtTime(detail.trace.start_time_ns) }}
      </div>
      <N1Box
        v-for="e in detail.n_plus_one"
        :key="e.id"
        :event="e"
        @source="openSource"
      />
      <div style="margin-top: 1rem">
        <Waterfall
          :spans="detail.spans"
          :selected-id="selectedSpan?.span_id"
          @select="
            (s) => {
              selectedSpan = s
              sourceOnly = null
            }
          "
        />
      </div>
      <SpanDetail :span="selectedSpan" :source-only="sourceOnly" />
    </div>
  </div>
</template>
