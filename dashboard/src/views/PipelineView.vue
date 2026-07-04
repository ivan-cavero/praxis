<script setup lang="ts">
import { ref, watch, computed } from 'vue'
import Icon from '../components/ui/Icon.vue'
import { useWebSocket, getEventPayload, type PhaseChangedEvent, type AgentCompletedEvent, type AgentStartedEvent, type AgentOutputEvent } from '../composables/useWebSocket'

const ws = useWebSocket()

type PhaseDef = {
  id: string
  label: string
  icon: string
  color: string
  active: boolean
}

const phases = ref<PhaseDef[]>([
  { id: 'planning', label: 'Planning', icon: 'brain', color: '#22c55e', active: false },
  { id: 'designing', label: 'Designing', icon: 'code', color: '#3b82f6', active: false },
  { id: 'implementing', label: 'Implementing', icon: 'terminal', color: '#eab308', active: false },
  { id: 'reviewing', label: 'Reviewing', icon: 'eye', color: '#f97316', active: false },
  { id: 'testing', label: 'Testing', icon: 'check', color: '#a855f7', active: false },
  { id: 'security', label: 'Security', icon: 'shield', color: '#ef4444', active: false },
  { id: 'finalizing', label: 'Finalizing', icon: 'server', color: '#22c55e', active: false },
])

interface LogEntry {
  timestamp: string
  text: string
  kind: 'phase' | 'agent' | 'gate'
}

const log = ref<LogEntry[]>([])

// Track accumulated streaming output per agent
const agentOutputs = ref<Record<string, string>>({})
const activeAgent = ref<string | null>(null)

// Latest streaming text for the active agent
const activeAgentStream = computed(() => {
  if (!activeAgent.value) return ''
  return agentOutputs.value[activeAgent.value] || ''
})

// Watch for PhaseChanged events via WebSocket
watch(() => ws.events.value, (allEvents) => {
  // Walk backwards — only process new events
  for (let i = allEvents.length - 1; i >= 0; i--) {
    const event = allEvents[i]
    const phaseChange = getEventPayload<PhaseChangedEvent>(event, 'PhaseChanged')
    if (phaseChange) {
        const toLower = (phaseChange.to || '').toLowerCase()

      // Deactivate all, then activate the target phase
      phases.value = phases.value.map((p) => ({
        ...p,
        active: p.id === toLower,
      }))

      log.value = [...log.value, {
        timestamp: event.timestamp,
        text: `Phase changed: ${phaseChange.from} → ${phaseChange.to}`,
        kind: 'phase' as const,
      }]
      continue
    }

    const agentStart = getEventPayload<AgentStartedEvent>(event, 'AgentStarted')
    if (agentStart) {
      log.value = [...log.value, {
        timestamp: event.timestamp,
        text: `${agentStart.role} (${agentStart.agent}) started in ${agentStart.phase}`,
        kind: 'agent' as const,
      }]
      continue
    }

    const agentDone = getEventPayload<AgentCompletedEvent>(event, 'AgentCompleted')
    if (agentDone) {
      log.value = [...log.value, {
        timestamp: event.timestamp,
        text: `${agentDone.role} (${agentDone.agent}) ${agentDone.status} in ${agentDone.duration_ms}ms`,
        kind: 'agent' as const,
      }]
      // Keep the output visible even after completion
      continue
    }

    // Accumulate streaming output per agent
    const agentOut = getEventPayload<AgentOutputEvent>(event, 'AgentOutput')
    if (agentOut && agentOut.delta) {
      const current = agentOutputs.value[agentOut.agent] || ''
      agentOutputs.value = {
        ...agentOutputs.value,
        [agentOut.agent]: current + agentOut.delta,
      }
      activeAgent.value = agentOut.agent
    }
  }
}, { deep: true })</script>

<template>
  <div class="pipeline-view">
    <div class="pipeline-header">
      <h1 class="pipeline-title">Pipeline</h1>
      <p class="pipeline-subtitle">Live agent workflow</p>
    </div>

    <div class="pipeline-flow">
      <div
        v-for="(phase, index) in phases"
        :key="phase.id"
        class="phase-node"
        :class="{ active: phase.active }"
      >
        <div v-if="index > 0" class="phase-node-connector" :class="{ active: phases[index - 1].active }">
          <div class="connector-line" />
          <Icon name="chevron-right" :size="14" class="connector-arrow" />
        </div>

        <div class="phase-node-card" :style="{ '--phase-color': phase.color }">
          <div class="phase-node-icon">
            <Icon :name="phase.icon" :size="24" />
          </div>
          <div class="phase-node-label">{{ phase.label }}</div>
          <div class="phase-node-status">
            <span v-if="phase.active" class="status-dot live" :style="{ background: phase.color }" />
            <span v-else class="status-dot" />
            {{ phase.active ? 'In Progress' : 'Pending' }}
      </div>
    </div>

    <!-- Agent streaming output -->
    <div v-if="activeAgent" class="stream-panel">
      <div class="stream-header">
        <h2 class="stream-title">Agent Output: {{ activeAgent }}</h2>
        <span class="stream-badge">LIVE</span>
      </div>
      <pre class="stream-content"><code>{{ activeAgentStream }}</code></pre>
    </div>
      </div>
    </div>

    <!-- Activity log -->
    <div class="pipeline-log">
      <div class="log-header">
        <h2 class="log-title">Activity Log</h2>
      </div>
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
</template>

