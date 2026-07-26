import { onMounted, onUnmounted, ref } from 'vue'
import { api } from '@/api/client'
import type { Healthz, Stats } from '@/api/types'

export function useStats(pollMs = 5000) {
  const label = ref('Loading…')
  let timer: ReturnType<typeof setInterval> | undefined

  async function refresh() {
    try {
      const [s, h] = await Promise.all([
        api<Stats>('/api/v1/stats'),
        api<Healthz>('/healthz'),
      ])
      label.value = `${s.traces} traces · ${s.spans} spans · ${s.n_plus_one_events || 0} N+1 · batches ${h.batches_received || 0}`
    } catch {
      label.value = 'offline'
    }
  }

  onMounted(() => {
    void refresh()
    timer = setInterval(() => void refresh(), pollMs)
  })
  onUnmounted(() => {
    if (timer) clearInterval(timer)
  })

  return { label, refresh }
}
