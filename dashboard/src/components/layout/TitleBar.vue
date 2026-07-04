<script setup lang="ts">
/**
 * TitleBar — Custom window chrome for Tauri.
 *
 * Provides a drag region and native window controls (minimize, maximize/restore, close).
 * Gracefully degrades in browser dev mode (no window controls).
 */
import { ref, onMounted, onUnmounted } from 'vue'
import Icon from '../ui/Icon.vue'

const isTauri = ref(false)
const isMaximized = ref(false)

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

    // Listen for resize events to update maximize button state
    const unlisten = await getCurrentWindow().onResized(async () => {
      isMaximized.value = await getCurrentWindow().isMaximized()
    })
    onUnmounted(() => { unlisten() })
  } catch {
    // Not in Tauri — running in browser dev mode
  }
})
</script>

<template>
  <header class="titlebar" data-tauri-drag-region>
    <!-- Left: App identity + drag region -->
    <div class="titlebar-left" data-tauri-drag-region>
      <span class="titlebar-logo">P</span>
      <span class="titlebar-appname">praxis</span>
    </div>

    <!-- Center: drag region (empty, just for dragging) -->
    <div class="titlebar-center" data-tauri-drag-region />

    <!-- Right: Window controls (Tauri only) -->
    <div v-if="isTauri" class="titlebar-controls">
      <button
        class="titlebar-btn titlebar-btn-minimize"
        @click="handleMinimize"
        title="Minimize"
      >
        <Icon name="minus" :size="14" />
      </button>
      <button
        class="titlebar-btn titlebar-btn-maximize"
        @click="handleMaximize"
        :title="isMaximized ? 'Restore' : 'Maximize'"
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

.titlebar-center {
  flex: 1;
  height: 100%;
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
