<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { useAppStore } from '../stores/app'
import { useApi, type SessionEntry, type AgentSummary, type Project } from '../composables/useApi'
import { useWebSocket } from '../composables/useWebSocket'
import { useTauriIpc } from '../composables/useTauriIpc'
import MetricCard from '../components/ui/MetricCard.vue'
import Badge from '../components/ui/Badge.vue'
import Icon from '../components/ui/Icon.vue'
import ConnectionIndicator from '../components/dashboard/ConnectionIndicator.vue'

const router = useRouter()
const store = useAppStore()
const api = useApi()
const ws = useWebSocket()

const sessions = ref<SessionEntry[]>([])
const agents = ref<AgentSummary[]>([])
const isLoading = ref(true)
const liveEvents = ref<number>(0)

// Goal launch state
const tauri = useTauriIpc()
const projects = ref<Project[]>([])
const goalText = ref('')
const selectedProjectId = ref('')
const isLaunching = ref(false)
const launchMessage = ref<string | null>(null)
const isTauri = ref(false)

let refreshInterval: ReturnType<typeof setInterval> | null = null

const uptimeFormatted = computed(() => {
  const s = store.uptime
  if (s.endsWith('m')) return s
  return s
})

const activeSessionsCount = computed(() =>
  sessions.value.filter(s => s.status === 'running').length
)

async function loadData() {
  isLoading.value = true
  try {
    await store.refreshAll()
    sessions.value = await api.getSessions()
    agents.value = await api.getAgents()
    projects.value = await api.getProjects()
    liveEvents.value = ws.events.value.length
    // Auto-select first project if only one
    if (projects.value.length === 1 && !selectedProjectId.value) {
      selectedProjectId.value = projects.value[0].id
    }
  } catch {
    // silent
  }
  isLoading.value = false
}

async function launchGoal() {
  if (!goalText.value.trim()) return
  isLaunching.value = true
  launchMessage.value = null
  try {
    const result = await tauri.runGoal(selectedProjectId.value, goalText.value.trim())
    launchMessage.value = `Goal launched — session ${result.session_id}`
    goalText.value = ''
    // Refresh sessions after a moment
    setTimeout(loadData, 2000)
  } catch (cause: any) {
    launchMessage.value = `Error: ${cause.message}`
  } finally {
    isLaunching.value = false
    setTimeout(() => { launchMessage.value = null }, 5000)
  }
}

// Attempt to detect Tauri mode
import('@tauri-apps/api/core').then(() => { isTauri.value = true }).catch(() => {})

function getStatusColor(status: string): 'green' | 'amber' | 'crimson' | 'gray' {
  switch (status) {
    case 'running': return 'green'
    case 'completed': return 'amber'
    case 'failed': return 'crimson'
    default: return 'gray'
  }
}

onMounted(() => {
  loadData()
  refreshInterval = setInterval(loadData, 10000)
})

onUnmounted(() => {
  if (refreshInterval) clearInterval(refreshInterval)
})
</script>