<style scoped>
.pipeline-view {
  padding: var(--space-6);
  display: flex;
  flex-direction: column;
  gap: var(--space-8);
  max-width: 1000px;
  margin: 0 auto;
}

.pipeline-header {
  margin-bottom: var(--space-4);
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

.pipeline-flow {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  position: relative;
}

.phase-node {
  display: flex;
  align-items: center;
  gap: var(--space-4);
}

.phase-node-connector {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  width: 40px;
  flex-shrink: 0;
  position: relative;
}

.connector-line {
  width: 2px;
  height: 24px;
  background: var(--border-subtle);
  margin: 0 auto;
  transition: background var(--transition-slow);
}

.phase-node-connector.active .connector-line {
  background: var(--primary);
}

.connector-arrow {
  color: var(--text-muted);
  position: absolute;
  right: -4px;
  top: 50%;
  transform: translateY(-50%);
}

.phase-node-card {
  display: flex;
  align-items: center;
  gap: var(--space-4);
  padding: var(--space-4) var(--space-5);
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  flex: 1;
  transition: all var(--transition-slow);
  border-left: 3px solid transparent;
}

.phase-node.active .phase-node-card {
  border-color: var(--border-default);
  border-left-color: var(--phase-color);
  box-shadow: 0 0 20px color-mix(in srgb, var(--phase-color) 10%, transparent);
}

.phase-node-icon {
  width: 44px;
  height: 44px;
  border-radius: var(--radius-md);
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--bg-elevated);
  color: var(--text-secondary);
  flex-shrink: 0;
}

.phase-node.active .phase-node-icon {
  color: var(--phase-color);
  background: color-mix(in srgb, var(--phase-color) 15%, transparent);
}

.phase-node-label {
  flex: 1;
  font-size: 15px;
  font-weight: 600;
  color: var(--text-primary);
}

.phase-node-status {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: 12px;
  color: var(--text-muted);
}

.status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--border-default);
}

.status-dot.live {
  animation: pulse 2s infinite;
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.4; }
}

.pipeline-log {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  overflow: hidden;
  background: var(--bg-surface);
}

.log-header {
  padding: var(--space-4);
  border-bottom: 1px solid var(--border-subtle);
}

.log-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
}

.log-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: var(--space-12);
  color: var(--text-muted);
  gap: var(--space-3);
}

.empty-icon {
  opacity: 0.3;
}

.log-list {
  max-height: 400px;
  overflow-y: auto;
}

.log-entry {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-2) var(--space-4);
  font-size: 13px;
  border-bottom: 1px solid var(--border-subtle);
  font-family: var(--font-mono);
}

.log-entry:last-child {
  border-bottom: none;
}

.log-time {
  color: var(--text-disabled);
  flex-shrink: 0;
  width: 60px;
}

.log-text {
  color: var(--text-primary);
}

.log-entry.phase .log-text {
  color: var(--primary);
}

.log-entry.agent .log-text {
  color: var(--text-secondary);
}

.log-entry.gate .log-text {
  color: var(--warning);
}

/* Agent streaming output panel */
.stream-panel {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  overflow: hidden;
  background: #0f0f11;
}

.stream-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-3) var(--space-4);
  border-bottom: 1px solid var(--border-subtle);
  background: var(--bg-surface);
}

.stream-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-primary);
}

.stream-badge {
  font-size: 10px;
  font-weight: 700;
  color: #22c55e;
  background: rgba(34, 197, 94, 0.1);
  padding: 2px 6px;
  border-radius: 4px;
  animation: pulse 2s infinite;
}

.stream-content {
  padding: var(--space-4);
  max-height: 300px;
  overflow-y: auto;
  margin: 0;
  font-family: var(--font-mono, 'JetBrains Mono', 'Fira Code', monospace);
  font-size: 12px;
  line-height: 1.6;
  color: #e4e4e7;
  white-space: pre-wrap;
  word-break: break-word;
}
</style>
