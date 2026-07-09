<script setup lang="ts">
import { ref, watch, computed, onMounted } from 'vue'
import Icon from '../components/ui/Icon.vue'
import { useApi } from '../composables/useApi'
import {
  useWebSocket,
  getEventPayload,
  type PhaseChangedEvent,
  type AgentCompletedEvent,
  type AgentStartedEvent,
  type AgentOutputEvent,
} from '../composables/useWebSocket'

const ws = useWebSocket()
const api = useApi()

// ─── Session selector ──────────────────────────────────────────────

interface SessionEntry {
  id: string
  project: string
  goal: string
  phase: string
  status: string
  started_at: string
}

const sessions = ref<SessionEntry[]>([])
const selectedSessionId = ref<string | null>(null)
const isLoadingSessions = ref(false)

async function loadSessions(): Promise<void> {
  isLoadingSessions.value = true
  try {
    const data = await api.get<SessionEntry[]>('/sessions')
    sessions.value = data || []
    // Auto-select the most recent running session, or the first one
    if (sessions.value.length > 0 && !selectedSessionId.value) {
      const running = sessions.value.find(s => s.status === 'running')
      selectedSessionId.value = running?.id ?? sessions.value[0].id
    }
  } catch {
    sessions.value = []
  } finally {
    isLoadingSessions.value = false
  }
}

onMounted(() => {
  loadSessions()
})

// ─── Phase definitions ────────────────────────────────────────────

interface AgentInfo {
  name: string
  role: string
  status: 'running' | 'completed' | 'failed'
  output: string
  durationMs: number | null
  startedAt: string | null
}

interface PhaseState {
  id: string
  label: string
  icon: string
  color: string
  active: boolean
  expanded: boolean
  agents: AgentInfo[]
}

const phases = ref<PhaseState[]>([
  { id: 'planning', label: 'Planning', icon: 'brain', color: '#22c55e', active: false, expanded: false, agents: [] },
  { id: 'designing', label: 'Designing', icon: 'code', color: '#3b82f6', active: false, expanded: false, agents: [] },
  { id: 'implementing', label: 'Implementing', icon: 'terminal', color: '#eab308', active: false, expanded: false, agents: [] },
  { id: 'reviewing', label: 'Reviewing', icon: 'eye', color: '#f97316', active: false, expanded: false, agents: [] },
  { id: 'testing', label: 'Testing', icon: 'check', color: '#a855f7', active: false, expanded: false, agents: [] },
  { id: 'security', label: 'Security', icon: 'shield', color: '#ef4444', active: false, expanded: false, agents: [] },
  { id: 'finalizing', label: 'Finalizing', icon: 'server', color: '#22c55e', active: false, expanded: false, agents: [] },
])

// ─── Activity log ──────────────────────────────────────────────────

interface LogEntry {
  timestamp: string
  text: string
  kind: 'phase' | 'agent' | 'gate'
}

const log = ref<LogEntry[]>([])
const logExpanded = ref(true)

// ─── Computed ──────────────────────────────────────────────────────

const runningAgentCount = computed(() =>
  phases.value.reduce((count, p) => count + p.agents.filter(a => a.status === 'running').length, 0)
)

// ─── Toggle expand ─────────────────────────────────────────────────

function togglePhase(phaseId: string) {
  const phase = phases.value.find(p => p.id === phaseId)
  if (phase) phase.expanded = !phase.expanded
}

// ─── Watch WebSocket events (filtered by selected session) ─────────

