<script setup lang="ts">
import { ref, watch, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useApi, type SessionEntry } from '../composables/useApi'
import { useWebSocket, filterEvents, type AgentOutputEvent, type AgentStartedEvent, type AgentCompletedEvent, type ToolCalledEvent } from '../composables/useWebSocket'
import Badge from '../components/ui/Badge.vue'
import Icon from '../components/ui/Icon.vue'

const route = useRoute()
const router = useRouter()
const api = useApi()
const ws = useWebSocket()

const session = ref<SessionEntry | null>(null)
const isLoading = ref(true)

// Live agent log
interface LiveLogEntry {
  timestamp: string
  text: string
  kind: 'start' | 'output' | 'complete' | 'tool'
  agent: string
}

const liveLog = ref<LiveLogEntry[]>([])
const agentStreams = ref<Record<string, string>>({})
const sessionActive = ref(false)

// Watch WebSocket events for live logs
watch(() => ws.events.value, (allEvents) => {
  // Process AgentStarted events
  const starts = filterEvents<AgentStartedEvent>(allEvents, 'AgentStarted')
  for (const s of starts.slice(-5)) { // last 5 starts
    if (!liveLog.value.find(l => l.text.includes(s.agent) && l.kind === 'start')) {
      liveLog.value = [...liveLog.value, {
        timestamp: new Date().toISOString(),
        text: `${s.role} (${s.agent}) started in ${s.phase}`,
        kind: 'start',
        agent: s.agent,
      }]
      sessionActive.value = true
    }
  }

  // Process AgentOutput events (streaming text)
  const outputs = filterEvents<AgentOutputEvent>(allEvents, 'AgentOutput')
  for (const o of outputs.slice(-20)) {
    const current = agentStreams.value[o.agent] || ''
    agentStreams.value = {
      ...agentStreams.value,
      [o.agent]: current + o.delta,
    }
  }

  // Process AgentCompleted events
  const completes = filterEvents<AgentCompletedEvent>(allEvents, 'AgentCompleted')
  for (const c of completes.slice(-5)) {
    if (!liveLog.value.find(l => l.text.includes(c.agent) && l.kind === 'complete')) {
      liveLog.value = [...liveLog.value, {
        timestamp: new Date().toISOString(),
        text: `${c.agent} ${c.status} in ${c.duration_ms}ms`,
        kind: 'complete',
        agent: c.agent,
      }]
    }
  }

  // Process ToolCalled events
  const tools = filterEvents<ToolCalledEvent>(allEvents, 'ToolCalled')
  for (const t of tools.slice(-10)) {
    liveLog.value = [...liveLog.value, {
      timestamp: new Date().toISOString(),
      text: `🔧 ${t.agent} called ${t.tool} ${t.success ? '✓' : '✗'} (${t.duration_ms}ms)`,
      kind: 'tool',
      agent: t.agent,
    }]
  }

  // Keep log manageable
  if (liveLog.value.length > 200) {
    liveLog.value = liveLog.value.slice(-200)
  }
}, { deep: true })

function getStatusColor(status: string): 'green' | 'amber' | 'crimson' | 'gray' {
  switch (status) {
    case 'running': return 'green'
    case 'completed': return 'amber'
    case 'failed': return 'crimson'
    default: return 'gray'
  }
}

// Import onMounted from vue — load session data on mount
onMounted(async () => {
  try {
    const id = route.params.id as string
    session.value = await api.getSession(id)
  } catch {
    // session not found
  }
  isLoading.value = false
})
</script>