<template>
  <div class="dashboard-view">
    <!-- Header -->
    <div class="dashboard-header">
      <div>
        <h1 class="dashboard-title">Dashboard</h1>
        <p class="dashboard-subtitle">
          v{{ store.version }} &middot; Up {{ uptimeFormatted }}
        </p>
      </div>
      <div class="dashboard-header-right">
        <ConnectionIndicator class="header-indicator" />
        <div class="connection-status" :class="{ connected: ws.connected.value }">
          <Icon :name="ws.connected.value ? 'wifi' : 'wifi-off'" :size="14" />
          <span>{{ ws.connected.value ? 'Live' : 'Offline' }}</span>
        </div>
      </div>
    </div>

    <!-- Metric Cards -->
    <div class="metric-grid">
      <MetricCard
        label="Active Sessions"
        :value="activeSessionsCount"
        sub="Currently running"
        color="green"
      />
      <MetricCard
        label="Total Sessions"
        :value="sessions.length"
        sub="All time"
        color="emerald"
      />
      <MetricCard
        label="Agents"
        :value="agents.length"
        sub="Configured agents"
        color="amber"
      />
      <MetricCard
        label="Events"
        :value="liveEvents"
        sub="Since connection"
        color="crimson"
      />
    </div>

    <!-- Goal Launch -->
    <div class="section goal-launch-section">
      <div class="section-header">
        <h2 class="section-title">Run a Goal</h2>
        <span v-if="!isTauri" class="desktop-hint">
          <Icon name="terminal" :size="12" />
          Available in desktop app
        </span>
      </div>

      <div class="goal-launch-card">
        <div class="goal-input-row">
          <input
            v-model="goalText"
            class="goal-input"
            placeholder="Describe what you want to build..."
            :disabled="isLaunching"
            @keydown.enter="launchGoal"
          />
          <select
            v-model="selectedProjectId"
            class="goal-project-select"
            :disabled="isLaunching"
          >
            <option value="" disabled>Project...</option>
            <option v-for="project in projects" :key="project.id" :value="project.id">
              {{ project.name }}
            </option>
          </select>
          <button
            class="btn-launch"
            :disabled="isLaunching || !goalText.trim() || !isTauri"
            @click="launchGoal"
          >
            <Icon v-if="isLaunching" name="refresh" :size="16" class="spin" />
            <Icon v-else name="send" :size="16" />
            <span>{{ isLaunching ? 'Launching...' : 'Launch' }}</span>
          </button>
        </div>
        <p v-if="launchMessage" class="launch-message" :class="{ error: launchMessage.startsWith('Error') }">
          {{ launchMessage }}
        </p>
      </div>
    </div>

    <!-- Sessions Table -->
    <div class="section">
      <div class="section-header">
        <h2 class="section-title">Active Sessions</h2>
        <button class="btn btn-ghost btn-sm" @click="router.push('/sessions')">
          View all
          <Icon name="chevron-right" :size="14" />
        </button>
      </div>

      <div v-if="sessions.length === 0" class="empty-state">
        <Icon name="server" :size="32" class="empty-icon" />
        <p>No sessions yet. Run a goal to see it here.</p>
      </div>

      <div v-else class="session-list">
        <div
          v-for="session in sessions"
          :key="session.id"
          class="session-row"
          @click="router.push(`/sessions/${session.id}`)"
        >
          <div class="session-row-info">
            <div class="session-row-title">{{ session.goal }}</div>
            <div class="session-row-meta">
              {{ session.project }} &middot; Phase {{ session.phase }}
              &middot; Iteration {{ session.iteration }}
            </div>
          </div>
          <div class="session-row-right">
            <Badge :variant="getStatusColor(session.status)">
              {{ session.status }}
            </Badge>
            <Icon name="chevron-right" :size="14" class="row-chevron" />
          </div>
        </div>
      </div>
    </div>

    <!-- Agents Grid -->
    <div class="section">
      <div class="section-header">
        <h2 class="section-title">Agent Status</h2>
      </div>

      <div v-if="agents.length === 0" class="empty-state">
        <Icon name="robot" :size="32" class="empty-icon" />
        <p>No agents configured.</p>
      </div>

      <div v-else class="agent-grid">
        <div
          v-for="agent in agents"
          :key="agent.name"
          class="agent-card"
        >
          <div class="agent-card-header">
            <div class="agent-avatar">{{ agent.name.charAt(0).toUpperCase() }}</div>
            <div class="agent-card-info">
              <div class="agent-name">{{ agent.role }}</div>
              <div class="agent-model">{{ agent.model }}</div>
            </div>
            <Badge variant="green" size="sm">Idle</Badge>
          </div>
          <div class="agent-card-tools">
            <span v-for="tool in agent.tools" :key="tool" class="agent-tool-tag">
              {{ tool }}
            </span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.dashboard-view {
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
  padding: var(--space-6);
  max-width: 1200px;
  width: 100%;
  margin: 0 auto;
}

.dashboard-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
}

.dashboard-title {
  font-size: 24px;
  font-weight: 600;
  color: var(--text-primary);
  letter-spacing: -0.02em;
}

.dashboard-subtitle {
  font-size: 13px;
  color: var(--text-muted);
  margin-top: var(--space-1);
}

.dashboard-header-right {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.connection-status {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-1) var(--space-3);
  border-radius: var(--radius-full);
  font-size: 12px;
  color: var(--text-muted);
  border: 1px solid var(--border-subtle);
}

.connection-status.connected {
  color: var(--primary);
  border-color: var(--primary-muted);
  background: var(--primary-muted);
}

.metric-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(220px, 1fr));
  gap: var(--space-4);
}

.section {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.section-title {
  font-size: 16px;
  font-weight: 600;
  color: var(--text-primary);
}

.session-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  overflow: hidden;
  background: var(--bg-surface);
}

.session-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-4);
  cursor: pointer;
  transition: background var(--transition-fast);
  border-bottom: 1px solid var(--border-subtle);
}

.session-row:last-child {
  border-bottom: none;
}

.session-row:hover {
  background: var(--bg-hover);
}

