<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { useRouter } from 'vue-router'
import { useApi, type SessionEntry } from '../composables/useApi'
import { useToast } from '../composables/useToast'
import Badge from '../components/ui/Badge.vue'
import EmptyState from '../components/ui/EmptyState.vue'
import Icon from '../components/ui/Icon.vue'

const router = useRouter()
const api = useApi()
const toast = useToast()

const sessions = ref<SessionEntry[]>([])
const isLoading = ref(true)

let refreshInterval: ReturnType<typeof setInterval> | null = null

function getStatusColor(status: string): 'green' | 'amber' | 'crimson' | 'gray' {
  switch (status) {
    case 'running': return 'green'
    case 'completed': return 'amber'
    case 'failed': return 'crimson'
    default: return 'gray'
  }
}

async function loadSessions() {
  try {
    sessions.value = await api.getSessions()
  } catch {
    // Background polling — don't spam toasts on every failed poll
  }
  isLoading.value = false
}

async function handleStop(sessionId: string) {
  try {
    await api.stopSession(sessionId)
    toast.success('Session stop signal sent')
    await loadSessions()
  } catch (error: unknown) {
    toast.error('Failed to stop session')
  }
}

onMounted(() => {
  loadSessions()
  refreshInterval = setInterval(loadSessions, 5000)
})

onUnmounted(() => {
  if (refreshInterval) clearInterval(refreshInterval)
})
</script>

<template>
  <div class="sessions-view">
    <div class="sessions-header">
      <div>
        <h1 class="sessions-title">Sessions</h1>
        <p class="sessions-subtitle">All goal execution sessions</p>
      </div>
      <button class="btn btn-ghost" @click="router.push('/')">
        <Icon name="chevron-left" :size="14" />
        Back
      </button>
    </div>

    <template v-if="isLoading">
      <div class="loading-state">
        <span class="loading-spinner" />
        Loading sessions...
      </div>
    </template>

    <EmptyState
      v-else-if="sessions.length === 0"
      icon="server"
      title="No sessions yet"
      description="Run a goal to see session activity here."
      action-label="Go to Dashboard"
      :on-action="() => router.push('/')"
    />

    <div v-else class="sessions-table-wrapper">
      <!-- Desktop table -->
      <div class="sessions-table table-layout">
        <div class="table-header">
          <span class="col-id">ID</span>
          <span class="col-goal">Goal</span>
          <span class="col-project">Project</span>
          <span class="col-phase">Phase</span>
          <span class="col-iteration">Iteration</span>
          <span class="col-status">Status</span>
          <span class="col-actions">Actions</span>
        </div>

        <div
          v-for="session in sessions"
          :key="session.id"
          class="table-row"
          @click="router.push(`/sessions/${session.id}`)"
        >
          <span class="col-id mono">{{ session.id.slice(0, 8) }}</span>
          <span class="col-goal">{{ session.goal }}</span>
          <span class="col-project">{{ session.project }}</span>
          <span class="col-phase">{{ session.phase }}</span>
          <span class="col-iteration">{{ session.iteration }}</span>
          <span class="col-status">
            <Badge :variant="getStatusColor(session.status)" size="sm">
              {{ session.status }}
            </Badge>
          </span>
          <span class="col-actions" @click.stop>
            <button
              v-if="session.status === 'running'"
              class="btn-icon"
              @click="handleStop(session.id)"
              aria-label="Stop session"
            >
              <Icon name="stop" :size="14" />
            </button>
          </span>
        </div>
      </div>

      <!-- Mobile cards -->
      <div class="sessions-cards">
        <div
          v-for="session in sessions"
          :key="session.id"
          class="session-card"
          @click="router.push(`/sessions/${session.id}`)"
        >
          <div class="session-card-header">
            <span class="session-card-id mono">{{ session.id.slice(0, 8) }}</span>
            <Badge :variant="getStatusColor(session.status)" size="sm">
              {{ session.status }}
            </Badge>
          </div>
          <div class="session-card-goal">{{ session.goal }}</div>
          <div class="session-card-meta">
            <span>{{ session.project }}</span>
            <span>&middot;</span>
            <span>{{ session.phase }}</span>
            <span>&middot;</span>
            <span>Iter {{ session.iteration }}</span>
          </div>
          <div v-if="session.status === 'running'" class="session-card-actions" @click.stop>
            <button class="btn-icon" @click="handleStop(session.id)" aria-label="Stop session">
              <Icon name="stop" :size="14" />
            </button>
          </div>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.sessions-view {
  padding: var(--space-6);
  width: 100%;
  flex: 1;
  overflow-y: auto;
  min-height: 0;
}

