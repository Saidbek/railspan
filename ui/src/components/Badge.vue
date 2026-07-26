<script setup lang="ts">
defineProps<{ variant?: 'err' | 'n1' | 'job' | '' }>()
</script>

<template>
  <span class="badge" :class="variant || undefined"><slot /></span>
</template>
