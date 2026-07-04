<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useAppStore } from './stores/app'
import { setApiPort } from './composables/useApi'
import { useWebSocket } from './composables/useWebSocket'
import { useUpdater } from './composables/useUpdater'
import Icon from './components/ui/Icon.vue'
import LoginView from './views/LoginView.vue'

const router = useRouter()
const route = useRoute()
const store = useAppStore()
const ws = useWebSocket()
const updater = useUpdater()

const isAuthenticated = ref(false)

let refreshInterval: ReturnType<typeof setInterval> | null = null

const navItems = [
  { name: 'dashboard', label: 'Dashboard', icon: 'dashboard', route: '/' },
  { name: 'pipeline', label: 'Pipeline', icon: 'chart-line', route: '/pipeline' },
  { name: 'sessions', label: 'Sessions', icon: 'server', route: '/sessions' },
  { name: 'settings', label: 'Settings', icon: 'settings', route: '/settings' },
]

const currentRouteName = computed(() => route.name as string || 'dashboard')

async function listenTauriEvents() {
  try {
    const { listen } = await import('@tauri-apps/api/event')
    await listen<number>('api:ready', (event) => {
      setApiPort(event.payload)
    })
    await listen('core:ready', () => {
      // Core runtime ready
    })
    // System tray events
    await listen('tray:new_session', () => {
      router.push('/sessions')
    })
    await listen('tray:settings', () => {
      router.push('/settings')
    })
  } catch {
    // Not in Tauri — running in browser dev mode
  }
}

onMounted(async () => {
  await listenTauriEvents()

  // Check for updates (Tauri only — silently ignored in browser)
  updater.checkForUpdates()

  const token = localStorage.getItem('praxis-token')
  if (token) {
    isAuthenticated.value = true
    store.refreshAll()
    refreshInterval = setInterval(() => store.refreshAll(), 10000)
  }
})

onUnmounted(() => {
  if (refreshInterval) clearInterval(refreshInterval)
})

function handleLogin(token: string) {
  isAuthenticated.value = true
  localStorage.setItem('praxis-token', token)
  store.refreshAll()
  refreshInterval = setInterval(() => store.refreshAll(), 10000)
}

/** Restart the app — uses Tauri relaunch in desktop, falls back to page reload. */
async function restartApp() {
  try {
    const { relaunch } = await import('@tauri-apps/plugin-process')
    await relaunch()
  } catch {
    window.location.reload()
  }
}
</script>

<template>
  <LoginView v-if="!isAuthenticated" @login="handleLogin" />

  <!-- Update available banner -->
  <div
    v-if="isAuthenticated && updater.updateAvailable.value && !updater.dismissed.value"
    class="update-banner"
  >
    <div class="update-banner-content">
      <Icon name="download" :size="16" class="update-icon" />
      <span v-if="updater.installDone.value">
        Update {{ updater.updateVersion.value }} installed — restart to apply
      </span>
      <span v-else-if="updater.installing.value">
        Installing update...
      </span>
      <span v-else-if="updater.downloading.value">
        Downloading update {{ updater.updateVersion.value }} —
        {{ updater.progressPercent() }}%
      </span>
      <span v-else>
        Update {{ updater.updateVersion.value }} available
        <span v-if="updater.updateBody.value" class="update-body">
          — {{ updater.updateBody.value.slice(0, 80) }}...
        </span>
      </span>
    </div>
    <div class="update-banner-actions">
      <button
        v-if="!updater.downloading.value && !updater.installDone.value"
        class="update-btn"
        @click="updater.installUpdate()"
      >
        Install
      </button>
      <button
        v-if="updater.installDone.value"
        class="update-btn"
        @click="restartApp()"
      >
        Restart
      </button>
      <button class="update-dismiss" @click="updater.dismissUpdate()" title="Dismiss">
        &times;
      </button>
    </div>
  </div>

  <div class="layout">
    <aside class="sidebar">
      <div class="sidebar-header">
        <div class="logo-mark">P</div>
        <div class="logo-text">
          <div class="brand">praxis</div>
        </div>
      </div>

      <div class="sidebar-nav">
        <button
          v-for="item in navItems"
          :key="item.name"
          class="nav-item"
          :class="{ active: currentRouteName === item.name }"
          @click="router.push(item.route)"
        >
          <Icon :name="item.icon" :size="18" class="nav-icon" />
          <span class="nav-label">{{ item.label }}</span>
        </button>
      </div>

      <div class="sidebar-status">
        <div class="status-item" :class="{ online: ws.connected.value }">
          <span class="status-dot" />
          <span class="status-label">{{ ws.connected.value ? 'Connected' : 'Offline' }}</span>
        </div>
      </div>

      <div class="sidebar-footer">
        <div class="sidebar-user">
          <div class="user-avatar">I</div>
          <span class="user-name">Ivan</span>
        </div>
      </div>
    </aside>

    <div class="main-content">
      <router-view />
    </div>
  </div>
</template>

<style scoped>
.sidebar-status {
  padding: var(--space-2) var(--space-3);
  margin: 0 var(--space-3);
  border-top: 1px solid var(--border-subtle);
}

.status-item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-md);
  font-size: 12px;
  color: var(--text-muted);
}

.status-item.online {
  color: var(--primary);
}

.status-item .status-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--text-disabled);
}

.status-item.online .status-dot {
  background: var(--primary);
  box-shadow: 0 0 6px var(--primary-glow);
}

/* ─── Update Banner ──────────────────────────────────────────── */

.update-banner {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  z-index: 1000;
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  padding: var(--space-2) var(--space-4);
  background: var(--primary);
  color: var(--bg-base);
  font-size: 13px;
}

.update-banner-content {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.update-icon {
  flex-shrink: 0;
  opacity: 0.8;
}

.update-body {
  opacity: 0.7;
}

.update-banner-actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-shrink: 0;
}

.update-btn {
  padding: var(--space-1) var(--space-3);
  border: 1px solid var(--bg-base);
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--bg-base);
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  transition: background 0.15s, color 0.15s;
}

.update-btn:hover {
  background: var(--bg-base);
  color: var(--primary);
}

.update-dismiss {
  padding: 0 var(--space-1);
  border: none;
  background: transparent;
  color: var(--bg-base);
  font-size: 18px;
  cursor: pointer;
  opacity: 0.6;
  line-height: 1;
}

.update-dismiss:hover {
  opacity: 1;
}
</style>
