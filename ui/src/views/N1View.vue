<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'
import { useRouter } from 'vue-router'
import { api } from '@/api/client'
import type { NPlusOneEvent } from '@/api/types'
import N1Box from '@/components/N1Box.vue'
import { useHours } from '@/composables/useHours'

const router = useRouter()
const { hours } = useHours()
const events = ref<NPlusOneEvent[]>([])
const error = ref<string | null>(null)

async function load() {
  error.value = null
  try {
    const data = await api<{ events: NPlusOneEvent[] }>(`/api/v1/n-plus-one?hours=${hours.value}`)
    events.value = data.events || []
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
    events.value = []
  }
}

function openTrace(id: string) {
  void router.push({ name: 'trace', params: { traceId: id }, query: { hours: String(hours.value) } })
}

onMounted(() => void load())
watch(hours, () => void load())
defineExpose({ reload: load })
</script>

<template>
  <div>
    <div v-if="error" class="error-banner">{{ error }}</div>
    <div v-if="!events.length" class="empty">No N+1 events in range</div>
    <N1Box
      v-for="e in events"
      :key="e.id"
      :event="e"
      show-resource
      show-time
      @open="openTrace"
    />
  </div>
</template>
