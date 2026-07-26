<script setup lang="ts">
import { computed } from 'vue'
import type { NPlusOneEvent } from '@/api/types'
import { fmtMs, fmtTime, formatCodeLoc } from '@/utils/format'

const props = defineProps<{
  event: NPlusOneEvent
  showResource?: boolean
  showTime?: boolean
}>()

const emit = defineEmits<{
  open: [traceId: string]
  source: [path: string, line: number]
}>()

const loc = computed(() =>
  formatCodeLoc(props.event.code_filepath, props.event.code_lineno, props.event.code_function),
)
</script>

<template>
  <div class="n1-box" @click="emit('open', event.trace_id)">
    <template v-if="showResource">
      <strong>{{ event.root_resource || 'trace' }}</strong>
      ·
    </template>
    <strong v-else>N+1</strong>
    {{ event.repeat_count }}× · {{ fmtMs(event.total_duration_ms) }}
    <div class="num meta">{{ event.sql_fingerprint }}</div>
    <div
      v-if="loc"
      class="code-loc"
      @click.stop="
        event.code_filepath &&
          emit('source', event.code_filepath, event.code_lineno || 1)
      "
    >
      {{ loc }}
    </div>
    <div v-if="showTime" class="meta">
      {{ fmtTime(event.detected_at_ns) }} · {{ event.trace_id.slice(0, 12) }}…
    </div>
  </div>
</template>
