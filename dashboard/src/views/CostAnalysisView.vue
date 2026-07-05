<script setup lang="ts">
/**
 * CostAnalysisView — Dedicated cost & efficiency analysis dashboard.
 *
 * Shows:
 * - Total tokens and cost across all sessions
 * - Per-session breakdown (tokens, cost, duration, $/iteration)
 * - Cost by project
 * - Efficiency metrics (tokens per iteration, cost per agent)
 */
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useApi, type SessionEntry } from '../composables/useApi'
import Icon from '../components/ui/Icon.vue'

const api = useApi()

const sessions = ref<SessionEntry[]>([])
const isLoading = ref(true)
let refreshInterval: ReturnType<typeof setInterval> | null = null

async function loadData() {
  isLoading.value = true
  try {
    sessions.value = await api.getSessions()
  } catch {
    // silent
  }
  isLoading.value = false
}

onMounted(() => {
  loadData()
  refreshInterval = setInterval(loadData, 5000)
})

onUnmounted(() => {
  if (refreshInterval) clearInterval(refreshInterval)
})

// ─── Computed metrics ────────────────────────────────────────────

const totalTokens = computed(() =>
  sessions.value.reduce((sum, s) => sum + (s.tokens_used || 0), 0)
)

const totalCost = computed(() =>
  sessions.value.reduce((sum, s) => sum + (s.cost_usd || 0), 0)
)

const completedSessions = computed(() =>
  sessions.value.filter(s => s.status === 'completed' || s.status === 'failed')
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
  for (const s of sessions.value) {
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
  return sessions.value
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
      <button class="refresh-btn" @click="loadData" :disabled="isLoading">
        <Icon v-if="isLoading" name="refresh" :size="14" class="animate-spin" />
        <Icon v-else name="refresh" :size="14" />
      </button>
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
      <div v-if="projectCosts.length === 0" class="empty-state">
        <p>No data yet. Run a goal to see cost analysis.</p>
      </div>
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
      <div v-if="sessionBreakdown.length === 0" class="empty-state">
        <p>No sessions yet.</p>
      </div>
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

.cell-status.completed { color: var(--success, #22c55e); }
.cell-status.failed { color: var(--danger, #ef4444); }
.cell-status.running { color: var(--warning, #f59e0b); }
.cell-status.stopped { color: var(--text-muted); }

.animate-spin {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}
</style>