.session-row-title {
  font-size: 14px;
  font-weight: 500;
  color: var(--text-primary);
}

.session-row-meta {
  font-size: 12px;
  color: var(--text-muted);
  margin-top: var(--space-1);
}

.session-row-right {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.row-chevron {
  color: var(--text-muted);
  opacity: 0.4;
}

.agent-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(280px, 1fr));
  gap: var(--space-4);
}

.agent-card {
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  padding: var(--space-4);
  transition: border-color var(--transition-fast);
}

.agent-card:hover {
  border-color: var(--border-default);
}

.agent-card-header {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  margin-bottom: var(--space-3);
}

.agent-avatar {
  width: 36px;
  height: 36px;
  border-radius: var(--radius-md);
  display: flex;
  align-items: center;
  justify-content: center;
  font-weight: 600;
  font-size: 14px;
  background: var(--bg-elevated);
  color: var(--text-primary);
  flex-shrink: 0;
}

.agent-card-info {
  flex: 1;
}

.agent-name {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-primary);
}

.agent-model {
  font-size: 11px;
  color: var(--text-muted);
  font-family: var(--font-mono);
  margin-top: 2px;
}

.agent-card-tools {
  display: flex;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.agent-tool-tag {
  padding: 2px 8px;
  border-radius: var(--radius-full);
  font-size: 10px;
  font-weight: 500;
  background: var(--bg-elevated);
  color: var(--text-muted);
}

.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: var(--space-12) var(--space-8);
  color: var(--text-muted);
  gap: var(--space-3);
  border: 1px dashed var(--border-subtle);
  border-radius: var(--radius-lg);
}

.empty-icon {
  opacity: 0.4;
}

.btn {
  all: unset;
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  padding: var(--space-1) var(--space-3);
  border-radius: var(--radius-md);
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: all var(--transition-fast);
  font-family: inherit;
  color: var(--text-secondary);
}

.btn-sm {
  padding: var(--space-1) var(--space-2);
  font-size: 12px;
}

.btn-ghost:hover {
  color: var(--text-primary);
  background: var(--bg-hover);
}

/* ─── Goal Launch ────────────────────────────────────────────── */

.goal-launch-section {
  margin-bottom: var(--space-2);
}

.goal-launch-card {
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  padding: var(--space-4);
}

.goal-input-row {
  display: flex;
  gap: var(--space-3);
  align-items: center;
}

.goal-input {
  flex: 1;
  padding: var(--space-3) var(--space-4);
  border-radius: var(--radius-md);
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  color: var(--text-primary);
  font-size: 14px;
  font-family: inherit;
  transition: border-color var(--transition-fast);
}

.goal-input:focus {
  outline: none;
  border-color: var(--primary);
  box-shadow: 0 0 0 3px var(--primary-muted);
}

.goal-input::placeholder {
  color: var(--text-muted);
}

.goal-input:disabled {
  opacity: 0.5;
}

.goal-project-select {
  padding: var(--space-3) var(--space-3);
  border-radius: var(--radius-md);
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  color: var(--text-primary);
  font-size: 13px;
  font-family: inherit;
  min-width: 140px;
  cursor: pointer;
}

.goal-project-select:focus {
  outline: none;
  border-color: var(--primary);
}

.goal-project-select:disabled {
  opacity: 0.5;
}

.btn-launch {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-3) var(--space-5);
  border: none;
  border-radius: var(--radius-md);
  background: var(--primary);
  color: var(--bg-base);
  font-size: 14px;
  font-weight: 600;
  cursor: pointer;
  transition: background var(--transition-fast);
  white-space: nowrap;
  font-family: inherit;
}

.btn-launch:hover:not(:disabled) {
  background: var(--primary-hover);
}

.btn-launch:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.desktop-hint {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  font-size: 11px;
  color: var(--text-disabled);
  padding: var(--space-1) var(--space-2);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-full);
}

.launch-message {
  margin: var(--space-3) 0 0;
  font-size: 13px;
  color: var(--primary);
  padding: var(--space-2) var(--space-3);
  background: var(--primary-muted);
  border-radius: var(--radius-sm);
}

.launch-message.error {
  color: var(--error, #ef4444);
  background: rgba(239, 68, 68, 0.08);
}

/* Responsive: stack goal input on mobile */
@media (max-width: 767px) {
  .goal-input-row {
    flex-direction: column;
    align-items: stretch;
  }

  .goal-project-select {
    min-width: auto;
  }

  .btn-launch {
    justify-content: center;
  }
}
</style>
