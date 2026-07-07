<script setup lang="ts">
import { computed } from 'vue'
import Icon from './ui/Icon.vue'
import type { SystemEvent, AgentStartedEvent, AgentCompletedEvent, ToolCalledEvent, PhaseChangedEvent, TokenUsedEvent, DelegationStartedEvent, DelegationCompletedEvent, ContextPressureEvent, ContextCompressionEvent, AgentOutputEvent } from '../composables/useWebSocket'

interface Props {
  events: SystemEvent[]
  sessionId?: string
}

const { events = [], sessionId: _sessionId } = defineProps<Props>()

const MAX_VISIBLE = 50

const visibleEvents = computed(() => {
  const sorted = events.slice().sort((a: SystemEvent, b: SystemEvent) => {
    return new Date(a.timestamp).getTime() - new Date(b.timestamp).getTime()
  })
  return sorted.slice(-MAX_VISIBLE)
})

const groupedEvents = computed(() => {
  const groups: Array<{ kind: string; icon: string; color: string; items: SystemEvent[] }> = []
  const kindMap = new Map<string, typeof groups[number]>()

  const kindConfig: Record<string, { icon: string; color: string }> = {
    AgentStarted: { icon: 'user-plus', color: 'emerald' },
    AgentCompleted: { icon: 'check-circle', color: 'emerald' },
    AgentOutput: { icon: 'message-square', color: 'blue' },
    PhaseChanged: { icon: 'git-branch', color: 'violet' },
    ToolCalled: { icon: 'wrench', color: 'amber' },
    TokenUsed: { icon: 'coins', color: 'cyan' },
    DelegationStarted: { icon: 'share-2', color: 'purple' },
    DelegationCompleted: { icon: 'users', color: 'purple' },
    ContextPressure: { icon: 'alert-triangle', color: 'amber' },
    ContextCompression: { icon: 'archive', color: 'slate' },
  }

  for (const event of visibleEvents.value) {
    const kindName = Object.keys(event.kind).find((k) => event.kind[k] !== undefined)
    if (!kindName) continue

    let group = kindMap.get(kindName)
    if (!group) {
      const config = kindConfig[kindName] || { icon: 'activity', color: 'slate' }
      group = { kind: kindName, icon: config.icon, color: config.color, items: [] }
      groups.push(group)
      kindMap.set(kindName, group)
    }
    group.items.push(event)
  }

  return groups
})

function formatTime(timestamp: string): string {
  try {
    const d = new Date(timestamp)
    return d.toLocaleTimeString([], { hour: '2-digit', minute: '2-digit', second: '2-digit' })
  } catch {
    return timestamp
  }
}

function formatDuration(ms: number): string {
  if (ms < 1000) return `${ms}ms`
  const s = Math.floor(ms / 1000)
  if (s < 60) return `${s}s`
  const m = Math.floor(s / 60)
  return `${m}m ${s % 60}s`
}

function getEventDescription(event: SystemEvent, kindName: string): string {
  const payload = getEventPayload(event, kindName)
  switch (kindName) {
    case 'AgentStarted': {
      const p = payload as AgentStartedEvent
      return `Agent "${p.agent}" (${p.role}) started in phase "${p.phase}"`
    }
    case 'AgentCompleted': {
      const p = payload as AgentCompletedEvent
      return `Agent "${p.agent}" completed (${p.status}) — ${formatDuration(p.duration_ms)}`
    }
    case 'AgentOutput': {
      const p = payload as AgentOutputEvent
      return `Agent "${p.agent}" output — ${p.delta.slice(0, 40)}${p.delta.length > 40 ? '...' : ''}`
    }
    case 'PhaseChanged': {
      const p = payload as PhaseChangedEvent
      return `Phase: ${p.from} → ${p.to}`
    }
    case 'ToolCalled': {
      const p = payload as ToolCalledEvent
      return `Agent "${p.agent}" called "${p.tool}" (${formatDuration(p.duration_ms)})`
    }
    case 'TokenUsed': {
      const p = payload as TokenUsedEvent
      return `Tokens: ${p.input} in / ${p.output} out (${p.provider} · ${p.model})`
    }
    case 'DelegationStarted': {
      const p = payload as DelegationStartedEvent
      return `Delegation: "${p.parent}" → "${p.child}" (depth ${p.depth})`
    }
    case 'DelegationCompleted': {
      const p = payload as DelegationCompletedEvent
      return `Delegation done: "${p.child}" (${p.status}) — ${formatDuration(p.duration_ms)}`
    }
    case 'ContextPressure': {
      const p = payload as ContextPressureEvent
      return `Context pressure: ${p.action} (${p.pressure}%)`
    }
    case 'ContextCompression': {
      const p = payload as ContextCompressionEvent
      return `Compression: ${p.before_tokens} → ${p.after_tokens} tokens (${(p.ratio * 100).toFixed(0)}% reduction)`
    }
    default:
      return JSON.stringify(event.kind)
  }
}

function getEventPayload(event: SystemEvent, kindName: string): unknown {
  return event.kind[kindName]
}

function isRecent(event: SystemEvent): boolean {
  const now = Date.now()
  const then = new Date(event.timestamp).getTime()
  return now - then < 30000
}

</script>