watch(() => ws.events.value, (allEvents) => {
  for (let i = allEvents.length - 1; i >= 0; i--) {
    const event = allEvents[i]

    // Filter by session ID if one is selected
    if (selectedSessionId.value) {
      const eventSession = (event as { metadata?: { session_id?: string } }).metadata?.session_id
      if (eventSession && eventSession !== selectedSessionId.value) continue
    }

    // Phase changed
    const phaseChange = getEventPayload<PhaseChangedEvent>(event, 'PhaseChanged')
    if (phaseChange) {
      const toLower = (phaseChange.to || '').toLowerCase()
      phases.value = phases.value.map((p) => ({
        ...p,
        active: p.id === toLower,
        expanded: p.id === toLower ? true : p.expanded,
      }))
      log.value = [...log.value, {
        timestamp: event.timestamp,
        text: `Phase changed: ${phaseChange.from} → ${phaseChange.to}`,
        kind: 'phase' as const,
      }]
      continue
    }

    // Agent started
    const agentStart = getEventPayload<AgentStartedEvent>(event, 'AgentStarted')
    if (agentStart) {
      const phase = phases.value.find(p => p.id === (agentStart.phase || '').toLowerCase())
      if (phase) {
        phase.agents = [...phase.agents, {
          name: agentStart.agent,
          role: agentStart.role,
          status: 'running' as const,
          output: '',
          durationMs: null,
          startedAt: event.timestamp,
        }]
      }
      log.value = [...log.value, {
        timestamp: event.timestamp,
        text: `${agentStart.role} (${agentStart.agent}) started in ${agentStart.phase}`,
        kind: 'agent' as const,
      }]
      continue
    }

    // Agent completed
    const agentDone = getEventPayload<AgentCompletedEvent>(event, 'AgentCompleted')
    if (agentDone) {
      for (const phase of phases.value) {
        const agent = phase.agents.find(a => a.name === agentDone.agent)
        if (agent) {
          agent.status = agentDone.status === 'completed' ? 'completed' as const : 'failed' as const
          agent.durationMs = agentDone.duration_ms
          break
        }
      }
      log.value = [...log.value, {
        timestamp: event.timestamp,
        text: `${agentDone.role} (${agentDone.agent}) ${agentDone.status} in ${agentDone.duration_ms}ms`,
        kind: 'agent' as const,
      }]
      continue
    }

    // Agent streaming output
    const agentOut = getEventPayload<AgentOutputEvent>(event, 'AgentOutput')
    if (agentOut && agentOut.delta) {
      for (const phase of phases.value) {
        const agent = phase.agents.find(a => a.name === agentOut.agent)
        if (agent) {
          agent.output += agentOut.delta
          break
        }
      }
    }
  }
}, { deep: true })
</script>

<template>
  <div class="pipeline-view">
    <!-- Header -->
    <div class="pipeline-header">
      <div>
        <h1 class="pipeline-title">Pipeline</h1>
        <p class="pipeline-subtitle">
          Live agent workflow
          <template v-if="runningAgentCount > 0">
            &middot; <span class="text-primary">{{ runningAgentCount }} agent{{ runningAgentCount > 1 ? 's' : '' }} running</span>
          </template>
        </p>
      </div>

      <!-- Session selector -->
      <div class="session-selector">
        <label class="session-label">
          <Icon name="server" :size="14" />
          Session
        </label>
        <select
          v-model="selectedSessionId"
          class="session-select"
          :disabled="isLoadingSessions || sessions.length === 0"
        >
          <option :value="null" v-if="sessions.length === 0">No sessions available</option>
          <option v-for="s in sessions" :key="s.id" :value="s.id">
            {{ s.id.slice(0, 8) }}... — {{ s.goal.slice(0, 40) }}{{ s.goal.length > 40 ? '...' : '' }} ({{ s.status }})
          </option>
        </select>
        <button class="session-refresh" @click="loadSessions" title="Refresh sessions">
          <Icon name="refresh" :size="14" />
        </button>
      </div>
    </div>

    <!-- Phase Flow -->
    <div class="pipeline-flow">
      <div
        v-for="(phase, index) in phases"
        :key="phase.id"
        class="phase-wrapper"
        :class="{ active: phase.active }"
      >
        <!-- Connector -->
        <div v-if="index > 0" class="phase-connector" :class="{ active: phases[index - 1].active }">
          <div class="connector-line" />
        </div>

        <!-- Phase Header (clickable) -->
        <div class="phase-header" :style="{ '--phase-color': phase.color }" @click="togglePhase(phase.id)">
          <div class="phase-icon" :class="{ active: phase.active }">
            <Icon :name="phase.icon" :size="20" />
          </div>
          <div class="phase-info">
            <span class="phase-label">{{ phase.label }}</span>
            <span class="phase-meta">
              <span v-if="phase.active" class="phase-status-live" :style="{ color: phase.color }">In Progress</span>
              <span v-else class="phase-status-idle">Pending</span>
              <template v-if="phase.agents.length > 0">
                &middot; {{ phase.agents.length }} agent{{ phase.agents.length > 1 ? 's' : '' }}
              </template>
            </span>
          </div>
          <div class="phase-right">
            <span v-if="phase.active" class="phase-pulse" :style="{ background: phase.color }" />
            <Icon
              :name="phase.expanded ? 'chevron-up' : 'chevron-down'"
              :size="16"
              class="phase-chevron"
            />
          </div>
        </div>

        <!-- Expanded Agent Details -->
        <div v-if="phase.expanded" class="phase-agents">
          <div v-if="phase.agents.length === 0" class="phase-agents-empty">
            No agents in this phase yet
          </div>
          <div v-else v-for="agent in phase.agents" :key="agent.name" class="agent-detail-card">
            <div class="agent-detail-header">
              <div class="agent-detail-info">
                <span class="agent-detail-name">{{ agent.role }}</span>
                <span class="agent-detail-id">{{ agent.name }}</span>
              </div>
              <div class="agent-detail-status" :class="agent.status">
                <span class="agent-status-dot" />
                <span>{{ agent.status }}</span>
                <span v-if="agent.durationMs" class="agent-duration">{{ agent.durationMs }}ms</span>
              </div>
            </div>
            <div v-if="agent.output" class="agent-detail-output">
              <pre><code>{{ agent.output }}</code></pre>
            </div>
          </div>
        </div>
      </div>
    </div>

    <!-- Activity Log -->
    <div class="pipeline-log">
      <div class="log-header" @click="logExpanded = !logExpanded">
        <h2 class="log-title">Activity Log</h2>
        <Icon :name="logExpanded ? 'chevron-up' : 'chevron-down'" :size="16" class="log-chevron" />
      </div>
      <div v-if="logExpanded">
        <div v-if="log.length === 0" class="log-empty">
          <Icon name="terminal" :size="24" class="empty-icon" />
          <p>Agent activity will appear here during a session.</p>
        </div>
        <div v-else class="log-list">
          <div v-for="(entry, idx) in log" :key="idx" class="log-entry" :class="entry.kind">
            <span class="log-time">{{ entry.timestamp.slice(11, 19) }}</span>
            <span class="log-text">{{ entry.text }}</span>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.pipeline-view {
  padding: var(--space-6);
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
  width: 100%;
  flex: 1;
  overflow-y: auto;
  min-height: 0;
}

