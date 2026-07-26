<script setup lang="ts">
import { ref, watch } from 'vue'
import { api, ApiError } from '@/api/client'
import type { SourceSnippet } from '@/api/types'

const props = defineProps<{
  path: string
  line?: number
  context?: number
}>()

const snippet = ref<SourceSnippet | null>(null)
const error = ref<string | null>(null)
const loading = ref(false)

async function load() {
  if (!props.path) return
  loading.value = true
  error.value = null
  snippet.value = null
  try {
    const q = new URLSearchParams({
      path: props.path,
      line: String(props.line || 1),
      context: String(props.context ?? 6),
    })
    snippet.value = await api<SourceSnippet>(`/api/v1/source?${q}`)
  } catch (e) {
    const msg = e instanceof ApiError || e instanceof Error ? e.message : String(e)
    error.value = msg
  } finally {
    loading.value = false
  }
}

watch(
  () => [props.path, props.line, props.context] as const,
  () => void load(),
  { immediate: true },
)
</script>

<template>
  <div v-if="loading" class="meta">Loading source…</div>
  <div v-else-if="error" class="code-miss">
    Code snippet unavailable ({{ error }}). Set
    <code>--source-root</code> / <code>RAILSPAN_SOURCE_ROOT</code> to your app path for highlight.
  </div>
  <div v-else-if="snippet" class="code-block">
    <table>
      <tbody>
        <tr
          v-for="(text, i) in snippet.lines"
          :key="snippet.start_line + i"
          :class="{ hl: snippet.start_line + i === snippet.line }"
        >
          <td class="ln">{{ snippet.start_line + i }}</td>
          <td>{{ text }}</td>
        </tr>
      </tbody>
    </table>
  </div>
</template>
