<script setup lang="ts">
/**
 * AgentDebugView — Inspect agent definitions and raw configuration.
 *
 * Shows:
 * - All registered agents with their model, tools, and scope
 * - Detailed view of each agent's system prompt and config
 * - Token/cost breakdown per agent (when available)
 */
import { ref, computed, onMounted } from 'vue'
import { useApi, type AgentDefinition } from '../composables/useApi'
import Icon from '../components/ui/Icon.vue'
import EmptyState from '../components/ui/EmptyState.vue'

const api = useApi()

const agents = ref<AgentDefinition[]>([])
const isLoading = ref(true)
const selectedAgentName = ref<string | null>(null)
const searchQuery = ref('')

const filteredAgents = computed(() => {
  if (!searchQuery.value.trim()) return agents.value
  const q = searchQuery.value.toLowerCase()
  return agents.value.filter(a =>
    a.name.toLowerCase().includes(q) ||
    a.model.toLowerCase().includes(q) ||
    a.description.toLowerCase().includes(q)
  )
})

const selectedAgent = computed(() =>
  agents.value.find(a => a.name === selectedAgentName.value) ?? null
)

function loadAgents() {
  isLoading.value = true
  api.getAgents()
    .then(data => { agents.value = data })
    .catch(() => { /* Error loading agents */ })
    .finally(() => { isLoading.value = false })
}

function selectAgent(name: string) {
  selectedAgentName.value = selectedAgentName.value === name ? null : name
}


onMounted(() => {
  loadAgents()
})
</script>

<template>
  <div class="agent-debug-view">
    <div class="ad-header">
      <h1 class="ad-title">Agent Debug</h1>
      <div class="ad-controls">
        <input
          v-model="searchQuery"
          class="search-input"
          placeholder="Search agents..."
          aria-label="Search agents"
        />
        <button class="refresh-btn" @click="loadAgents" :disabled="isLoading" aria-label="Refresh agents">
          <Icon v-if="isLoading" name="refresh" :size="14" class="animate-spin" />
          <Icon v-else name="refresh" :size="14" />
        </button>
      </div>
    </div>

    <EmptyState
      v-if="!isLoading && filteredAgents.length === 0"
      icon="robot"
      title="No agents found"
      description="Create agents with 'praxis agent add' or via the Agents view."
    />

    <div v-else class="agent-layout">
      <!-- Agent list -->
      <div class="agent-list">
        <div
          v-for="agent in filteredAgents"
          :key="agent.name"
          class="agent-card"
          :class="{ selected: selectedAgentName === agent.name }"
          @click="selectAgent(agent.name)"
        >
          <div class="agent-card-header">
            <Icon name="robot" :size="16" />
            <span class="agent-name">{{ agent.name }}</span>
            <span class="agent-scope">{{ agent.scope }}</span>
          </div>
          <div class="agent-card-meta">
            <span class="meta-model">{{ agent.model }}</span>
            <span class="meta-tools">{{ agent.tools.length }} tools</span>
          </div>
        </div>
      </div>

      <!-- Detail panel -->
      <div v-if="selectedAgent" class="agent-detail">
        <div class="detail-header">
          <h2 class="detail-name">{{ selectedAgent.name }}</h2>
          <span class="detail-scope">{{ selectedAgent.scope }}</span>
        </div>
        <p v-if="selectedAgent.description" class="detail-desc">{{ selectedAgent.description }}</p>

        <div class="detail-grid">
          <div class="detail-item">
            <span class="detail-key">Model</span>
            <span class="detail-val mono">{{ selectedAgent.model }}</span>
          </div>
          <div class="detail-item">
            <span class="detail-key">Temperature</span>
            <span class="detail-val mono">{{ selectedAgent.temperature }}</span>
          </div>
          <div class="detail-item">
            <span class="detail-key">Max Tokens</span>
            <span class="detail-val mono">{{ selectedAgent.max_tokens }}</span>
          </div>
          <div class="detail-item">
            <span class="detail-key">Max Turns</span>
            <span class="detail-val mono">{{ selectedAgent.max_turns }}</span>
          </div>
          <div class="detail-item">
            <span class="detail-key">Max Depth</span>
            <span class="detail-val mono">{{ selectedAgent.max_depth }}</span>
          </div>
        </div>

        <div class="detail-section">
          <h3 class="detail-section-title">Tools</h3>
          <div class="tag-list">
            <span v-for="tool in selectedAgent.tools" :key="tool" class="tag">{{ tool }}</span>
            <span v-if="selectedAgent.tools.length === 0" class="no-tags">No tools configured</span>
          </div>
        </div>

        <div class="detail-section">
          <h3 class="detail-section-title">Can Spawn</h3>
          <div class="tag-list">
            <span v-for="spawn in selectedAgent.can_spawn" :key="spawn" class="tag tag-spawn">{{ spawn }}</span>
            <span v-if="selectedAgent.can_spawn.length === 0" class="no-tags">Cannot spawn sub-agents</span>
          </div>
        </div>

        <div class="detail-section">
          <h3 class="detail-section-title">System Prompt</h3>
          <pre class="prompt-text">{{ selectedAgent.system_prompt }}</pre>
        </div>
      </div>
      <div v-else class="detail-placeholder">
        <Icon name="robot" :size="32" />
        <p>Select an agent to inspect its configuration</p>
      </div>
    </div>
  </div>
