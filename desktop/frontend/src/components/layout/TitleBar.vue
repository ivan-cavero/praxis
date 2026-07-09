<script setup lang="ts">
/**
 * TitleBar — Custom window chrome for Tauri.
 *
 * Provides a drag region and native window controls (minimize, maximize/restore, close).
 * Gracefully degrades in browser dev mode (no window controls).
 */
import { ref, computed, onMounted, onUnmounted } from 'vue'
import Icon from '../ui/Icon.vue'
import { useAppStore } from '../../stores/app'
import { useApi } from '../../composables/useApi'
import { useApiStatus } from '../../composables/useApiStatus'

const store = useAppStore()

const isTauri = ref(false)
const isMaximized = ref(false)
const api = useApi()
const apiStatus = useApiStatus()
let unlistenResize: (() => void) | null = null

const sessions = ref<number>(0)
const agentsRunning = ref<number>(0)

const connectionLabel = computed(() => apiStatus.statusLabel.value)

onMounted(async () => {
  // Load counts periodically
  async function loadCounts() {
    try {
      const allSessions = await api.getSessions()
      sessions.value = allSessions.length
      agentsRunning.value = allSessions.filter(s => s.status === 'running').length
    } catch {
      // silent
    }
  }
  loadCounts()
  setInterval(loadCounts, 15000)

  // Tauri window controls
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window')
    isTauri.value = true
    isMaximized.value = await getCurrentWindow().isMaximized()

    unlistenResize = await getCurrentWindow().onResized(async () => {
      isMaximized.value = await getCurrentWindow().isMaximized()
    })
  } catch {
    // Not in Tauri — browser dev mode
  }
})

async function handleMinimize() {
  if (!isTauri.value) return
  const { getCurrentWindow } = await import('@tauri-apps/api/window')
  await getCurrentWindow().minimize()
}

async function handleMaximize() {
  if (!isTauri.value) return
  const { getCurrentWindow } = await import('@tauri-apps/api/window')
  await getCurrentWindow().toggleMaximize()
}

async function handleClose() {
  if (!isTauri.value) return
  const { getCurrentWindow } = await import('@tauri-apps/api/window')
  await getCurrentWindow().close()
}

onMounted(async () => {
  try {
    const { getCurrentWindow } = await import('@tauri-apps/api/window')
    isTauri.value = true
    isMaximized.value = await getCurrentWindow().isMaximized()

    unlistenResize = await getCurrentWindow().onResized(async () => {
      isMaximized.value = await getCurrentWindow().isMaximized()
    })
  } catch {
    // Not in Tauri — running in browser dev mode
  }
})

onUnmounted(() => {
  if (unlistenResize) unlistenResize()
})
</script>

<template>
  <header class="titlebar" data-tauri-drag-region>
    <!-- Left: App identity + drag region -->
    <div class="titlebar-left" data-tauri-drag-region>
      <span class="titlebar-logo">P</span>
      <span class="titlebar-appname">praxis</span>
      <span class="titlebar-version" v-if="store.version">v{{ store.version }}</span>
    </div>

    <!-- Center: connection status + live data + drag region -->
    <div class="titlebar-center" data-tauri-drag-region>
      <div class="titlebar-status">
        <span class="tb-status-dot" :class="{ online: apiStatus.status.value === 'connected' }" />
        <span class="tb-status-label">{{ connectionLabel }}</span>
        <span class="tb-separator">|</span>
        <span class="tb-metric">
          <Icon name="server" :size="12" />
          {{ sessions }} sessions
        </span>
        <template v-if="agentsRunning > 0">
          <span class="tb-separator">|</span>
          <span class="tb-metric tb-metric-running">
            <Icon name="refresh" :size="12" class="tb-spin" />
            {{ agentsRunning }} running
          </span>
        </template>
      </div>
    </div>

    <!-- Right: Window controls (Tauri only) -->
    <div v-if="isTauri" class="titlebar-controls">
      <button
        class="titlebar-btn titlebar-btn-minimize"
        @click="handleMinimize"
        title="Minimize"
        aria-label="Minimize"
      >
        <Icon name="minus" :size="14" />
      </button>
      <button
        class="titlebar-btn titlebar-btn-maximize"
        @click="handleMaximize"
        :title="isMaximized ? 'Restore' : 'Maximize'"
        :aria-label="isMaximized ? 'Restore' : 'Maximize'"
      >
        <template v-if="isMaximized">
          <!-- Restore icon: two overlapping squares -->
          <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="square">
            <rect x="1" y="5" width="8" height="8" />
            <rect x="5" y="1" width="8" height="8" />
          </svg>
        </template>
        <template v-else>
          <!-- Maximize icon: single square -->
          <svg width="14" height="14" viewBox="0 0 14 14" fill="none" stroke="currentColor" stroke-width="1.5" stroke-linecap="square">
            <rect x="2" y="2" width="10" height="10" />
          </svg>
        </template>
      </button>
      <button
        class="titlebar-btn titlebar-btn-close"
        @click="handleClose"
        title="Close"
        aria-label="Close"
      >
        <Icon name="x" :size="14" />
      </button>
    </div>
  </header>
</template>

<style scoped>
.titlebar {
  display: flex;
  align-items: center;
  height: var(--titlebar-height, 38px);
  min-height: var(--titlebar-height, 38px);
  background: var(--bg-surface);
  border-bottom: 1px solid var(--border-subtle);
  user-select: none;
  position: relative;
  z-index: 200;
}

.titlebar-left {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: 0 var(--space-3);
  height: 100%;
  min-width: var(--sidebar-width, 260px);
}

.titlebar-logo {
  width: 22px;
  height: 22px;
  border-radius: var(--radius-sm);
  display: flex;
  align-items: center;
  justify-content: center;
  font-weight: 700;
  font-size: 11px;
  background: var(--text-primary);
  color: var(--bg-base);
  flex-shrink: 0;
}

.titlebar-appname {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-secondary);
  letter-spacing: -0.01em;
}

.titlebar-version {
  font-size: 11px;
  color: var(--text-disabled);
  font-weight: 400;
  letter-spacing: -0.01em;
}

.titlebar-center {
  flex: 1;
  height: 100%;
  display: flex;
  align-items: center;
  justify-content: center;
}

.titlebar-status {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: 12px;
  color: var(--text-muted);
  pointer-events: none;
}

.tb-status-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--text-disabled);
  flex-shrink: 0;
}

.tb-status-dot.online {
  background: var(--primary);
  box-shadow: 0 0 6px var(--primary-glow);
}

.tb-status-label {
  font-weight: 500;
}

.tb-separator {
  color: var(--border-default);
  font-size: 10px;
}

.tb-metric {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  opacity: 0.6;
}

.tb-metric-running {
  color: var(--primary);
  opacity: 1;
}

.tb-spin {
  animation: tbSpin 2s linear infinite;
}

@keyframes tbSpin {
  to { transform: rotate(360deg); }
}

.titlebar-controls {
  display: flex;
  align-items: center;
  height: 100%;
}

.titlebar-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 46px;
  height: 100%;
  border: none;
  background: transparent;
  color: var(--text-secondary);
  cursor: pointer;
  transition: background 0.1s, color 0.1s;
  font-family: inherit;
}

.titlebar-btn:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}

.titlebar-btn-close:hover {
  background: #e81123;
  color: white;
}
</style>
