<script setup lang="ts">
/**
 * CostAnalysisView — Dedicated cost & efficiency analysis dashboard.
 *
 * Shows:
 * - Total tokens and cost across all sessions
 * - Per-session breakdown (tokens, cost, duration, $/iteration)
 * - Cost by project
 * - Efficiency metrics (tokens per iteration, cost per agent)
 * - Filtering by project and date range
 */
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useApi, type SessionEntry } from '../composables/useApi'
import Icon from '../components/ui/Icon.vue'
import EmptyState from '../components/ui/EmptyState.vue'

const api = useApi()

const sessions = ref<SessionEntry[]>([])
const isLoading = ref(true)
let refreshInterval: ReturnType<typeof setInterval> | null = null

// ─── Filters ────────────────────────────────────────────────────
type DateRange = '24h' | '7d' | '30d' | 'all'
const selectedProject = ref<string>('all')
const selectedDateRange = ref<DateRange>('all')

const projectOptions = computed(() => {
  const projects = new Set(sessions.value.map(s => s.project).filter(Boolean))
  return [...projects].sort()
})

const dateRangeMs: Record<DateRange, number> = {
  '24h': 24 * 60 * 60 * 1000,
  '7d': 7 * 24 * 60 * 60 * 1000,
  '30d': 30 * 24 * 60 * 60 * 1000,
  'all': 0,
}

const filteredSessions = computed(() => {
  const now = Date.now()
  const cutoff = selectedDateRange.value === 'all' ? 0 : now - dateRangeMs[selectedDateRange.value]
  return sessions.value.filter(s => {
    if (selectedProject.value !== 'all' && s.project !== selectedProject.value) return false
    if (cutoff > 0) {
      const sessionTime = new Date(s.started_at).getTime()
      if (sessionTime < cutoff) return false
    }
    return true
  })
})

function loadData() {
  isLoading.value = true
  api.getSessions()
    .then(data => { sessions.value = data })
    .catch(() => { /* Background polling — don't spam toasts */ })
    .finally(() => { isLoading.value = false })
}

onMounted(() => {
  loadData()
  refreshInterval = setInterval(loadData, 5000)
})

onUnmounted(() => {
  if (refreshInterval) clearInterval(refreshInterval)
})

// ─── Computed metrics (from filtered sessions) ─────────────────

const totalTokens = computed(() =>
  filteredSessions.value.reduce((sum, s) => sum + (s.tokens_used || 0), 0)
)

const totalCost = computed(() =>
  filteredSessions.value.reduce((sum, s) => sum + (s.cost_usd || 0), 0)
)

const completedSessions = computed(() =>
  filteredSessions.value.filter(s => s.status === 'completed' || s.status === 'failed')
)

const avgTokensPerSession = computed(() => {
  if (completedSessions.value.length === 0) return 0
  return Math.round(totalTokens.value / completedSessions.value.length)
})

const avgCostPerSession = computed(() => {
  if (completedSessions.value.length === 0) return 0
  return totalCost.value / completedSessions.value.length
})

const avgTokensPerIteration = computed(() => {
  const sessionsWithIterations = completedSessions.value.filter(s => s.iteration > 0)
  if (sessionsWithIterations.length === 0) return 0
  const totalIters = sessionsWithIterations.reduce((sum, s) => sum + s.iteration, 0)
  const totalToks = sessionsWithIterations.reduce((sum, s) => sum + (s.tokens_used || 0), 0)
  return totalIters > 0 ? Math.round(totalToks / totalIters) : 0
})

const avgCostPerIteration = computed(() => {
  const sessionsWithIterations = completedSessions.value.filter(s => s.iteration > 0)
  if (sessionsWithIterations.length === 0) return 0
  const totalIters = sessionsWithIterations.reduce((sum, s) => sum + s.iteration, 0)
  const totalC = sessionsWithIterations.reduce((sum, s) => sum + (s.cost_usd || 0), 0)
  return totalIters > 0 ? totalC / totalIters : 0
})

// ─── Per-project breakdown ──────────────────────────────────────

interface ProjectCost {
  project: string
  sessions: number
  tokens: number
  cost: number
  avgCostPerSession: number
}

const projectCosts = computed((): ProjectCost[] => {
  const map = new Map<string, ProjectCost>()
  for (const s of filteredSessions.value) {
    const existing = map.get(s.project) || {
      project: s.project,
      sessions: 0,
      tokens: 0,
      cost: 0,
      avgCostPerSession: 0,
    }
    existing.sessions += 1
    existing.tokens += s.tokens_used || 0
    existing.cost += s.cost_usd || 0
    map.set(s.project, existing)
  }
  const result = [...map.values()]
  for (const p of result) {
    p.avgCostPerSession = p.sessions > 0 ? p.cost / p.sessions : 0
  }
  return result.sort((a, b) => b.cost - a.cost)
})