.pipeline-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  gap: var(--space-4);
  margin-bottom: var(--space-2);
}
.pipeline-title {
  font-size: 24px;
  font-weight: 600;
  color: var(--text-primary);
  letter-spacing: -0.02em;
}
.pipeline-subtitle {
  font-size: 13px;
  color: var(--text-muted);
  margin-top: var(--space-1);
}
.text-primary { color: var(--primary); }

/* ─── Session Selector ─────────────────────────────────────────── */

.session-selector {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-shrink: 0;
}

.session-label {
  display: flex;
  align-items: center;
  gap: 4px;
  font-size: 12px;
  color: var(--text-muted);
  font-weight: 500;
}

.session-select {
  padding: var(--space-1) var(--space-3);
  border-radius: var(--radius-md);
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  color: var(--text-primary);
  font-size: 12px;
  font-family: inherit;
  cursor: pointer;
  max-width: 320px;
  outline: none;
  transition: border-color var(--transition-fast);
}

.session-select:focus {
  border-color: var(--primary);
}

.session-select:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.session-refresh {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: var(--bg-elevated);
  color: var(--text-muted);
  cursor: pointer;
  transition: all var(--transition-fast);
}

.session-refresh:hover {
  color: var(--text-primary);
  background: var(--bg-hover);
}

/* ─── Phase Flow ─────────────────────────────────────────────────── */

.pipeline-flow {
  display: flex;
  flex-direction: column;
  gap: 0;
  position: relative;
}

.phase-wrapper { display: flex; flex-direction: column; }

.phase-connector {
  display: flex;
  justify-content: center;
  padding: 2px 0;
  margin-left: 24px;
}
.connector-line {
  width: 2px;
  height: 16px;
  background: var(--border-subtle);
  transition: background var(--transition-slow);
}
.phase-connector.active .connector-line { background: var(--primary); }

/* ─── Phase Header ───────────────────────────────────────────────── */

