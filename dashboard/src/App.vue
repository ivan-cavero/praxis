<script setup lang="ts">
import { ref, onMounted, onUnmounted, computed } from 'vue'
import { useAppStore } from './stores/app'
import { useWebSocket } from './composables/useWebSocket'
import Card from './components/ui/Card.vue'
import Badge from './components/ui/Badge.vue'
import MetricCard from './components/ui/MetricCard.vue'
import StatusBar from './components/layout/StatusBar.vue'
import Icon from './components/ui/Icon.vue'
import LoginView from './views/LoginView.vue'
import SettingsView from './views/SettingsView.vue'

const store = useAppStore()
const ws = useWebSocket()
const currentView = ref('overview')
const sidebarCollapsed = ref(false)
const isAuthenticated = ref(false)

let refreshInterval: ReturnType<typeof setInterval> | null = null

// Check for existing token on mount
onMounted(() => {
  const token = localStorage.getItem('project-x-token')
  if (token) {
    isAuthenticated.value = true
    startApp()
  }
})

onUnmounted(() => {
  if (refreshInterval) clearInterval(refreshInterval)
})

function handleLogin(token: string) {
  isAuthenticated.value = true
  localStorage.setItem('project-x-token', token)
  startApp()
}

function handleLogout() {
  isAuthenticated.value = false
  localStorage.removeItem('project-x-token')
  if (refreshInterval) clearInterval(refreshInterval)
}

function startApp() {
  store.refreshAll()
  refreshInterval = setInterval(() => store.refreshAll(), 10000)
}

const navItems = [
  { id: 'overview', label: 'Overview', icon: 'dashboard' },
  { id: 'sessions', label: 'Sessions', icon: 'list' },
  { id: 'agents', label: 'Agents', icon: 'robot' },
  { id: 'context', label: 'Context', icon: 'brain' },
  { id: 'events', label: 'Events', icon: 'terminal' },
  { id: 'config', label: 'Settings', icon: 'settings' },
]

const recentEvents = computed(() => ws.events.value.slice().reverse().slice(0, 20))

function formatTime(iso: string) {
  const d = new Date(iso)
  return d.toLocaleTimeString('en-US', { hour12: false, hour: '2-digit', minute: '2-digit', second: '2-digit' })
}

function formatRelative(iso: string) {
  const diff = Date.now() - new Date(iso).getTime()
  if (diff < 60000) return `${Math.floor(diff / 1000)}s ago`
  if (diff < 3600000) return `${Math.floor(diff / 60000)}m ago`
  return `${Math.floor(diff / 3600000)}h ago`
}
</script>