// ─── Per-session breakdown ──────────────────────────────────────

const sessionBreakdown = computed(() => {
  return filteredSessions.value
    .map(s => ({
      id: s.id,
      project: s.project,
      goal: s.goal,
      status: s.status,
      phase: s.phase,
      iteration: s.iteration,
      tokens: s.tokens_used || 0,
      cost: s.cost_usd || 0,
      tokensPerIteration: s.iteration > 0 ? Math.round((s.tokens_used || 0) / s.iteration) : 0,
      costPerIteration: s.iteration > 0 ? (s.cost_usd || 0) / s.iteration : 0,
    }))
    .sort((a, b) => b.cost - a.cost)
})

function formatCost(cost: number): string {
  if (cost < 0.01) return `$${cost.toFixed(4)}`
  return `$${cost.toFixed(2)}`
}

function formatTokens(tokens: number): string {
  if (tokens >= 1_000_000) return `${(tokens / 1_000_000).toFixed(2)}M`
  if (tokens >= 1_000) return `${(tokens / 1_000).toFixed(1)}K`
  return tokens.toString()
}
</script>

<template>
  <div class="cost-analysis-view">
    <div class="cost-header">
      <h1 class="cost-title">Cost & Efficiency Analysis</h1>
      <div class="filter-bar">
        <select v-model="selectedProject" class="filter-select" aria-label="Filter by project">
          <option value="all">All Projects</option>
          <option v-for="p in projectOptions" :key="p" :value="p">{{ p }}</option>
        </select>
        <select v-model="selectedDateRange" class="filter-select" aria-label="Filter by date range">
          <option value="24h">Last 24h</option>
          <option value="7d">Last 7 days</option>
          <option value="30d">Last 30 days</option>
          <option value="all">All time</option>
        </select>
        <button class="refresh-btn" @click="loadData" :disabled="isLoading" aria-label="Refresh data">
          <Icon v-if="isLoading" name="refresh" :size="14" class="animate-spin" />
          <Icon v-else name="refresh" :size="14" />
        </button>
      </div>
    </div>

    <!-- Summary metrics -->
    <div class="metric-grid">
      <div class="metric-card">
        <div class="metric-label">Total Tokens</div>
        <div class="metric-value">{{ formatTokens(totalTokens) }}</div>
      </div>
      <div class="metric-card">
        <div class="metric-label">Total Cost</div>
        <div class="metric-value">{{ formatCost(totalCost) }}</div>
      </div>
      <div class="metric-card">
        <div class="metric-label">Avg Tokens/Session</div>
        <div class="metric-value">{{ formatTokens(avgTokensPerSession) }}</div>
      </div>
      <div class="metric-card">
        <div class="metric-label">Avg Cost/Session</div>
        <div class="metric-value">{{ formatCost(avgCostPerSession) }}</div>
      </div>
      <div class="metric-card">
        <div class="metric-label">Avg Tokens/Iteration</div>
        <div class="metric-value">{{ formatTokens(avgTokensPerIteration) }}</div>
      </div>
      <div class="metric-card">
        <div class="metric-label">Avg Cost/Iteration</div>
        <div class="metric-value">{{ formatCost(avgCostPerIteration) }}</div>
      </div>
    </div>

    <!-- Per-project breakdown -->
    <div class="section">
      <h2 class="section-title">Cost by Project</h2>
      <EmptyState
        v-if="projectCosts.length === 0"
        icon="chart"
        title="No cost data yet"
        description="Run a goal to see cost analysis and efficiency metrics."
      />
      <div v-else class="project-table">
        <div class="table-header">
          <div>Project</div>
          <div>Sessions</div>
          <div>Tokens</div>
          <div>Cost</div>
          <div>Avg/Session</div>
        </div>
        <div v-for="p in projectCosts" :key="p.project" class="table-row">
          <div class="cell-project">{{ p.project }}</div>
          <div class="cell-number">{{ p.sessions }}</div>
          <div class="cell-number">{{ formatTokens(p.tokens) }}</div>
          <div class="cell-number">{{ formatCost(p.cost) }}</div>
          <div class="cell-number">{{ formatCost(p.avgCostPerSession) }}</div>
        </div>
      </div>
    </div>

    <!-- Per-session breakdown -->
    <div class="section">
      <h2 class="section-title">Session Breakdown</h2>
      <EmptyState
        v-if="sessionBreakdown.length === 0"
        icon="chart"
        title="No sessions yet"
        description="Completed sessions will appear here with their cost breakdown."
      />
      <div v-else class="session-table">
        <div class="table-header">
          <div>Goal</div>
          <div>Project</div>
          <div>Status</div>
          <div>Iter</div>
          <div>Tokens</div>
          <div>Cost</div>
          <div>Tok/Iter</div>
          <div>$/Iter</div>
        </div>
        <div v-for="s in sessionBreakdown" :key="s.id" class="table-row" :class="s.status">
          <div class="cell-goal">{{ s.goal.length > 40 ? s.goal.slice(0, 40) + '...' : s.goal }}</div>
          <div class="cell-project">{{ s.project }}</div>
          <div class="cell-status" :class="s.status">{{ s.status }}</div>
          <div class="cell-number">{{ s.iteration }}</div>
          <div class="cell-number">{{ formatTokens(s.tokens) }}</div>
          <div class="cell-number">{{ formatCost(s.cost) }}</div>
          <div class="cell-number">{{ formatTokens(s.tokensPerIteration) }}</div>
          <div class="cell-number">{{ formatCost(s.costPerIteration) }}</div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.cost-analysis-view {
  padding: var(--space-4);
  height: 100%;
  overflow-y: auto;
  background: var(--bg-base);
}

