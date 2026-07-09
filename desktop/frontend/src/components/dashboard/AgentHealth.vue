<script setup lang="ts">
import { ref, onMounted, onUnmounted } from 'vue'
import { useApi, type AgentDefinition } from '../../composables/useApi'
import Badge from '../ui/Badge.vue'
import EmptyState from '../components/ui/EmptyState.vue'

const api = useApi()

const agents = ref<AgentDefinition[]>([])
let refreshInterval: ReturnType<typeof setInterval> | null = null

function getScopeColor(scope: string): 'green' | 'emerald' | 'amber' | 'gray' {
  switch (scope) {
    case 'builtin': return 'gray'
    case 'global': return 'emerald'
    case 'project': return 'green'
    default: return 'amber'
  }
}

async function loadAgents() {
  try {
    agents.value = await api.getAgents()
  } catch {
    // silent
  }
}

onMounted(() => {
  loadAgents()
  refreshInterval = setInterval(loadAgents, 10000)
})

onUnmounted(() => {
  if (refreshInterval) clearInterval(refreshInterval)
})
</script>

<template>
  <div class="agent-health">
    <div class="health-header">
      <h3 class="health-title">Agent Health</h3>
    </div>

    <div class="health-table">
      <div class="health-row header">
        <span class="col-agent">Agent</span>
        <span class="col-model">Model</span>
        <span class="col-depth">Depth</span>
        <span class="col-tools">Tools</span>
        <span class="col-scope">Scope</span>
      </div>

      <div
        v-for="agent in agents"
        :key="agent.name"
        class="health-row"
      >
        <span class="col-agent">
          <div class="agent-avatar-sm">{{ agent.name.charAt(0).toUpperCase() }}</div>
          {{ agent.name }}
        </span>
        <span class="col-model mono">{{ agent.model }}</span>
        <span class="col-depth">{{ agent.max_depth }}</span>
        <span class="col-tools">
          <span v-for="tool in agent.tools" :key="tool" class="tool-chip">{{ tool }}</span>
        </span>
        <span class="col-scope">
          <Badge :variant="getScopeColor(agent.scope)" size="sm">
            {{ agent.scope }}
          </Badge>
        </span>
      </div>

        <EmptyState
          v-if="agents.length === 0"
          icon="robot"
          title="No agents loaded"
          description="Agents will appear here once the system loads."
        />
    </div>
  </div>
</template>

<style scoped>
.agent-health {
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  overflow: hidden;
}

.health-header {
  padding: var(--space-4);
  border-bottom: 1px solid var(--border-subtle);
}

.health-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
}

.health-table {
  display: flex;
  flex-direction: column;
}

.health-row {
  display: grid;
  grid-template-columns: 120px 100px 1fr 1fr 80px;
  gap: var(--space-3);
  padding: var(--space-3) var(--space-4);
  font-size: 12px;
  align-items: center;
  border-bottom: 1px solid var(--border-subtle);
  transition: all var(--transition-fast);
  cursor: default;
}
.health-row:hover {
  background: var(--bg-hover);
  transform: translateX(2px);
}

.health-row.header {
  font-size: 10px;
  font-weight: 600;
  letter-spacing: 0.05em;
  text-transform: uppercase;
  color: var(--text-muted);
  background: var(--bg-elevated);
}

.col-agent {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  color: var(--text-primary);
  font-weight: 500;
}

.agent-avatar-sm {
  width: 24px;
  height: 24px;
  border-radius: var(--radius-sm);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 11px;
  font-weight: 600;
  background: var(--bg-elevated);
  color: var(--text-secondary);
  flex-shrink: 0;
}

.col-role {
  color: var(--text-secondary);
}

.col-model {
  color: var(--text-secondary);
  font-size: 11px;
}

.mono {
  font-family: var(--font-mono);
}

.tool-chip {
  display: inline-block;
  padding: 1px 6px;
  border-radius: var(--radius-full);
  font-size: 10px;
  background: var(--bg-elevated);
  color: var(--text-muted);
  margin-right: var(--space-1);
  transition: all var(--transition-fast);
}
.tool-chip:hover {
  background: var(--bg-hover);
  color: var(--text-secondary);
  transform: scale(1.05);
}

.health-empty {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  padding: var(--space-8);
  color: var(--text-muted);
  font-size: 13px;
}

.empty-icon {
  opacity: 0.4;
}
</style>
