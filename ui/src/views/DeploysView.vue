<script setup lang="ts">
import { onMounted, ref, watch } from 'vue'
import { api } from '@/api/client'
import type { DeployMarker } from '@/api/types'
import { useHours } from '@/composables/useHours'
import { fmtTime } from '@/utils/format'

const { hours } = useHours()
const deploys = ref<DeployMarker[]>([])
const error = ref<string | null>(null)
const saving = ref(false)

async function load() {
  error.value = null
  try {
    const data = await api<{ deploys: DeployMarker[] }>(`/api/v1/deploys?hours=${hours.value}`)
    deploys.value = data.deploys || []
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
    deploys.value = []
  }
}

async function recordDeploy() {
  saving.value = true
  try {
    await api('/v1/deploys', {
      method: 'POST',
      headers: { 'content-type': 'application/json' },
      body: JSON.stringify({
        version: `manual-${new Date().toISOString()}`,
        git_sha: 'ui',
      }),
    })
    await load()
  } catch (e) {
    error.value = e instanceof Error ? e.message : String(e)
  } finally {
    saving.value = false
  }
}

onMounted(() => void load())
watch(hours, () => void load())
defineExpose({ reload: load })
</script>

<template>
  <div>
    <div class="toolbar">
      <button type="button" :disabled="saving" @click="recordDeploy">Record deploy (now)</button>
    </div>
    <div v-if="error" class="error-banner">{{ error }}</div>
    <div v-if="!deploys.length" class="empty">No deploy markers</div>
    <div v-for="d in deploys" :key="d.id" class="panel" style="margin-top: 0.5rem">
      <div>
        <strong>{{ d.version || d.git_sha || d.id }}</strong>
      </div>
      <div class="meta">{{ fmtTime(d.deployed_at_ns) }} · sha {{ d.git_sha || '—' }}</div>
    </div>
  </div>
</template>
