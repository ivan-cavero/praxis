<script setup lang="ts">
/**
 * LiveMonitorPanel — real-time session monitor.
 *
 * Consumes WebSocket events via useLiveMonitor and shows:
 * - Current phase + iteration
 * - Active agents with tool calls and streaming output
 * - Delegation tree (parent → child)
 * - Total tokens + cost
 */
import { computed } from 'vue'
import { useLiveMonitor } from '../composables/useLiveMonitor'
import Badge from './ui/Badge.vue'

const props = defineProps<{
  sessionId: string | null
}>()

const { state, agents, activeAgents, completedAgents } = useLiveMonitor(
  computed(() => props.sessionId)
)

function formatDuration(ms: number | null): string {
  if (ms === null) return '—'
  if (ms < 1000) return `${ms}ms`
  if (ms < 60000) return `${(ms / 1000).toFixed(1)}s`
  return `${(ms / 60000).toFixed(1)}m`
}

function formatTokens(n: number): string {
  if (n < 1000) return n.toString()
  if (n < 1000000) return `${(n / 1000).toFixed(1)}k`
  return `${(n / 1000000).toFixed(2)}M`
}

function agentStatusColor(status: string): 'green' | 'emerald' | 'amber' | 'crimson' | 'gray' {
  switch (status) {
    case 'running': return 'emerald'
    case 'completed': return 'green'
    case 'failed': return 'crimson'
    case 'queued': return 'gray'
    default: return 'gray'
  }
}

const streamingText = computed(() => {
  const running = activeAgents.value
  if (running.length === 0) return ''
  const agent = running[0]
  return agent.outputChunks.slice(-50).join('')
})
</script>

<template>
  <div class="live-monitor">
    <!-- Header bar -->
    <div class="monitor-header">
      <div class="header-left">
        <span class="live-indicator" :class="{ active: activeAgents.length > 0 }"></span>
        <span class="label">LIVE MONITOR</span>
      </div>
      <div class="header-right">
        <Badge :variant="activeAgents.length > 0 ? 'emerald' : 'gray'">
          {{ activeAgents.length }} running
        </Badge>
        <Badge variant="green">{{ completedAgents.length }} done</Badge>
      </div>
    </div>

    <!-- Phase + iteration bar -->
    <div class="phase-bar">
      <div class="phase-item">
        <span class="phase-label">Phase</span>
        <span class="phase-value">{{ state.phase || '—' }}</span>
      </div>
      <div class="phase-item">
        <span class="phase-label">Iteration</span>
        <span class="phase-value">{{ state.iteration }}</span>
      </div>
      <div class="phase-item">
        <span class="phase-label">Tokens</span>
        <span class="phase-value">{{ formatTokens(state.totalTokens) }}</span>
      </div>
      <div class="phase-item">
        <span class="phase-label">Cost</span>
        <span class="phase-value">${{ state.totalCost.toFixed(4) }}</span>
      </div>
    </div>

    <!-- Agent list -->
    <div class="agent-list">
      <div v-if="agents.length === 0" class="empty-state">
        No agents have run yet. Waiting for events...
      </div>
      <div
        v-for="agent in agents"
        :key="agent.name"
        class="agent-row"
        :class="{ running: agent.status === 'running' }"
      >
        <div class="agent-header">
          <span class="agent-status-dot" :class="agent.status"></span>
          <span class="agent-name">{{ agent.name }}</span>
          <span class="agent-role" v-if="agent.role !== agent.name">({{ agent.role }})</span>
          <Badge :variant="agentStatusColor(agent.status)" size="sm">
            {{ agent.status }}
          </Badge>
          <span class="agent-duration">{{ formatDuration(agent.durationMs) }}</span>
          <span class="agent-tokens" v-if="agent.tokensUsed > 0">
            {{ formatTokens(agent.tokensUsed) }} tok
          </span>
          <span class="agent-delegation" v-if="agent.delegatedFrom">
            ← {{ agent.delegatedFrom }}
          </span>
        </div>

        <!-- Tool calls -->
        <div v-if="agent.toolCalls.length > 0" class="tool-calls">
          <div v-for="(tc, i) in agent.toolCalls" :key="i" class="tool-call">
            <span class="tool-icon">{{ tc.success ? '✓' : '✗' }}</span>
            <span class="tool-name">{{ tc.tool }}</span>
            <span class="tool-duration">{{ formatDuration(tc.durationMs) }}</span>
          </div>
        </div>

        <!-- Delegated to -->
        <div v-if="agent.delegatedTo.length > 0" class="delegations">
          <div v-for="child in agent.delegatedTo" :key="child" class="delegation-arrow">
            └→ {{ child }}
          </div>
        </div>
      </div>
    </div>

    <!-- Streaming output -->
    <div v-if="streamingText" class="streaming-output">
      <div class="streaming-label">Streaming output:</div>
      <pre class="streaming-text">{{ streamingText }}</pre>
    </div>

    <!-- Delegation tree -->
    <div v-if="state.delegations.length > 0" class="delegation-tree">
      <div class="tree-label">Delegations ({{ state.delegations.length }}):</div>
      <div v-for="(d, i) in state.delegations" :key="i" class="tree-entry">
        <span class="tree-parent">{{ d.parent }}</span>
        <span class="tree-arrow">→</span>
        <span class="tree-child">{{ d.child }}</span>
        <Badge :variant="d.status === 'completed' ? 'green' : d.status === 'failed' ? 'crimson' : 'emerald'" size="sm">
          {{ d.status }}
        </Badge>
        <span class="tree-duration" v-if="d.durationMs">{{ formatDuration(d.durationMs) }}</span>
        <span class="tree-tokens" v-if="d.tokensUsed > 0">{{ formatTokens(d.tokensUsed) }} tok</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.live-monitor {
  border: 1px solid var(--border-color);
  border-radius: var(--radius-md);
  overflow: hidden;
  font-size: 13px;
}