<template>
  <div class="session-timeline">
    <div v-if="events.length === 0" class="timeline-empty">
      <Icon name="activity" :size="24" class="timeline-empty-icon" />
      <p>No events yet</p>
      <p class="timeline-empty-hint">Events will appear here as the session runs</p>
    </div>

    <div v-else class="timeline-feed">
      <div
        v-for="group in groupedEvents"
        :key="group.kind"
        class="timeline-group"
      >
        <div class="timeline-group-header">
          <Icon :name="group.icon" :size="14" :class="`timeline-icon-${group.color}`" />
          <span class="timeline-group-label">{{ group.kind }}</span>
          <span class="timeline-group-count">{{ group.items.length }}</span>
        </div>

        <div class="timeline-items">
          <div
            v-for="event in group.items"
            :key="event.id"
            class="timeline-item"
            :class="{ 'timeline-item-recent': isRecent(event) }"
          >
            <div class="timeline-item-dot" :class="`timeline-dot-${group.color}`" />
            <div class="timeline-item-content">
              <div class="timeline-item-text">{{ getEventDescription(event, Object.keys(event.kind).find(k => event.kind[k]) || '') }}</div>
              <div class="timeline-item-meta">
                <span class="timeline-item-time">{{ formatTime(event.timestamp) }}</span>
                <span class="timeline-item-source">{{ event.source }}</span>
              </div>
            </div>
          </div>
        </div>
      </div>
    </div>

    <div v-if="events.length > MAX_VISIBLE" class="timeline-more">
      Showing last {{ MAX_VISIBLE }} of {{ events.length }} events
    </div>
  </div>
</template>

<style scoped>
.session-timeline {
  display: flex;
  flex-direction: column;
  gap: 16px;
  font-size: 13px;
}

/* ── Empty state ─────────────────────────────────────────── */

.timeline-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 8px;
  padding: 48px 24px;
  color: var(--text-muted, #666);
}

.timeline-empty-icon {
  opacity: 0.3;
}

.timeline-empty p {
  margin: 0;
}

.timeline-empty-hint {
  font-size: 12px;
  opacity: 0.6;
}

/* ── Feed ────────────────────────────────────────────────── */

.timeline-feed {
  display: flex;
  flex-direction: column;
  gap: 16px;
  max-height: 600px;
  overflow-y: auto;
  padding-right: 4px;
}

.timeline-feed::-webkit-scrollbar {
  width: 4px;
}

.timeline-feed::-webkit-scrollbar-track {
  background: transparent;
}

.timeline-feed::-webkit-scrollbar-thumb {
  background: var(--border-color, #333);
  border-radius: 2px;
}

/* ── Group ───────────────────────────────────────────────── */

.timeline-group {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.timeline-group-header {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 4px 0;
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-muted, #666);
  border-bottom: 1px solid var(--border-color, #222);
}

.timeline-group-label {
  flex: 1;
}

.timeline-group-count {
  font-size: 10px;
  font-weight: 500;
  background: var(--bg-secondary, #1a1a2e);
  padding: 1px 6px;
  border-radius: 10px;
  color: var(--text-secondary, #aaa);
}

/* ── Items ───────────────────────────────────────────────── */

.timeline-items {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.timeline-item {
  display: flex;
  gap: 10px;
  padding: 6px 8px;
  border-radius: 6px;
  transition: background 0.15s ease;
}

.timeline-item:hover {
  background: var(--bg-secondary, rgba(255, 255, 255, 0.03));
}

.timeline-item-recent {
  animation: timelineFadeIn 0.5s ease;
}

@keyframes timelineFadeIn {
  from { opacity: 0; transform: translateY(-4px); }
  to { opacity: 1; transform: translateY(0); }
}

.timeline-item-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  flex-shrink: 0;
  margin-top: 5px;
}

.timeline-item-content {
  flex: 1;
  min-width: 0;
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.timeline-item-text {
  color: var(--text-primary, #eee);
  line-height: 1.4;
  word-break: break-word;
}

.timeline-item-meta {
  display: flex;
  gap: 12px;
  font-size: 11px;
  color: var(--text-muted, #666);
}

.timeline-item-time {
  font-variant-numeric: tabular-nums;
}

.timeline-item-source {
  font-family: var(--font-mono, 'JetBrains Mono', 'SF Mono', monospace);
  font-size: 10px;
  opacity: 0.7;
}

/* ── Dot colors ──────────────────────────────────────────── */

.timeline-dot-emerald { background: #4ade80; box-shadow: 0 0 4px #4ade8044; }
.timeline-dot-blue { background: #60a5fa; box-shadow: 0 0 4px #60a5fa44; }
.timeline-dot-amber { background: #fbbf24; box-shadow: 0 0 4px #fbbf2444; }
.timeline-dot-violet { background: #a78bfa; box-shadow: 0 0 4px #a78bfa44; }
.timeline-dot-cyan { background: #22d3ee; box-shadow: 0 0 4px #22d3ee44; }
.timeline-dot-purple { background: #c084fc; box-shadow: 0 0 4px #c084fc44; }
.timeline-dot-slate { background: #94a3b8; }

/* ── Icon colors ─────────────────────────────────────────── */

.timeline-icon-emerald { color: #4ade80; }
.timeline-icon-blue { color: #60a5fa; }
.timeline-icon-amber { color: #fbbf24; }
.timeline-icon-violet { color: #a78bfa; }
.timeline-icon-cyan { color: #22d3ee; }
.timeline-icon-purple { color: #c084fc; }
.timeline-icon-slate { color: #94a3b8; }

/* ── More indicator ──────────────────────────────────────── */

.timeline-more {
  text-align: center;
  font-size: 11px;
  color: var(--text-muted, #666);
  padding: 8px;
  border-top: 1px solid var(--border-color, #222);
}
</style>
