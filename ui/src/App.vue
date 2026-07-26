<script setup lang="ts">
import { ref } from 'vue'
import AppHeader from '@/components/AppHeader.vue'

const viewKey = ref(0)

function onRefresh() {
  viewKey.value += 1
}
</script>

<template>
  <AppHeader @refresh="onRefresh" />
  <main>
    <RouterView :key="viewKey" />
  </main>
</template>
