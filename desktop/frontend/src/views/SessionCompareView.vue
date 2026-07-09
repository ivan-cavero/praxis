<script setup lang="ts">
/**
 * SessionCompareView — Side-by-side comparison of two sessions.
 *
 * Lets users pick two sessions from a dropdown and compare their
 * metrics: goal, phase, iteration, tokens, cost, status, duration.
 */
import { ref, computed, onMounted } from 'vue'
import { useApi, type SessionEntry } from '../composables/useApi'
import Icon from '../components/ui/Icon.vue'
import EmptyState from '../components/ui/EmptyState.vue'

const api = useApi()

const sessions = ref<SessionEntry[]>([])
const sessionAId = ref<string>('')
const sessionBId = ref<string>('')

const sessionA = computed(() =>
  sessions.value.find(s => s.id === sessionAId.value) ?? null
)
const sessionB = computed(() =>
  sessions.value.find(s => s.id === sessionBId.value) ?? null
)

const bothSelected = computed(() => !!sessionA.value && !!sessionB.value)

interface MetricRow {
  label: string
  a: string
  b: string
  winner: 'a' | 'b' | 'tie'
}

const comparisonMetrics = computed<MetricRow[]>(() => {
  if (!bothSelected.value) return []
  const a = sessionA.value!
  const b = sessionB.value!
  return [
    { label: 'Status', a: a.status, b: b.status, winner: 'tie' },
    { label: 'Phase', a: a.phase, b: b.phase, winner: 'tie' },
    { label: 'Iteration', a: String(a.iteration), b: String(b.iteration), winner: a.iteration <= b.iteration ? 'a' : 'b' },
    { label: 'Tokens', a: formatTokens(a.tokens_used || 0), b: formatTokens(b.tokens_used || 0), winner: (a.tokens_used || 0) <= (b.tokens_used || 0) ? 'a' : 'b' },
    { label: 'Cost', a: formatCost(a.cost_usd || 0), b: formatCost(b.cost_usd || 0), winner: (a.cost_usd || 0) <= (b.cost_usd || 0) ? 'a' : 'b' },
    { label: 'Project', a: a.project, b: b.project, winner: 'tie' },
    { label: 'Started', a: formatTime(a.started_at), b: formatTime(b.started_at), winner: 'tie' },
    { label: 'Completed', a: a.completed_at ? formatTime(a.completed_at) : '—', b: b.completed_at ? formatTime(b.completed_at) : '—', winner: 'tie' },
  ]
})

function loadSessions() {
  api.getSessions()
    .then(data => { sessions.value = data })
    .catch(() => { /* Background polling */ })
}

function formatCost(cost: number): string {
  if (cost < 0.01) return `$${cost.toFixed(4)}`
  return `$${cost.toFixed(2)}`
}

function formatTokens(tokens: number): string {
  if (tokens >= 1_000_000) return `${(tokens / 1_000_000).toFixed(2)}M`
  if (tokens >= 1_000) return `${(tokens / 1_000).toFixed(1)}K`
  return tokens.toString()
}

function formatTime(ts: string): string {
  if (!ts) return '—'
  return ts.replace('T', ' ').slice(0, 19)
}

onMounted(() => {
  loadSessions()
})
</script>