.cost-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--space-4);
}
.filter-bar {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.filter-select {
  padding: 4px 8px;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: var(--bg-surface);
  color: var(--text-secondary);
  font-size: 12px;
  cursor: pointer;
}
.filter-select:hover { border-color: var(--text-muted); }
.filter-select:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 1px;
}


.cost-title {
  font-size: 20px;
  font-weight: 600;
  color: var(--text-primary);
}

.refresh-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: var(--bg-surface);
  color: var(--text-muted);
  cursor: pointer;
}
.refresh-btn:hover { color: var(--text-primary); }

.metric-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
  gap: var(--space-3);
  margin-bottom: var(--space-5);
}

.metric-card {
  padding: var(--space-3);
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
}

.metric-label {
  font-size: 11px;
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.05em;
  margin-bottom: var(--space-1);
}

.metric-value {
  font-size: 22px;
  font-weight: 600;
  color: var(--text-primary);
  font-family: var(--font-mono, monospace);
}

.section {
  margin-bottom: var(--space-5);
}

.section-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--text-primary);
  margin-bottom: var(--space-3);
}

.empty-state {
  padding: var(--space-4);
  text-align: center;
  color: var(--text-muted);
  font-size: 13px;
}

.project-table, .session-table {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  overflow: hidden;
}

.table-header {
  display: grid;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  background: var(--bg-surface);
  border-bottom: 1px solid var(--border-subtle);
  font-size: 11px;
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.project-table .table-header {
  grid-template-columns: 2fr 1fr 1fr 1fr 1fr;
}

.session-table .table-header {
  grid-template-columns: 2.5fr 1fr 0.8fr 0.5fr 0.8fr 0.8fr 0.8fr 0.8fr;
}

.table-row {
  display: grid;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  border-bottom: 1px solid var(--border-subtle);
  font-size: 13px;
  color: var(--text-secondary);
}

.table-row:last-child {
  border-bottom: none;
}

.project-table .table-row {
  grid-template-columns: 2fr 1fr 1fr 1fr 1fr;
}

.session-table .table-row {
  grid-template-columns: 2.5fr 1fr 0.8fr 0.5fr 0.8fr 0.8fr 0.8fr 0.8fr;
}

.cell-project {
  color: var(--text-primary);
  font-weight: 500;
}

.cell-goal {
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.cell-number {
  font-family: var(--font-mono, monospace);
  text-align: right;
}

.cell-status {
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.cell-status.completed { color: var(--success); }
.cell-status.failed { color: var(--error); }
.cell-status.running { color: var(--warning); }
.cell-status.stopped { color: var(--text-muted); }

.animate-spin {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}

/* ═══ Responsive ═══ */

@media (max-width: 767px) {
  .metric-grid {
    grid-template-columns: 1fr 1fr;
  }

  /* Tables: horizontally scrollable on mobile */
  .project-table, .session-table {
    overflow-x: auto;
  }

  .project-table .table-header,
  .project-table .table-row {
    grid-template-columns: 2fr 1fr 1fr 1fr 1fr;
    min-width: 500px;
  }

  .session-table .table-header,
  .session-table .table-row {
    grid-template-columns: 2fr 1fr 1fr 1fr 1fr 1fr 1fr 1fr;
    min-width: 600px;
  }
}
</style>
