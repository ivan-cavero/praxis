/**
 * useApiStatus — Polls the API server health endpoint.
 *
 * Provides connection status: connected | disconnected | checking
 * Used by the sidebar indicator to show if the backend is reachable.
 */

import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useApi } from './useApi'

export type ApiStatus = 'connected' | 'disconnected' | 'checking'

const status = ref<ApiStatus>('checking')
let pollInterval: ReturnType<typeof setInterval> | null = null
let activeCount = 0

async function checkHealth(): Promise<void> {
  const api = useApi()
  try {
    await api.get<{ status: string }>('/health')
    status.value = 'connected'
  } catch {
    status.value = 'disconnected'
  }
}

const statusLabel = computed(() => {
  switch (status.value) {
    case 'connected': return 'Connected'
    case 'disconnected': return 'Disconnected'
    case 'checking': return 'Checking...'
  }
})

export function useApiStatus() {
  onMounted(() => {
    activeCount++
    if (pollInterval === null) {
      checkHealth()
      pollInterval = setInterval(checkHealth, 10000)
    }
  })

  onUnmounted(() => {
    activeCount--
    if (activeCount <= 0 && pollInterval !== null) {
      clearInterval(pollInterval)
      pollInterval = null
    }
  })

  return { status, statusLabel, checkHealth }
}