<template>
  <!-- Login Screen -->
  <LoginView v-if="!isAuthenticated" @login="handleLogin" />

  <!-- Main App -->
  <div v-else class="h-screen flex overflow-hidden scanlines noise grid-bg">

    <!-- ═══ SIDEBAR ═══ -->
    <aside
      class="flex flex-col border-r border-[var(--border-subtle)] shrink-0 transition-all duration-300 relative"
      :class="sidebarCollapsed ? 'w-16' : 'w-56'"
      style="background: linear-gradient(180deg, var(--abyss) 0%, var(--void) 100%)"
    >
      <!-- Logo -->
      <div class="h-14 flex items-center px-4 gap-3 border-b border-[var(--border-subtle)]">
        <div class="w-8 h-8 rounded-lg flex items-center justify-center text-xs font-black shrink-0"
          style="background: linear-gradient(135deg, var(--cyan-dim), var(--cyan)); color: var(--void);">
          X
        </div>
        <div v-if="!sidebarCollapsed" class="overflow-hidden">
          <div class="text-xs font-bold tracking-wide text-[var(--text-primary)]">PROJECT-X</div>
          <div class="text-[9px] font-mono text-[var(--text-ghost)] tracking-widest">NEURAL CMD</div>
        </div>
      </div>

      <!-- Navigation -->
      <nav class="flex-1 py-3 px-2 space-y-0.5 stagger">
        <button
          v-for="item in navItems"
          :key="item.id"
          @click="currentView = item.id"
          class="w-full flex items-center gap-3 px-3 py-2.5 rounded-[var(--radius-md)] text-xs transition-all duration-200 group"
          :class="currentView === item.id
            ? 'bg-[var(--cyan-ghost)] text-[var(--cyan)] border border-[rgba(0,229,255,0.1)]'
            : 'text-[var(--text-muted)] hover:text-[var(--text-secondary)] hover:bg-white/[0.02] border border-transparent'"
        >
          <Icon :name="item.icon" :size="16" class="opacity-60 group-hover:opacity-100 transition-opacity"
            :color="currentView === item.id ? 'var(--cyan)' : undefined" />
          <span v-if="!sidebarCollapsed" class="tracking-wide">{{ item.label }}</span>
        </button>
      </nav>

      <!-- Status Bar -->
      <StatusBar
        v-if="!sidebarCollapsed"
        :connected="ws.connected.value"
        :version="store.health?.version"
        :uptime="store.uptime"
        :eventCount="ws.events.value.length"
      />

      <!-- Logout -->
      <div v-if="!sidebarCollapsed" class="px-2 pb-2">
        <button
          @click="handleLogout"
          class="w-full flex items-center gap-3 px-3 py-2 rounded-[var(--radius-md)] text-[10px] text-[var(--text-ghost)] hover:text-[var(--crimson)] hover:bg-[var(--crimson-glow)] transition-all"
        >
          <Icon name="logout" :size="14" />
          <span>Disconnect</span>
        </button>
      </div>

      <!-- Collapse Toggle -->
      <button
        @click="sidebarCollapsed = !sidebarCollapsed"
        class="absolute -right-3 top-20 w-6 h-6 rounded-full bg-[var(--surface)] border border-[var(--border)] flex items-center justify-center text-[var(--text-muted)] hover:text-[var(--cyan)] hover:border-[var(--cyan)] transition-all"
      >
        <Icon :name="sidebarCollapsed ? 'chevron-right' : 'chevron-left'" :size="10" />
      </button>
    </aside>

    <!-- ═══ MAIN CONTENT ═══ -->
    <div class="flex-1 flex flex-col min-w-0">

      <!-- Header -->
      <header class="h-12 border-b border-[var(--border-subtle)] flex items-center px-6 justify-between shrink-0"
        style="background: linear-gradient(90deg, var(--abyss), var(--void))">
        <div class="flex items-center gap-3">
          <span class="text-[10px] font-mono text-[var(--text-ghost)] tracking-[0.2em] uppercase">Sector</span>
          <span class="text-xs font-semibold text-[var(--text-primary)] tracking-wide">
            {{ navItems.find(n => n.id === currentView)?.label }}
          </span>
        </div>
        <div class="flex items-center gap-4">
          <span v-if="store.loading" class="text-[10px] font-mono text-[var(--amber)] animate-pulse">SYNCING</span>
          <span v-if="store.error" class="text-[10px] font-mono text-[var(--crimson)]">ERR: {{ store.error }}</span>
          <div class="flex items-center gap-1.5">
            <Icon :name="ws.connected.value ? 'wifi' : 'wifi-off'" :size="12"
              :color="ws.connected.value ? 'var(--emerald)' : 'var(--crimson)'" />
            <span class="text-[9px] font-mono text-[var(--text-ghost)] tracking-widest">
              {{ ws.connected.value ? 'LIVE' : 'DARK' }}
            </span>
          </div>
        </div>
      </header>

      <!-- ═══ OVERVIEW ═══ -->
      <div v-if="currentView === 'overview'" class="flex-1 overflow-y-auto px-6 py-5">
        <div class="max-w-6xl space-y-5">

          <!-- Metric Cards -->
          <div class="grid grid-cols-2 lg:grid-cols-4 gap-3 stagger">
            <MetricCard
              label="System Status"
              :value="store.isHealthy ? 'ONLINE' : 'UNKNOWN'"
              icon="◉"
              :accent="store.isHealthy ? 'emerald' : 'crimson'"
              class="anim-slide-up"
            />
            <MetricCard
              label="Active Sessions"
              :value="store.sessions.length"
              icon="◈"
              accent="cyan"
              class="anim-slide-up"
            />
            <MetricCard
              label="ASI Score"
              :value="`${store.metrics?.avg_asi_score?.toFixed(0) || '100'}%`"
              icon="◎"
              :accent="(store.metrics?.avg_asi_score || 100) >= 80 ? 'emerald' : 'amber'"
              class="anim-slide-up"
            />
            <MetricCard
              label="Tokens Consumed"
              :value="(store.metrics?.total_tokens || 0).toLocaleString()"
              icon="⬡"
              accent="cyan"
              class="anim-slide-up"
            />
          </div>

          <!-- Live Event Stream -->
          <Card title="EVENT STREAM" subtitle="real-time from EventBus" glow="cyan" class="anim-slide-up" style="animation-delay: 200ms">
            <div class="max-h-72 overflow-y-auto -mx-5 -mb-5">
              <div v-if="recentEvents.length === 0" class="p-8 text-center">
                <Icon name="terminal" :size="24" class="mx-auto mb-2 text-[var(--text-ghost)]" />
                <div class="text-[var(--text-ghost)] text-xs font-mono">Awaiting signal transmission...</div>
              </div>
              <div
                v-for="(event, idx) in recentEvents"
                :key="event.id"
                class="px-5 py-2.5 border-b border-[var(--border-subtle)] flex items-center gap-3 hover:bg-white/[0.01] transition-colors"
                :style="{ animationDelay: `${idx * 30}ms` }"
              >
                <span class="text-[9px] font-mono text-[var(--text-ghost)] w-16 shrink-0">{{ formatTime(event.timestamp) }}</span>
                <span class="text-[10px] font-mono px-1.5 py-0.5 rounded bg-[var(--cyan-ghost)] text-[var(--cyan)] shrink-0">{{ event.kind }}</span>
                <span class="text-[10px] font-mono text-[var(--text-muted)] truncate">{{ event.source }}</span>
                <span class="text-[9px] font-mono text-[var(--text-ghost)] ml-auto shrink-0">{{ formatRelative(event.timestamp) }}</span>
              </div>
            </div>
          </Card>
        </div>
      </div>

      <!-- ═══ SESSIONS ═══ -->
      <div v-else-if="currentView === 'sessions'" class="flex-1 overflow-y-auto px-6 py-5">
        <div class="max-w-4xl space-y-4 stagger">
          <div class="flex items-center justify-between mb-2">
            <h2 class="text-sm font-semibold tracking-wide">ACTIVE SESSIONS</h2>
            <Badge variant="cyan" size="sm">{{ store.sessions.length }} active</Badge>
          </div>

          <div v-if="store.sessions.length === 0" class="card p-12 text-center anim-fade-in">
            <Icon name="list" :size="28" class="mx-auto mb-3 text-[var(--text-ghost)]" />
            <div class="text-xs text-[var(--text-muted)] font-mono">No active sessions</div>
            <div class="text-[10px] text-[var(--text-ghost)] mt-1 font-mono">Execute a goal via CLI to initialize</div>
          </div>

          <div
            v-for="session in store.sessions"
            :key="session.id"
            class="card card-glow p-4 anim-slide-up"
          >
            <div class="flex items-center justify-between">
              <div class="space-y-1">
                <div class="text-sm font-semibold text-[var(--text-primary)]">{{ session.goal }}</div>
                <div class="flex items-center gap-3 text-[10px] font-mono text-[var(--text-muted)]">
                  <span>Phase: <span class="text-[var(--cyan)]">{{ session.phase }}</span></span>
                  <span>Iter: <span class="text-[var(--text-secondary)]">{{ session.iteration }}</span></span>
                </div>
              </div>
              <Badge
                :variant="session.status === 'active' ? 'green' : 'gray'"
                size="sm"
                :pulse="session.status === 'active'"
              >
                {{ session.status }}
              </Badge>
            </div>
          </div>
        </div>
      </div>

      <!-- ═══ AGENTS ═══ -->
      <div v-else-if="currentView === 'agents'" class="flex-1 overflow-y-auto px-6 py-5">
        <div class="max-w-4xl space-y-4 stagger">
          <h2 class="text-sm font-semibold tracking-wide mb-2">AGENT ROLES</h2>
          <div class="grid grid-cols-2 lg:grid-cols-3 gap-3">
            <div
              v-for="(role, idx) in [
                { name: 'Architect', desc: 'System design, ADRs', icon: 'code', accent: 'cyan' },
                { name: 'Coder', desc: 'Code generation', icon: 'terminal', accent: 'cyan' },
                { name: 'Reviewer', desc: 'Code review', icon: 'eye', accent: 'emerald' },
                { name: 'Security', desc: 'Vulnerability scan', icon: 'shield', accent: 'amber' },
                { name: 'Tester', desc: 'Test generation', icon: 'check', accent: 'emerald' },
                { name: 'Researcher', desc: 'Web research', icon: 'search', accent: 'cyan' },
              ]"
              :key="role.name"
              class="card card-glow p-4 anim-slide-up"
              :style="{ animationDelay: `${idx * 60}ms` }"
            >
              <div class="flex items-start justify-between mb-3">
                <Icon :name="role.icon" :size="20" class="opacity-40" />
                <Badge variant="gray" size="sm">active</Badge>
              </div>
              <div class="text-sm font-semibold text-[var(--text-primary)] mb-1">{{ role.name }}</div>
              <div class="text-[10px] text-[var(--text-muted)] font-mono">{{ role.desc }}</div>
              <div class="mt-3 pt-3 border-t border-[var(--border-subtle)]">
                <div class="data-label mb-1">Model</div>
                <div class="text-[10px] font-mono text-[var(--cyan)]">configured in forge.toml</div>
              </div>
            </div>
          </div>
        </div>
      </div>

      <!-- ═══ CONTEXT ═══ -->
      <div v-else-if="currentView === 'context'" class="flex-1 overflow-y-auto px-6 py-5">
        <div class="max-w-4xl space-y-4 stagger">
          <h2 class="text-sm font-semibold tracking-wide mb-2">CONTEXT MANAGEMENT</h2>
          <Card title="CONTEXT BUDGET" subtitle="per-session allocation" glow="cyan" class="anim-slide-up">
            <div v-if="store.sessions.length === 0" class="text-xs text-[var(--text-muted)] font-mono py-4">
              No active sessions to display context data
            </div>
            <div v-else class="space-y-4">
              <div v-for="session in store.sessions" :key="session.id" class="space-y-2">
                <div class="flex items-center justify-between">
                  <span class="text-xs text-[var(--text-primary)]">{{ session.goal }}</span>
                  <span class="text-[10px] font-mono text-[var(--text-muted)]">Phase: {{ session.phase }}</span>
                </div>
                <div class="h-1.5 bg-[var(--surface-overlay)] rounded-full overflow-hidden">
                  <div class="h-full rounded-full transition-all duration-1000 ease-out"
                    style="background: linear-gradient(90deg, var(--cyan-dim), var(--cyan))"
                    :style="{ width: '0%' }" />
                </div>
              </div>
            </div>
          </Card>
        </div>
      </div>

      <!-- ═══ EVENTS ═══ -->
      <div v-else-if="currentView === 'events'" class="flex-1 overflow-y-auto px-6 py-5">
        <div class="max-w-4xl space-y-4 stagger">
          <div class="flex items-center justify-between mb-2">
            <h2 class="text-sm font-semibold tracking-wide">EVENT LOG</h2>
            <div class="flex items-center gap-3">
              <div class="flex items-center gap-1.5">
                <Icon :name="ws.connected.value ? 'wifi' : 'wifi-off'" :size="12"
                  :color="ws.connected.value ? 'var(--emerald)' : 'var(--crimson)'" />
                <span class="text-[9px] font-mono text-[var(--text-ghost)] tracking-widest">
                  {{ ws.connected.value ? 'STREAMING' : 'DORMANT' }}
                </span>
              </div>
              <button @click="ws.clearEvents()" class="btn btn-ghost text-[10px] px-2 py-1">Clear</button>
            </div>
          </div>

          <Card :padding="false" class="anim-slide-up">
            <div v-if="ws.events.value.length === 0" class="p-12 text-center">
              <Icon name="terminal" :size="24" class="mx-auto mb-2 text-[var(--text-ghost)]" />
              <div class="text-[var(--text-ghost)] text-xs font-mono">No events captured</div>
            </div>
            <div
              v-for="event in ws.events.value.slice().reverse()"
              :key="event.id"
              class="px-4 py-2.5 border-b border-[var(--border-subtle)] flex items-center gap-3 hover:bg-white/[0.01] transition-colors group"
            >
              <span class="text-[9px] font-mono text-[var(--text-ghost)] w-20 shrink-0 tabular-nums">{{ formatTime(event.timestamp) }}</span>
              <span class="text-[10px] font-mono px-1.5 py-0.5 rounded bg-[var(--cyan-ghost)] text-[var(--cyan)] shrink-0 min-w-[120px] text-center">{{ event.kind }}</span>
              <span class="text-[10px] font-mono text-[var(--text-muted)] truncate flex-1">{{ event.source }}</span>
              <span class="text-[9px] font-mono text-[var(--text-ghost)] shrink-0 opacity-0 group-hover:opacity-100 transition-opacity">{{ formatRelative(event.timestamp) }}</span>
            </div>
          </Card>
        </div>
      </div>

      <!-- ═══ SETTINGS ═══ -->
      <SettingsView v-if="currentView === 'config'" />

    </div>
  </div>
</template>