<template>
  <div class="session-view">
    <button class="back-btn" @click="router.push('/sessions')">
      <Icon name="chevron-left" :size="14" />
      All Sessions
    </button>

    <div v-if="isLoading" class="loading-state">
      <span class="loading-spinner" />
      Loading...
    </div>

    <div v-else-if="!session" class="empty-state">
      <Icon name="alert" :size="32" class="empty-icon" />
      <p>Session not found.</p>
    </div>

    <template v-else>
      <div class="session-header">
        <div>
          <h1 class="session-title">{{ session.goal }}</h1>
          <p class="session-meta">
            {{ session.project }} &middot; {{ session.id.slice(0, 8) }}
            &middot; Started {{ new Date(session.started_at).toLocaleString() }}
          </p>
        </div>
        <Badge :variant="getStatusColor(session.status)" size="md">
          {{ session.status }}
        </Badge>
      </div>

      <div class="session-detail-grid">
        <div class="detail-card">
          <div class="detail-card-label">Phase</div>
          <div class="detail-card-value">{{ session.phase }}</div>
        </div>
        <div class="detail-card">
          <div class="detail-card-label">Iteration</div>
          <div class="detail-card-value">{{ session.iteration }}</div>
        </div>
        <div class="detail-card">
          <div class="detail-card-label">Status</div>
          <div class="detail-card-value">{{ session.status }}</div>
        </div>
        <div class="detail-card">
          <div class="detail-card-label">Completed</div>
          <div class="detail-card-value">
            {{ session.completed_at ? new Date(session.completed_at).toLocaleString() : '—' }}
          </div>
        </div>
      </div>

      <!-- Live agent logs -->
      <div class="session-logs">
        <div class="logs-header">
          <h2 class="logs-title">Agent Logs</h2>
          <span v-if="sessionActive" class="live-badge">LIVE</span>
        </div>
        <div v-if="liveLog.length === 0 && !sessionActive" class="logs-empty">
          <Icon name="terminal" :size="24" class="empty-icon" />
          <p>Live agent logs will appear here during execution.</p>
        </div>
        <div v-else class="logs-list">
          <div v-for="(entry, idx) in liveLog" :key="idx" class="log-entry" :class="entry.kind">
            <span class="log-time">{{ entry.timestamp.slice(11, 19) }}</span>
            <span class="log-agent">{{ entry.agent }}</span>
            <span class="log-text">{{ entry.text }}</span>
          </div>
          <!-- Show active agent stream -->
          <div v-for="(stream, agent) in agentStreams" :key="agent" class="stream-entry">
            <span class="log-time">stream</span>
            <span class="log-agent">{{ agent }}</span>
            <pre class="stream-text">{{ stream.slice(-500) }}</pre>
          </div>
        </div>
      </div>
    </template>
  </div>
</template>

<style scoped>
.session-view {
  padding: var(--space-6);
  max-width: 1000px;
  margin: 0 auto;
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
  flex: 1;
  overflow-y: auto;
  min-height: 0;
}

.back-btn {
  all: unset;
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-md);
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  color: var(--text-secondary);
  transition: all var(--transition-fast);
  font-family: inherit;
  align-self: flex-start;
}

.back-btn:hover {
  color: var(--text-primary);
  background: var(--bg-hover);
}

.loading-state {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-3);
  padding: var(--space-12);
  color: var(--text-muted);
}

.loading-spinner {
  width: 14px;
  height: 14px;
  border: 2px solid transparent;
  border-top-color: currentColor;
  border-radius: 50%;
  animation: spin 0.8s linear infinite;
  display: inline-block;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: var(--space-16);
  color: var(--text-muted);
  gap: var(--space-3);
}

.empty-icon {
  opacity: 0.3;
}

.session-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
}

.session-title {
  font-size: 24px;
  font-weight: 600;
  color: var(--text-primary);
  letter-spacing: -0.02em;
  margin-bottom: var(--space-1);
}

.session-meta {
  font-size: 13px;
  color: var(--text-muted);
}

.session-detail-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
  gap: var(--space-4);
}

.detail-card {
  padding: var(--space-4);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  background: var(--bg-surface);
}

.detail-card-label {
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.05em;
  text-transform: uppercase;
  color: var(--text-muted);
  margin-bottom: var(--space-2);
}

.detail-card-value {
  font-size: 16px;
  font-weight: 600;
  color: var(--text-primary);
}

.session-logs {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  overflow: hidden;
  background: var(--bg-surface);
}

.logs-header {
  padding: var(--space-4);
  border-bottom: 1px solid var(--border-subtle);
}

.logs-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
}

.logs-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: var(--space-12);
  color: var(--text-muted);
  gap: var(--space-3);
}

.logs-empty .empty-icon {
  opacity: 0.3;
}

.live-badge {
  font-size: 10px;
  font-weight: 700;
  color: #22c55e;
  background: rgba(34, 197, 94, 0.1);
  padding: 2px 6px;
  border-radius: 4px;
  animation: pulse 2s infinite;
}

.logs-list {
  max-height: 500px;
  overflow-y: auto;
  padding: var(--space-2) 0;
}

.log-entry,
.stream-entry {
  display: flex;
  align-items: flex-start;
  gap: var(--space-3);
  padding: var(--space-1) var(--space-4);
  font-size: 12px;
  font-family: var(--font-mono, 'JetBrains Mono', monospace);
  border-bottom: 1px solid var(--border-subtle);
}

.log-time {
  color: var(--text-disabled);
  flex-shrink: 0;
  width: 55px;
}

.log-agent {
  color: #22c55e;
  flex-shrink: 0;
  width: 80px;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.log-text {
  color: var(--text-secondary);
}

.log-entry.start .log-text {
  color: #3b82f6;
}

.log-entry.complete .log-text {
  color: #22c55e;
}

.log-entry.tool .log-text {
  color: #eab308;
}

.stream-text {
  margin: 0;
  color: #e4e4e7;
  white-space: pre-wrap;
  word-break: break-word;
  line-height: 1.5;
  max-height: 120px;
  overflow-y: auto;
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.4; }
}
</style>
