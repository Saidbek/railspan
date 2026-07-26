<script setup lang="ts">
import { useAuth } from '@/composables/auth'
import { useStats } from '@/composables/useStats'
import { useHours } from '@/composables/useHours'

const emit = defineEmits<{ refresh: [] }>()
const { hasToken, promptToken } = useAuth()
const { label, refresh } = useStats()
const { hours, options } = useHours()

function onAuth() {
  promptToken()
  void refresh()
  emit('refresh')
}

function onRefresh() {
  void refresh()
  emit('refresh')
}
</script>

<template>
  <header class="app-header">
    <h1>
      <span>Rail</span>span
      <small class="ui-badge" title="Vue 3 + TypeScript SPA">Vue</small>
    </h1>
    <nav>
      <RouterLink to="/">Endpoints</RouterLink>
      <RouterLink to="/jobs">Jobs</RouterLink>
      <RouterLink to="/n-plus-one">N+1</RouterLink>
      <RouterLink to="/deploys">Deploys</RouterLink>
    </nav>
    <div class="meta">{{ label }}</div>
    <div class="toolbar" style="margin: 0">
      <label class="meta">
        Range
        <select v-model.number="hours">
          <option v-for="o in options" :key="o.value" :value="o.value">{{ o.label }}</option>
        </select>
      </label>
      <button class="primary" type="button" @click="onRefresh">Refresh</button>
      <button type="button" title="UI token for /api/*" @click="onAuth">
        {{ hasToken ? 'Auth ✓' : 'Auth' }}
      </button>
    </div>
  </header>
</template>
