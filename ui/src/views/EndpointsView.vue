<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { api } from '@/api/client'
import type { Endpoint } from '@/api/types'
import Badge from '@/components/Badge.vue'
import { useHours } from '@/composables/useHours'
import { fmtMs } from '@/utils/format'

const router = useRouter()
const { hours } = useHours()
const endpoints = ref<Endpoint[]>([])
const error = ref<string | null>(null)
const loading = ref(false)

async function load() {
  loading.value = true
  error.value = null
  try {
    const data = await api<{ endpoints: Endpoint[] }>(`/api/v1/endpoints?hours=${hours.value}`)
    endpoints.value = (data.endpoints || []).filter((e) => (e.kind || 'http.server') !== 'job')
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
    endpoints.value = []
  } finally {
    loading.value = false
  }
}

function open(ep: Endpoint) {
  void router.push({
    name: 'resource',
    // encode so Controller#action `#` is not treated as a URL fragment
    params: { resource: encodeURIComponent(ep.resource) },
    query: { hours: String(hours.value) },
  })
}

onMounted(() => void load())
watch(hours, () => void load())

defineExpose({ reload: load })
</script>

<template>
  <div>
    <div v-if="error" class="error-banner">{{ error }}</div>
    <div v-if="loading && !endpoints.length" class="meta">Loading…</div>
    <div v-else-if="!endpoints.length" class="empty">
      <strong>No traces yet</strong>
      <p>Instrument your Rails app and hit some endpoints.</p>
      <code>cargo run -p railspan-cli -- serve --source-root ./examples/dummy_rails
# Gemfile: gem "railspan", path: "…/gem/railspan"
Railspan.configure { |c| c.endpoint = "http://127.0.0.1:7421"; c.exporter = :http }</code>
    </div>
    <table v-else>
      <thead>
        <tr>
          <th>Endpoint</th>
          <th>Kind</th>
          <th>Count</th>
          <th>Errors</th>
          <th>N+1</th>
          <th>p50</th>
          <th>p95</th>
          <th>p99</th>
          <th>avg</th>
        </tr>
      </thead>
      <tbody>
        <tr v-for="ep in endpoints" :key="ep.resource" @click="open(ep)">
          <td>{{ ep.resource }}</td>
          <td><Badge>{{ ep.kind || 'http' }}</Badge></td>
          <td class="num">{{ ep.count }}</td>
          <td>
            <Badge :variant="ep.error_count ? 'err' : ''">{{ ep.error_count }}</Badge>
          </td>
          <td>
            <Badge v-if="ep.n_plus_one_count" variant="n1">{{ ep.n_plus_one_count }}</Badge>
            <span v-else>—</span>
          </td>
          <td class="num">{{ fmtMs(ep.p50_ms) }}</td>
          <td class="num">{{ fmtMs(ep.p95_ms) }}</td>
          <td class="num">{{ fmtMs(ep.p99_ms) }}</td>
          <td class="num">{{ fmtMs(ep.avg_ms) }}</td>
        </tr>
      </tbody>
    </table>
  </div>
</template>