<template>
  <div class="compare-view">
    <div class="compare-header">
      <h1 class="compare-title">Session Comparison</h1>
    </div>

    <EmptyState
      v-if="sessions.length < 2"
      icon="server"
      title="Not enough sessions to compare"
      description="Run at least two goals to enable side-by-side comparison."
    />

    <template v-else>
      <!-- Session selectors -->
      <div class="selector-bar">
        <div class="selector-group">
          <label class="selector-label">Session A</label>
          <select v-model="sessionAId" class="selector-select" aria-label="Select session A">
            <option value="">Select session...</option>
            <option v-for="s in sessions" :key="s.id" :value="s.id">
              {{ s.goal.length > 30 ? s.goal.slice(0, 30) + '...' : s.goal }} ({{ s.id.slice(0, 8) }})
            </option>
          </select>
        </div>
        <div class="vs-badge">VS</div>
        <div class="selector-group">
          <label class="selector-label">Session B</label>
          <select v-model="sessionBId" class="selector-select" aria-label="Select session B">
            <option value="">Select session...</option>
            <option v-for="s in sessions" :key="s.id" :value="s.id">
              {{ s.goal.length > 30 ? s.goal.slice(0, 30) + '...' : s.goal }} ({{ s.id.slice(0, 8) }})
            </option>
          </select>
        </div>
      </div>

      <!-- Comparison table -->
      <div v-if="bothSelected" class="comparison-table">
        <div class="table-header">
          <div class="col-label">Metric</div>
          <div class="col-session" :class="{ winner: comparisonMetrics.every(m => m.winner === 'a') }">
            Session A
          </div>
          <div class="col-session" :class="{ winner: comparisonMetrics.every(m => m.winner === 'b') }">
            Session B
          </div>
        </div>
        <div
          v-for="metric in comparisonMetrics"
          :key="metric.label"
          class="table-row"
        >
          <div class="col-label">{{ metric.label }}</div>
          <div class="col-session" :class="{ better: metric.winner === 'a' }">{{ metric.a }}</div>
          <div class="col-session" :class="{ better: metric.winner === 'b' }">{{ metric.b }}</div>
        </div>
      </div>

      <!-- Goal display -->
      <div v-if="bothSelected" class="goal-display">
        <div class="goal-card">
          <h3 class="goal-card-title">Session A Goal</h3>
          <p class="goal-text">{{ sessionA?.goal }}</p>
        </div>
        <div class="goal-card">
          <h3 class="goal-card-title">Session B Goal</h3>
          <p class="goal-text">{{ sessionB?.goal }}</p>
        </div>
      </div>

      <!-- Placeholder when not both selected -->
      <div v-else class="compare-placeholder">
        <Icon name="server" :size="32" />
        <p>Select two sessions to compare</p>
      </div>
    </template>
  </div>
</template>

<style scoped>
.compare-view {
  padding: var(--space-4);
  height: 100%;
  overflow-y: auto;
  background: var(--bg-base);
}

.compare-header {
  margin-bottom: var(--space-4);
}

.compare-title {
  font-size: 20px;
  font-weight: 600;
  color: var(--text-primary);
}

.selector-bar {
  display: flex;
  align-items: flex-end;
  gap: var(--space-3);
  margin-bottom: var(--space-4);
}

.selector-group {
  flex: 1;
}

.selector-label {
  display: block;
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-muted);
  margin-bottom: var(--space-1);
}

.selector-select {
  width: 100%;
  padding: 6px 10px;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: var(--bg-surface);
  color: var(--text-primary);
  font-size: 13px;
  cursor: pointer;
}
.selector-select:hover { border-color: var(--text-muted); }
.selector-select:focus-visible {
  outline: 2px solid var(--primary);
  outline-offset: 1px;
}

.vs-badge {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  border-radius: var(--radius-full);
  background: var(--bg-elevated);
  color: var(--text-muted);
  font-size: 11px;
  font-weight: 600;
  flex-shrink: 0;
  margin-bottom: 2px;
}

.comparison-table {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  overflow: hidden;
  margin-bottom: var(--space-4);
}

.table-header {
  display: grid;
  grid-template-columns: 1fr 1fr 1fr;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  background: var(--bg-surface);
  border-bottom: 1px solid var(--border-subtle);
  font-size: 11px;
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.table-row {
  display: grid;
  grid-template-columns: 1fr 1fr 1fr;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  border-bottom: 1px solid var(--border-subtle);
  font-size: 13px;
  color: var(--text-secondary);
}
.table-row:last-child { border-bottom: none; }

.col-label {
  color: var(--text-muted);
  font-weight: 500;
}

.col-session {
  font-family: var(--font-mono, monospace);
}

.col-session.better {
  color: var(--success);
  font-weight: 600;
}

.col-session.winner {
  color: var(--success);
}

.goal-display {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: var(--space-3);
}

.goal-card {
  padding: var(--space-3);
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
}

.goal-card-title {
  font-size: 12px;
  font-weight: 600;
  text-transform: uppercase;
  color: var(--text-muted);
  letter-spacing: 0.05em;
  margin-bottom: var(--space-2);
}

.goal-text {
  font-size: 14px;
  color: var(--text-primary);
  line-height: 1.5;
}

.compare-placeholder {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  padding: var(--space-8);
  color: var(--text-muted);
  font-size: 14px;
}
</style>