</template>

<style scoped>
.agent-debug-view {
  padding: var(--space-4);
  height: 100%;
  overflow-y: auto;
  background: var(--bg-base);
}

.ad-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--space-4);
}

.ad-title {
  font-size: 20px;
  font-weight: 600;
  color: var(--text-primary);
}

.ad-controls {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.search-input {
  padding: 4px 10px;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: var(--bg-surface);
  color: var(--text-primary);
  font-size: 12px;
  width: 200px;
}
.search-input:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 1px;
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

.agent-layout {
  display: flex;
  gap: var(--space-4);
  height: calc(100% - 60px);
}

.agent-list {
  width: 280px;
  flex-shrink: 0;
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  overflow-y: auto;
}

.agent-card {
  padding: var(--space-2) var(--space-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: var(--bg-surface);
  cursor: pointer;
  transition: border-color 0.15s;
}
.agent-card:hover { border-color: var(--text-muted); }
.agent-card.selected {
  border-color: var(--accent);
  background: var(--bg-elevated);
}

.agent-card-header {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin-bottom: 4px;
}

.agent-name {
  flex: 1;
  font-size: 13px;
  font-weight: 500;
  color: var(--text-primary);
}

.agent-scope {
  font-size: 10px;
  text-transform: uppercase;
  color: var(--text-muted);
  padding: 1px 4px;
  border-radius: var(--radius-sm);
  background: var(--bg-elevated);
}

.agent-card-meta {
  display: flex;
  gap: var(--space-2);
  font-size: 11px;
  color: var(--text-muted);
}

.meta-model {
  font-family: var(--font-mono, monospace);
}

.agent-detail {
  flex: 1;
  overflow-y: auto;
  padding: var(--space-3);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  background: var(--bg-surface);
}

.detail-header {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin-bottom: var(--space-2);
}

.detail-name {
  font-size: 18px;
  font-weight: 600;
  color: var(--text-primary);
}

.detail-scope {
  font-size: 11px;
  text-transform: uppercase;
  color: var(--text-muted);
  padding: 2px 6px;
  border-radius: var(--radius-sm);
  background: var(--bg-elevated);
}

.detail-desc {
  font-size: 13px;
  color: var(--text-secondary);
  margin-bottom: var(--space-3);
}

.detail-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
  gap: var(--space-2);
  margin-bottom: var(--space-4);
}

.detail-item {
  padding: var(--space-2);
  background: var(--bg-elevated);
  border-radius: var(--radius-md);
}

.detail-key {
  display: block;
  font-size: 10px;
  text-transform: uppercase;
  color: var(--text-muted);
  letter-spacing: 0.05em;
  margin-bottom: 2px;
}

.detail-val {
  font-size: 13px;
  color: var(--text-primary);
}

.mono {
  font-family: var(--font-mono, monospace);
}

.detail-section {
  margin-bottom: var(--space-4);
}

.detail-section-title {
  font-size: 12px;
  font-weight: 600;
  text-transform: uppercase;
  color: var(--text-muted);
  letter-spacing: 0.05em;
  margin-bottom: var(--space-2);
}

.tag-list {
  display: flex;
  flex-wrap: wrap;
  gap: 4px;
}

.tag {
  padding: 2px 8px;
  border-radius: var(--radius-sm);
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  font-size: 11px;
  font-family: var(--font-mono, monospace);
  color: var(--text-secondary);
}

.tag-spawn {
  border-color: var(--accent);
  color: var(--accent);
}

.no-tags {
  font-size: 12px;
  color: var(--text-muted);
  font-style: italic;
}

.prompt-text {
  padding: var(--space-3);
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  font-size: 12px;
  line-height: 1.5;
  color: var(--text-secondary);
  font-family: var(--font-mono, monospace);
  white-space: pre-wrap;
  word-break: break-word;
  max-height: 400px;
  overflow-y: auto;
}

.detail-placeholder {
  flex: 1;
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  color: var(--text-muted);
  font-size: 13px;
}

.animate-spin {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  from { transform: rotate(0deg); }
  to { transform: rotate(360deg); }
}
</style>