.sessions-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  margin-bottom: var(--space-6);
}

.sessions-title {
  font-size: 24px;
  font-weight: 600;
  color: var(--text-primary);
  letter-spacing: -0.02em;
}

.sessions-subtitle {
  font-size: 13px;
  color: var(--text-muted);
  margin-top: var(--space-1);
}

.btn {
  all: unset;
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-md);
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: all var(--transition-fast);
  font-family: inherit;
  color: var(--text-secondary);
}

.btn-ghost:hover {
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
  border: 1px dashed var(--border-subtle);
  border-radius: var(--radius-lg);
}

.empty-icon {
  opacity: 0.3;
}

.sessions-table-wrapper {
  width: 100%;
}

/* Desktop table — hidden on mobile */
.table-layout {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  overflow: hidden;
  background: var(--bg-surface);
}

.table-header {
  display: grid;
  grid-template-columns: 100px 1fr 120px 100px 80px 100px 60px;
  gap: var(--space-4);
  padding: var(--space-3) var(--space-4);
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.05em;
  text-transform: uppercase;
  color: var(--text-muted);
  border-bottom: 1px solid var(--border-subtle);
  background: var(--bg-elevated);
}

.table-row {
  display: grid;
  grid-template-columns: 100px 1fr 120px 100px 80px 100px 60px;
  gap: var(--space-4);
  padding: var(--space-3) var(--space-4);
  font-size: 13px;
  color: var(--text-secondary);
  cursor: pointer;
  transition: background var(--transition-fast);
  border-bottom: 1px solid var(--border-subtle);
  align-items: center;
}

.table-row:last-child {
  border-bottom: none;
}

.table-row:hover {
  background: var(--bg-hover);
}

.mono {
  font-family: var(--font-mono);
  font-size: 12px;
}

.btn-icon {
  all: unset;
  width: 28px;
  height: 28px;
  display: flex;
  align-items: center;
  justify-content: center;
  border-radius: var(--radius-md);
  cursor: pointer;
  color: var(--text-muted);
  transition: all var(--transition-fast);
}

.btn-icon:hover {
  color: var(--error);
  background: rgba(239, 68, 68, 0.1);
}

/* Mobile cards — hidden on desktop */
.session-card {
  display: none;
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  padding: var(--space-4);
  cursor: pointer;
  transition: border-color var(--transition-fast);
}

.session-card:hover {
  border-color: var(--border-default);
}

.session-card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--space-2);
}

.session-card-id {
  font-size: 12px;
  color: var(--text-muted);
}

.session-card-goal {
  font-size: 14px;
  font-weight: 500;
  color: var(--text-primary);
  margin-bottom: var(--space-1);
}

.session-card-meta {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: 12px;
  color: var(--text-muted);
}

.session-card-actions {
  margin-top: var(--space-3);
  display: flex;
  justify-content: flex-end;
}

@media (max-width: 767px) {
  .table-layout {
    display: none;
  }

  .sessions-cards {
    display: flex;
    flex-direction: column;
    gap: var(--space-3);
  }

  .session-card {
    display: block;
  }
}

@media (min-width: 768px) {
  .sessions-cards {
    display: none;
  }
}
</style>
