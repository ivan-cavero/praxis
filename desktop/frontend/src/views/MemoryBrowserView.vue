<script setup lang="ts">
/**
 * MemoryBrowserView — Browse memory layers and persistence state.
 *
 * Shows:
 * - Memory stats (sessions, events, snapshots, projects)
 * - Session checkpoints with their state (phase, iteration, goal)
 * - Event store path and database info
 */
import { ref, onMounted, onUnmounted } from 'vue'
import { useApi } from '../composables/useApi'
import Icon from '../components/ui/Icon.vue'
import EmptyState from '../components/ui/EmptyState.vue'

const api = useApi()

interface MemoryStats {
  total_sessions: number
  total_events: number
  total_snapshots: number
  total_projects: number
  event_store_path: string | null
}

interface DebugSession {
  session_id: string
  goal: string
  phase: string
  iteration: number
  version: number
  updated_at: string
}

const stats = ref<MemoryStats | null>(null)
const debugSessions = ref<DebugSession[]>([])
const isLoading = ref(true)
const expandedSession = ref<string | null>(null)
let refreshInterval: ReturnType<typeof setInterval> | null = null

function loadData() {
  isLoading.value = true
  Promise.all([
    api.get<MemoryStats>('/memory/stats'),
    api.get<DebugSession[]>('/debug/sessions'),
  ])
    .then(([memStats, sessions]) => {
      stats.value = memStats
      debugSessions.value = sessions
    })
    .catch(() => { /* Background polling */ })
    .finally(() => { isLoading.value = false })
}

function toggleSession(id: string) {
  expandedSession.value = expandedSession.value === id ? null : id
}

function formatTime(ts: string): string {
  if (!ts) return '—'
  return ts.replace('T', ' ').slice(0, 19)
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
  <div class="memory-browser-view">
    <div class="mb-header">
      <h1 class="mb-title">Memory Browser</h1>
      <button class="refresh-btn" @click="loadData" :disabled="isLoading" aria-label="Refresh data">
        <Icon v-if="isLoading" name="refresh" :size="14" class="animate-spin" />
        <Icon v-else name="refresh" :size="14" />
      </button>
    </div>

    <!-- Stats grid -->
    <div class="stats-grid">
      <div class="stat-card">
        <Icon name="folder" :size="20" class="stat-icon" />
        <div class="stat-info">
          <div class="stat-value">{{ stats?.total_projects ?? 0 }}</div>
          <div class="stat-label">Projects</div>
        </div>
      </div>
      <div class="stat-card">
        <Icon name="server" :size="20" class="stat-icon" />
        <div class="stat-info">
          <div class="stat-value">{{ stats?.total_sessions ?? 0 }}</div>
          <div class="stat-label">Sessions</div>
        </div>
      </div>
      <div class="stat-card">
        <Icon name="database" :size="20" class="stat-icon" />
        <div class="stat-info">
          <div class="stat-value">{{ stats?.total_snapshots ?? 0 }}</div>
          <div class="stat-label">Snapshots</div>
        </div>
      </div>
      <div class="stat-card">
        <Icon name="list" :size="20" class="stat-icon" />
        <div class="stat-info">
          <div class="stat-value">{{ stats?.total_events ?? 0 }}</div>
          <div class="stat-label">Events</div>
        </div>
      </div>
    </div>

    <!-- Database info -->
    <div v-if="stats?.event_store_path" class="db-info">
      <Icon name="database" :size="14" />
      <span class="db-path">{{ stats.event_store_path }}</span>
    </div>

    <!-- Session checkpoints -->
    <div class="section">
      <h2 class="section-title">Session Checkpoints</h2>
      <EmptyState
        v-if="debugSessions.length === 0"
        icon="database"
        title="No checkpoints found"
        description="Run a goal to create session checkpoints."
      />
      <div v-else class="checkpoint-list">
        <div
          v-for="s in debugSessions"
          :key="s.session_id"
          class="checkpoint-item"
          :class="{ expanded: expandedSession === s.session_id }"
        >
          <div class="checkpoint-header" @click="toggleSession(s.session_id)">
            <Icon
              :name="expandedSession === s.session_id ? 'chevron-down' : 'chevron-right'"
              :size="14"
            />
            <span class="cp-goal">{{ s.goal.length > 50 ? s.goal.slice(0, 50) + '...' : s.goal }}</span>
            <span class="cp-phase" :class="s.phase">{{ s.phase }}</span>
            <span class="cp-iter">iter {{ s.iteration }}</span>
          </div>
          <div v-if="expandedSession === s.session_id" class="checkpoint-detail">
            <div class="detail-row">
              <span class="detail-label">Session ID</span>
              <span class="detail-value mono">{{ s.session_id }}</span>
            </div>
            <div class="detail-row">
              <span class="detail-label">Phase</span>
              <span class="detail-value">{{ s.phase }}</span>
            </div>
            <div class="detail-row">
              <span class="detail-label">Iteration</span>
              <span class="detail-value">{{ s.iteration }}</span>
            </div>
            <div class="detail-row">
              <span class="detail-label">Version</span>
              <span class="detail-value">{{ s.version }}</span>
            </div>
            <div class="detail-row">
              <span class="detail-label">Updated</span>
              <span class="detail-value">{{ formatTime(s.updated_at) }}</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.memory-browser-view {
  padding: var(--space-4);
  height: 100%;
  overflow-y: auto;
  background: var(--bg-base);
}

.mb-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--space-4);
}

.mb-title {
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

.stats-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(180px, 1fr));
  gap: var(--space-3);
  margin-bottom: var(--space-4);
}

.stat-card {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-3);
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
}

.stat-icon {
  color: var(--text-muted);
  flex-shrink: 0;
}

.stat-value {
  font-size: 22px;
  font-weight: 600;
  color: var(--text-primary);
  font-family: var(--font-mono, monospace);
}

.stat-label {
  font-size: 11px;
  color: var(--text-muted);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.db-info {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  margin-bottom: var(--space-4);
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  color: var(--text-muted);
  font-size: 12px;
}

.db-path {
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

.checkpoint-list {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  overflow: hidden;
}

.checkpoint-item {
  border-bottom: 1px solid var(--border-subtle);
}
.checkpoint-item:last-child { border-bottom: none; }

.checkpoint-header {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  cursor: pointer;
  font-size: 13px;
}
.checkpoint-header:hover {
  background: var(--bg-elevated);
}

.cp-goal {
  flex: 1;
  color: var(--text-primary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.cp-phase {
  font-size: 11px;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  padding: 2px 6px;
  border-radius: var(--radius-sm);
  background: var(--bg-elevated);
}
.cp-phase.Completed { color: var(--success); }
.cp-phase.Failed { color: var(--error); }
.cp-phase.Idle, .cp-phase.Planning { color: var(--info, var(--accent)); }

.cp-iter {
  color: var(--text-muted);
  font-family: var(--font-mono, monospace);
  font-size: 12px;
}

.checkpoint-detail {
  padding: var(--space-2) var(--space-3) var(--space-3);
  background: var(--bg-elevated);
}

.detail-row {
  display: flex;
  gap: var(--space-3);
  padding: 4px 0;
  font-size: 12px;
}

.detail-label {
  width: 100px;
  color: var(--text-muted);
  flex-shrink: 0;
}

.detail-value {
  color: var(--text-secondary);
}

.mono {
  font-family: var(--font-mono, monospace);
}

.animate-spin {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}
</style>
