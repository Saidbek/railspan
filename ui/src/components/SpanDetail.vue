<script setup lang="ts">
import { computed } from 'vue'
import type { SpanRow } from '@/api/types'
import CodeSnippet from '@/components/CodeSnippet.vue'
import Badge from '@/components/Badge.vue'
import { codeAttrs, fmtMs, formatCodeLoc } from '@/utils/format'

const props = defineProps<{
  span?: SpanRow | null
  sourceOnly?: { path: string; line: number } | null
}>()

const loc = computed(() => {
  if (props.sourceOnly) return formatCodeLoc(props.sourceOnly.path, props.sourceOnly.line)
  if (!props.span) return ''
  const c = codeAttrs(props.span)
  return formatCodeLoc(c.path, c.line, c.fn)
})

const code = computed(() => {
  if (props.sourceOnly) return props.sourceOnly
  if (!props.span) return null
  const c = codeAttrs(props.span)
  if (!c.path) return null
  return { path: c.path, line: c.line || 1 }
})

const attrs = computed(() => {
  if (!props.span) return []
  return Object.entries(props.span.attributes || {})
    .filter(([k]) => !k.startsWith('code.'))
    .slice(0, 24)
})
</script>

<template>
  <div v-if="span || sourceOnly" class="span-detail">
    <template v-if="span">
      <h3>
        {{ span.kind || 'span' }} · {{ span.resource || span.name }} ·
        {{ fmtMs(span.duration_ms) }}
      </h3>
      <div class="meta">
        span <span class="num">{{ span.span_id }}</span>
        <template v-if="span.status === 'error'">
          · <Badge variant="err">error</Badge>
        </template>
      </div>
    </template>
    <template v-else>
      <h3>Source</h3>
    </template>

    <div v-if="loc" class="code-loc" style="margin-top: 0.4rem">{{ loc }}</div>
    <div v-else class="meta" style="margin-top: 0.4rem">No code location on this span</div>

    <div v-if="attrs.length" class="attr-grid">
      <template v-for="[k, v] in attrs" :key="k">
        <div class="k">{{ k }}</div>
        <div class="v">{{ String(v).slice(0, 300) }}</div>
      </template>
    </div>

    <CodeSnippet v-if="code" :path="code.path" :line="code.line" />
  </div>
</template>