.phase-header {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-3) var(--space-4);
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  cursor: pointer;
  transition: all var(--transition-fast);
  border-left: 3px solid transparent;
  margin-bottom: var(--space-1);
}
.phase-header:hover {
  border-color: var(--border-default);
  background: var(--bg-hover);
}
.phase-wrapper.active .phase-header {
  border-color: var(--border-default);
  border-left-color: var(--phase-color);
  box-shadow: 0 0 16px color-mix(in srgb, var(--phase-color) 8%, transparent);
}

.phase-icon {
  width: 36px;
  height: 36px;
  border-radius: var(--radius-md);
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--bg-elevated);
  color: var(--text-muted);
  flex-shrink: 0;
  transition: all var(--transition-fast);
}
.phase-icon.active {
  color: var(--phase-color);
  background: color-mix(in srgb, var(--phase-color) 15%, transparent);
}

.phase-info { flex: 1; display: flex; flex-direction: column; gap: 2px; }
.phase-label {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
}
.phase-meta {
  font-size: 11px;
  color: var(--text-muted);
}
.phase-status-live { font-weight: 600; }
.phase-status-idle { color: var(--text-disabled); }

.phase-right {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}
.phase-pulse {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  animation: pulse 2s infinite;
}
.phase-chevron {
  color: var(--text-muted);
  transition: transform var(--transition-fast);
}

/* ─── Agent Detail Cards ──────────────────────────────────────────── */

.phase-agents {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-2) 0 var(--space-3) var(--space-6);
}
.phase-agents-empty {
  font-size: 12px;
  color: var(--text-disabled);
  padding: var(--space-2) var(--space-3);
  font-style: italic;
}

.agent-detail-card {
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  overflow: hidden;
}
.agent-detail-card:hover { border-color: var(--border-default); }

.agent-detail-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-2) var(--space-3);
}
.agent-detail-info { display: flex; align-items: center; gap: var(--space-2); }
.agent-detail-name {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-primary);
}
.agent-detail-id {
  font-size: 11px;
  font-family: var(--font-mono);
  color: var(--text-muted);
}
.agent-detail-status {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  font-size: 11px;
  font-weight: 600;
}
.agent-detail-status.running { color: var(--primary); }
.agent-detail-status.completed { color: var(--text-muted); }
.agent-detail-status.failed { color: var(--error); }

.agent-status-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: currentColor;
}
.agent-detail-status.running .agent-status-dot { animation: pulse 2s infinite; }

.agent-duration {
  font-family: var(--font-mono);
  font-size: 10px;
  opacity: 0.6;
}

.agent-detail-output {
  border-top: 1px solid var(--border-subtle);
  padding: var(--space-2) var(--space-3);
  max-height: 200px;
  overflow-y: auto;
  background: #0a0a0c;
}
.agent-detail-output pre {
  margin: 0;
  font-family: var(--font-mono);
  font-size: 11px;
  line-height: 1.5;
  color: #d4d4d8;
  white-space: pre-wrap;
  word-break: break-word;
}

/* ─── Activity Log ────────────────────────────────────────────────── */

.pipeline-log {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  overflow: hidden;
  background: var(--bg-surface);
}
.log-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-3) var(--space-4);
  border-bottom: 1px solid var(--border-subtle);
  cursor: pointer;
  transition: background var(--transition-fast);
}
.log-header:hover { background: var(--bg-hover); }
.log-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
}
.log-chevron { color: var(--text-muted); }

.log-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: var(--space-8);
  color: var(--text-muted);
  gap: var(--space-3);
}
.empty-icon { opacity: 0.3; }

.log-list { max-height: 300px; overflow-y: auto; }
.log-entry {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-1) var(--space-4);
  font-size: 12px;
  border-bottom: 1px solid var(--border-subtle);
  font-family: var(--font-mono);
}
.log-entry:last-child { border-bottom: none; }
.log-time {
  color: var(--text-disabled);
  flex-shrink: 0;
  width: 60px;
}
.log-text { color: var(--text-primary); }
.log-entry.phase .log-text { color: var(--primary); }
.log-entry.agent .log-text { color: var(--text-secondary); }
.log-entry.gate .log-text { color: var(--warning); }

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.4; }
}

/* ═══ Responsive ═══ */

@media (max-width: 767px) {
  .pipeline-header {
    flex-direction: column;
    gap: var(--space-3);
  }

  .session-selector {
    width: 100%;
  }

  .session-select {
    max-width: 100%;
  }
}
</style>
