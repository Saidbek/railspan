import { computed } from 'vue'
import { useRoute, useRouter } from 'vue-router'

const OPTIONS = [
  { value: 1, label: '1h' },
  { value: 6, label: '6h' },
  { value: 24, label: '24h' },
  { value: 168, label: '7d' },
] as const

export function useHours() {
  const route = useRoute()
  const router = useRouter()

  const hours = computed({
    get() {
      const raw = Number(route.query.hours ?? 24)
      return OPTIONS.some((o) => o.value === raw) ? raw : 24
    },
    set(v: number) {
      router.replace({ query: { ...route.query, hours: String(v) } })
    },
  })

  return { hours, options: OPTIONS }
}
