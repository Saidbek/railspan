<script setup lang="ts">
import { computed } from 'vue'
import type { SpanRow } from '@/api/types'
import { codeAttrs, fmtMs, formatCodeLoc, kindClass, trunc } from '@/utils/format'

const props = defineProps<{
  spans: SpanRow[]
  selectedId?: string | null
}>()

const emit = defineEmits<{ select: [span: SpanRow] }>()

const layout = computed(() => {
  if (!props.spans.length) return []
  const minStart = Math.min(...props.spans.map((s) => s.start_ns))
  const maxEnd = Math.max(...props.spans.map((s) => s.start_ns + s.duration_ns))
  const total = Math.max(maxEnd - minStart, 1)
  return props.spans.map((s) => {
    const c = codeAttrs(s)
    const loc = formatCodeLoc(c.path, c.line, null)
    return {
      span: s,
      left: ((s.start_ns - minStart) / total) * 100,
      width: Math.max((s.duration_ns / total) * 100, 0.3),
      label: s.resource || s.name,
      loc,
      kind: kindClass(s.kind),
    }
  })
})
</script>

<template>
  <div v-if="!spans.length" class="meta">No spans</div>
  <div v-else>
    <div
      v-for="row in layout"
      :key="row.span.span_id"
      class="wf-row"
      :class="{ selected: selectedId === row.span.span_id }"
      :title="row.loc ? `${row.label} — ${row.loc}` : row.label"
      @click="emit('select', row.span)"
    >
      <div class="wf-label">
        {{ trunc(row.label, 32) }}
        <div v-if="row.loc" class="code-loc">{{ trunc(row.loc, 36) }}</div>
      </div>
      <div class="num">{{ fmtMs(row.span.duration_ms) }}</div>
      <div class="wf-bar-wrap">
        <div
          class="wf-bar"
          :class="row.kind"
          :style="{ left: row.left + '%', width: row.width + '%' }"
        />
      </div>
    </div>
  </div>
</template>
