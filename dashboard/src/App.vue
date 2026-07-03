<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { useAppStore } from './stores/app'
import { setApiPort } from './composables/useApi'
import { useWebSocket } from './composables/useWebSocket'
import Icon from './components/ui/Icon.vue'
import LoginView from './views/LoginView.vue'

const router = useRouter()
const route = useRoute()
const store = useAppStore()
const ws = useWebSocket()

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
  } catch {
    // Not in Tauri — running in browser dev mode
  }
}

onMounted(async () => {
  await listenTauriEvents()

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
</script>

<template>
  <LoginView v-if="!isAuthenticated" @login="handleLogin" />

  <div v-else class="layout">
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
</style>
