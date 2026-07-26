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
const jobs = ref<Endpoint[]>([])
const error = ref<string | null>(null)

async function load() {
  error.value = null
  try {
    const data = await api<{ endpoints: Endpoint[] }>(`/api/v1/endpoints?hours=${hours.value}`)
    jobs.value = (data.endpoints || []).filter((e) => e.kind === 'job')
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
    jobs.value = []
  }
}

function open(ep: Endpoint) {
  void router.push({
    name: 'resource',
    // encode so job/resource names with `#` are path-safe
    params: { resource: encodeURIComponent(ep.resource) },
    query: { hours: String(hours.value), kind: 'job' },
  })
}

onMounted(() => void load())
watch(hours, () => void load())
defineExpose({ reload: load })
</script>

<template>
  <div>
    <div v-if="error" class="error-banner">{{ error }}</div>
    <table>
      <thead>
        <tr>
          <th>Job</th>
          <th>Count</th>
          <th>Errors</th>
          <th>p95</th>
          <th>avg</th>
        </tr>
      </thead>
      <tbody>
        <tr v-if="!jobs.length">
          <td colspan="5" class="meta">No job traces yet (ActiveJob/Sidekiq)</td>
        </tr>
        <tr v-for="ep in jobs" :key="ep.resource" @click="open(ep)">
          <td>{{ ep.resource }}</td>
          <td class="num">{{ ep.count }}</td>
          <td>
            <Badge :variant="ep.error_count ? 'err' : ''">{{ ep.error_count }}</Badge>
          </td>
          <td class="num">{{ fmtMs(ep.p95_ms) }}</td>
          <td class="num">{{ fmtMs(ep.avg_ms) }}</td>
        </tr>
      </tbody>
    </table>
  </div>
</template>
