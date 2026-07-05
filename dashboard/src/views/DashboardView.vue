<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { useAppStore } from '../stores/app'
import { useApi, type SessionEntry, type AgentDefinition, type Project } from '../composables/useApi'
import MetricCard from '../components/ui/MetricCard.vue'
import Badge from '../components/ui/Badge.vue'
import Icon from '../components/ui/Icon.vue'

const router = useRouter()
const store = useAppStore()
const api = useApi()

const sessions = ref<SessionEntry[]>([])
const agents = ref<AgentDefinition[]>([])
const isLoading = ref(true)
const metricsSummary = ref<{ total_tokens: number; avg_asi_score: number } | null>(null)

const projects = ref<Project[]>([])

// Session filter
const filterProject = ref('all')

let refreshInterval: ReturnType<typeof setInterval> | null = null

const filteredSessions = computed(() => {
  if (filterProject.value === 'all') return sessions.value
  return sessions.value.filter(s => s.project === filterProject.value)
})

const activeSessionsCount = computed(() =>
  sessions.value.filter(s => s.status === 'running').length
)

const totalTokens = computed(() =>
  sessions.value.reduce((sum, s) => sum + (s.tokens_used || 0), 0)
)

const totalCost = computed(() =>
  sessions.value.reduce((sum, s) => sum + (s.cost_usd || 0), 0)
)

const projectNames = computed(() => {
  const names = new Set(sessions.value.map(s => s.project))
  return [...names].sort()
})

async function loadData() {
  isLoading.value = true
  try {
    await store.refreshAll()
    sessions.value = await api.getSessions()
    agents.value = await api.getAgents()
    projects.value = await api.getProjects()
    try {
      metricsSummary.value = await api.getMetricsSummary()
    } catch { /* optional */ }
  } catch {
    // silent
  }
  isLoading.value = false
}

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
      <h1 class="dashboard-title">Dashboard</h1>
    </div>

    <!-- Metric Cards -->
    <div class="metric-grid">
      <MetricCard label="Active Sessions" :value="activeSessionsCount" sub="Currently running" color="green" />
      <MetricCard label="Total Sessions" :value="sessions.length" sub="All time" color="emerald" />
      <MetricCard label="Projects" :value="projects.length" sub="Created" color="blue" />
      <MetricCard label="Tokens" :value="totalTokens" sub="Total consumed" color="amber" />
      <MetricCard label="Est. Cost" :value="`$${totalCost.toFixed(2)}`" sub="All sessions" color="crimson" />
      <MetricCard label="Agents" :value="agents.length" sub="Configured" color="purple" />
    </div>



    <!-- Sessions Table -->
    <div class="section">
      <div class="section-header">
        <h2 class="section-title">Sessions</h2>
        <div class="section-header-right">
          <select v-model="filterProject" class="filter-select">
            <option value="all">All projects</option>
            <option v-for="name in projectNames" :key="name" :value="name">{{ name }}</option>
          </select>
          <button class="btn btn-ghost btn-sm" @click="router.push('/sessions')">
            View all
            <Icon name="chevron-right" :size="14" />
          </button>
        </div>
      </div>

      <div v-if="filteredSessions.length === 0" class="empty-state">
        <Icon name="server" :size="32" class="empty-icon" />
        <p>{{ sessions.length === 0 ? 'No sessions yet. Run a goal to see it here.' : 'No sessions for this project.' }}</p>
      </div>

      <div v-else class="session-list">
        <div
          v-for="session in filteredSessions"
          :key="session.id"
          class="session-row"
          @click="router.push(`/sessions/${session.id}`)"
        >
          <div class="session-row-info">
            <div class="session-row-title">{{ session.goal }}</div>
            <div class="session-row-meta">
              {{ session.project }} &middot; Phase {{ session.phase }}
              &middot; Iteration {{ session.iteration }}
              <span v-if="session.tokens_used" class="session-meta-tokens">
                &middot; {{ (session.tokens_used || 0).toLocaleString() }} tokens
                &middot; ${{ (session.cost_usd || 0).toFixed(4) }}
              </span>
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
              <div class="agent-name">{{ agent.name }}</div>
              <div class="agent-model">{{ agent.model }}</div>
            </div>
            <Badge :variant="agent.scope === 'builtin' ? 'gray' : 'emerald'" size="sm">
              {{ agent.scope }}
            </Badge>
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
  width: 100%;
  flex: 1;
  overflow-y: auto;
  min-height: 0;
}

.dashboard-header {
  margin-bottom: var(--space-6);
}

.dashboard-title {
  font-size: 24px;
  font-weight: 600;
  color: var(--text-primary);
  letter-spacing: -0.02em;
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

.section-header-right {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.filter-select {
  padding: var(--space-1) var(--space-3);
  border-radius: var(--radius-md);
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  color: var(--text-secondary);
  font-size: 12px;
  font-family: inherit;
  cursor: pointer;
  transition: all var(--transition-fast);
  appearance: none;
  background-image: url("data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' width='12' height='12' viewBox='0 0 24 24' fill='none' stroke='%2371717a' stroke-width='2' stroke-linecap='round' stroke-linejoin='round'%3E%3Cpath d='m6 9 6 6 6-6'/%3E%3C/svg%3E");
  background-repeat: no-repeat;
  background-position: right 6px center;
  padding-right: 28px;
}

.filter-select:hover {
  border-color: var(--border-default);
  color: var(--text-primary);
}

.filter-select:focus {
  outline: none;
  border-color: var(--primary);
  box-shadow: 0 0 0 2px var(--primary-muted);
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

.session-meta-tokens {
  font-family: var(--font-mono, monospace);
  color: var(--text-disabled);
  font-size: 11px;
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


</style>
