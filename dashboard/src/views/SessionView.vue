<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useApi, type SessionEntry } from '../composables/useApi'
import Badge from '../components/ui/Badge.vue'
import Icon from '../components/ui/Icon.vue'

const route = useRoute()
const router = useRouter()
const api = useApi()

const session = ref<SessionEntry | null>(null)
const isLoading = ref(true)

function getStatusColor(status: string): 'green' | 'amber' | 'crimson' | 'gray' {
  switch (status) {
    case 'running': return 'green'
    case 'completed': return 'amber'
    case 'failed': return 'crimson'
    default: return 'gray'
  }
}

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

      <!-- Placeholder for live logs -->
      <div class="session-logs">
        <div class="logs-header">
          <h2 class="logs-title">Agent Logs</h2>
        </div>
        <div class="logs-empty">
          <Icon name="terminal" :size="24" class="empty-icon" />
          <p>Live agent logs will appear here during execution.</p>
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
</style>
