<script setup lang="ts">
/**
 * LogsView — Real-time log stream in the browser.
 *
 * Polls the session events API for a selected session and displays
 * events in a scrollable, auto-updating feed with event type filtering.
 */
import { ref, computed, onMounted, onUnmounted, watch, nextTick, useTemplateRef } from 'vue'
import { useApi, type SessionEntry } from '../composables/useApi'
import Icon from '../components/ui/Icon.vue'
import EmptyState from '../components/ui/EmptyState.vue'

const api = useApi()

interface LogEvent {
  id: string
  event_type: string
  payload: Record<string, unknown>
  version: number
  created_at: string
}

const sessions = ref<SessionEntry[]>([])
const selectedSessionId = ref<string>('')
const events = ref<LogEvent[]>([])
const isStreaming = ref(false)
const autoScroll = ref(true)
const eventTypeFilter = ref<string>('all')
let pollInterval: ReturnType<typeof setInterval> | null = null

const logContainer = useTemplateRef('logContainer')

const eventTypes = computed(() => {
  const types = new Set(events.value.map(e => e.event_type))
  return ['all', ...[...types].sort()]
})

const filteredEvents = computed(() => {
  if (eventTypeFilter.value === 'all') return events.value
  return events.value.filter(e => e.event_type === eventTypeFilter.value)
})

function loadSessions() {
  api.getSessions()
    .then(data => { sessions.value = data })
    .catch(() => { /* Background polling */ })
}

function loadEvents() {
  if (!selectedSessionId.value) return
  api.get<LogEvent[]>(`/sessions/${selectedSessionId.value}/events`)
    .then(data => {
      events.value = data
      if (autoScroll.value) {
        nextTick(scrollToBottom)
      }
    })
    .catch(() => { /* Session may not have events yet */ })
}

function startStreaming() {
  if (isStreaming.value) return
  isStreaming.value = true
  loadEvents()
  pollInterval = setInterval(loadEvents, 2000)
}

function stopStreaming() {
  isStreaming.value = false
  if (pollInterval) {
    clearInterval(pollInterval)
    pollInterval = null
  }
}

function toggleStreaming() {
  if (isStreaming.value) stopStreaming()
  else startStreaming()
}

function scrollToBottom() {
  const el = logContainer.value
  if (el) el.scrollTop = el.scrollHeight
}

function formatTime(ts: string): string {
  if (!ts) return ''
  return ts.slice(11, 19)
}

function eventClass(eventType: string): string {
  if (eventType.includes('error') || eventType.includes('fail')) return 'event-error'
  if (eventType.includes('complete') || eventType.includes('success')) return 'event-success'
  if (eventType.includes('start') || eventType.includes('begin')) return 'event-info'
  return 'event-default'
}

function clearEvents() {
  events.value = []
}

watch(selectedSessionId, () => {
  events.value = []
  if (selectedSessionId.value) startStreaming()
})

onMounted(() => {
  loadSessions()
})

onUnmounted(() => {
  stopStreaming()
})
</script>

<template>
  <div class="logs-view">
    <div class="logs-header">
      <h1 class="logs-title">Live Logs</h1>
      <div class="logs-controls">
        <select v-model="selectedSessionId" class="filter-select" aria-label="Select session">
          <option value="">Select a session...</option>
          <option v-for="s in sessions" :key="s.id" :value="s.id">
            {{ s.goal.length > 30 ? s.goal.slice(0, 30) + '...' : s.goal }} ({{ s.id.slice(0, 8) }})
          </option>
        </select>
        <select
          v-model="eventTypeFilter"
          class="filter-select"
          aria-label="Filter by event type"
          :disabled="!selectedSessionId"
        >
          <option v-for="t in eventTypes" :key="t" :value="t">{{ t }}</option>
        </select>
        <button
          class="stream-btn"
          :class="{ active: isStreaming }"
          @click="toggleStreaming"
          :disabled="!selectedSessionId"
        >
          <Icon :name="isStreaming ? 'stop' : 'play'" :size="14" />
          {{ isStreaming ? 'Stop' : 'Start' }}
        </button>
        <button
          class="clear-btn"
          @click="clearEvents"
          :disabled="events.length === 0"
          aria-label="Clear events"
        >
          <Icon name="trash" :size="14" />
        </button>
      </div>
    </div>

    <div v-if="!selectedSessionId" class="logs-empty">
      <EmptyState
        icon="list"
        title="No session selected"
        description="Select a session above to start streaming its event log."
      />
    </div>
    <div v-else ref="logContainer" class="log-container">
      <div v-if="filteredEvents.length === 0" class="log-empty-msg">
        <Icon name="loader" :size="16" class="animate-spin" />
        <span>Waiting for events...</span>
      </div>
      <div
        v-for="event in filteredEvents"
        :key="event.id"
        class="log-entry"
        :class="eventClass(event.event_type)"
      >
        <span class="log-time">{{ formatTime(event.created_at) }}</span>
        <span class="log-type">{{ event.event_type }}</span>
        <span class="log-payload">{{ JSON.stringify(event.payload).slice(0, 200) }}</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.logs-view {
  padding: var(--space-4);
  height: 100%;
  display: flex;
  flex-direction: column;
  background: var(--bg-base);
}

.logs-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--space-3);
  flex-shrink: 0;
}

.logs-title {
  font-size: 20px;
  font-weight: 600;
  color: var(--text-primary);
}

.logs-controls {
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
.filter-select:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.stream-btn {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 4px 10px;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: var(--bg-surface);
  color: var(--text-secondary);
  font-size: 12px;
  cursor: pointer;
}
.stream-btn:hover { color: var(--text-primary); }
.stream-btn.active {
  background: var(--error);
  color: white;
  border-color: var(--error);
}
.stream-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.clear-btn {
  display: flex;
  align-items: center;
  padding: 4px 8px;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: var(--bg-surface);
  color: var(--text-muted);
  cursor: pointer;
}
.clear-btn:hover { color: var(--error); }
.clear-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.logs-empty {
  flex: 1;
  display: flex;
  align-items: center;
  justify-content: center;
}

.log-container {
  flex: 1;
  overflow-y: auto;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  background: var(--bg-surface);
  padding: var(--space-2);
  font-family: var(--font-mono, monospace);
  font-size: 12px;
}

.log-empty-msg {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  color: var(--text-muted);
  padding: var(--space-3);
}

.log-entry {
  display: flex;
  gap: var(--space-2);
  padding: 2px 4px;
  border-bottom: 1px solid var(--border-subtle);
  line-height: 1.6;
}
.log-entry:last-child { border-bottom: none; }

.log-time {
  color: var(--text-muted);
  flex-shrink: 0;
  width: 70px;
}

.log-type {
  flex-shrink: 0;
  width: 140px;
  font-weight: 500;
}

.log-payload {
  color: var(--text-secondary);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.event-error .log-type { color: var(--error); }
.event-success .log-type { color: var(--success); }
.event-info .log-type { color: var(--info, var(--accent)); }
.event-default .log-type { color: var(--text-muted); }

.animate-spin {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}
</style>