.monitor-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  padding: var(--space-2) var(--space-3);
  background: var(--bg-secondary);
  border-bottom: 1px solid var(--border-color);
}

.header-left {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.live-indicator {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--text-disabled);
}

.live-indicator.active {
  background: var(--success);
  box-shadow: 0 0 6px var(--success);
  animation: pulse 1.5s infinite;
}

@keyframes pulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.5; }
}

.label {
  font-weight: 600;
  letter-spacing: 0.5px;
  color: var(--text-secondary);
}

.header-right {
  display: flex;
  gap: var(--space-2);
}

.phase-bar {
  display: flex;
  gap: var(--space-4);
  padding: var(--space-2) var(--space-3);
  background: var(--bg-tertiary);
  border-bottom: 1px solid var(--border-color);
}

.phase-item {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.phase-label {
  font-size: 10px;
  text-transform: uppercase;
  color: var(--text-muted);
}

.phase-value {
  font-weight: 600;
  color: var(--text-primary);
}

.agent-list {
  max-height: 400px;
  overflow-y: auto;
}

.empty-state {
  padding: var(--space-6);
  text-align: center;
  color: var(--text-muted);
}

.agent-row {
  padding: var(--space-2) var(--space-3);
  border-bottom: 1px solid var(--border-subtle);
}

.agent-row.running {
  background: var(--bg-hover);
}

.agent-header {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-wrap: wrap;
}

.agent-status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
}

.agent-status-dot.running { background: #00aaff; animation: pulse 1s infinite; }
.agent-status-dot.completed { background: var(--success); }
.agent-status-dot.failed { background: var(--error); }
.agent-status-dot.queued { background: var(--text-disabled); }

.agent-name {
  font-weight: 600;
  color: var(--text-primary);
}

.agent-role {
  color: var(--text-muted);
  font-size: 11px;
}

.agent-duration, .agent-tokens {
  color: var(--text-secondary);
  font-size: 11px;
}

.agent-delegation {
  color: var(--text-accent);
  font-size: 11px;
}

.tool-calls {
  margin-top: var(--space-1);
  padding-left: var(--space-5);
}

.tool-call {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: 11px;
  color: var(--text-secondary);
}

.tool-icon { font-size: 10px; }
.tool-call .tool-icon { color: var(--success); }
.tool-call:last-child .tool-icon { color: var(--error); }

.delegations {
  padding-left: var(--space-5);
  margin-top: 2px;
}

.delegation-arrow {
  font-size: 11px;
  color: var(--text-accent);
}

.streaming-output {
  padding: var(--space-2) var(--space-3);
  border-top: 1px solid var(--border-color);
  background: var(--bg-tertiary);
}

.streaming-label {
  font-size: 10px;
  text-transform: uppercase;
  color: var(--text-muted);
  margin-bottom: var(--space-1);
}

.streaming-text {
  font-family: var(--font-mono, 'Fira Code', monospace);
  font-size: 12px;
  color: var(--text-primary);
  max-height: 150px;
  overflow-y: auto;
  white-space: pre-wrap;
  word-break: break-word;
}

.delegation-tree {
  padding: var(--space-2) var(--space-3);
  border-top: 1px solid var(--border-color);
}

.tree-label {
  font-size: 10px;
  text-transform: uppercase;
  color: var(--text-muted);
  margin-bottom: var(--space-1);
}

.tree-entry {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: 12px;
  padding: 2px 0;
}

.tree-parent { font-weight: 600; }
.tree-arrow { color: var(--text-accent); }
.tree-child { color: var(--text-primary); }
.tree-duration, .tree-tokens {
  color: var(--text-secondary);
  font-size: 11px;
}
</style>
