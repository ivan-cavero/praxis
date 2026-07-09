<script setup lang="ts">
/**
 * AgentsView — agent management and editor.
 *
 * Features:
 * - List all agents (builtin + global + project) with scope badges
 * - Click an agent to view/edit its definition
 * - Create new agents with Markdown+YAML editor
 * - Delete agents (only project/global, not builtin)
 * - Clone a built-in to project scope for customization
 */
import { ref, computed, onMounted } from 'vue'
import { useApi, type AgentDefinition, type CreateAgentRequest } from '../composables/useApi'
import Badge from '../components/ui/Badge.vue'
import EmptyState from '../components/ui/EmptyState.vue'
import Icon from '../components/ui/Icon.vue'

const api = useApi()

const agents = ref<AgentDefinition[]>([])
const selectedAgent = ref<AgentDefinition | null>(null)
const isEditing = ref(false)
const isLoading = ref(false)
const error = ref<string | null>(null)

// Edit form state
const editForm = ref<CreateAgentRequest>({
  name: '',
  system_prompt: '',
  model: 'gpt-5',
  temperature: 0.3,
  max_tokens: 4096,
  max_turns: 25,
  max_depth: 0,
  tools: [],
  can_spawn: [],
  scope: 'project',
})

const toolsInput = ref('')
const canSpawnInput = ref('')

const isBuiltin = computed(() => selectedAgent.value?.scope === 'builtin')
const isNew = computed(() => !selectedAgent.value)

/** Names that appear in any agent's can_spawn list — these are subagents. */
const subagentNames = computed(() => {
  const names = new Set<string>()
  for (const agent of agents.value) {
    for (const spawn of agent.can_spawn) {
      names.add(spawn)
    }
  }
  return names
})

/** Main agents — not spawned by any other agent. */
const mainAgents = computed(() =>
  agents.value.filter(a => !subagentNames.value.has(a.name))
)

/** Subagents — appear in some other agent's can_spawn list. */
const subagents = computed(() =>
  agents.value.filter(a => subagentNames.value.has(a.name))
)

async function loadAgents() {
  isLoading.value = true
  error.value = null
  try {
    agents.value = await api.getAgents()
  } catch (e) {
    error.value = e instanceof Error ? e.message : 'Failed to load agents'
  }
  isLoading.value = false
}

function selectAgent(agent: AgentDefinition) {
  selectedAgent.value = agent
  isEditing.value = false
  // Populate edit form
  editForm.value = {
    name: agent.name,
    description: agent.description,
    model: agent.model,
    temperature: agent.temperature,
    max_tokens: agent.max_tokens,
    tools: [...agent.tools],
    max_turns: agent.max_turns,
    max_depth: agent.max_depth,
    can_spawn: [...agent.can_spawn],
    system_prompt: agent.system_prompt,
    scope: agent.scope === 'builtin' ? 'project' : agent.scope,
  }
  toolsInput.value = agent.tools.join(', ')
  canSpawnInput.value = agent.can_spawn.join(', ')
}

function startNewAgent() {
  selectedAgent.value = null
  isEditing.value = true
  editForm.value = {
    name: '',
    system_prompt: '',
    model: 'gpt-5',
    temperature: 0.3,
    max_tokens: 4096,
    tools: [],
    max_turns: 25,
    max_depth: 0,
    can_spawn: [],
    scope: 'project',
  }
  toolsInput.value = ''
  canSpawnInput.value = ''
}

function startEdit() {
  isEditing.value = true
}

function cancelEdit() {
  isEditing.value = false
  if (selectedAgent.value) {
    selectAgent(selectedAgent.value)
  }
}

function parseToolsInput() {
  editForm.value.tools = toolsInput.value
    .split(',')
    .map(t => t.trim())
    .filter(t => t.length > 0)
}

function parseCanSpawnInput() {
  editForm.value.can_spawn = canSpawnInput.value
    .split(',')
    .map(t => t.trim())
    .filter(t => t.length > 0)
}

async function saveAgent() {
  parseToolsInput()
  parseCanSpawnInput()

  if (!editForm.value.name || !editForm.value.system_prompt) {
    error.value = 'Name and system prompt are required'
    return
  }

  error.value = null
  try {
    if (isNew.value) {
      const created = await api.createAgent(editForm.value)
      selectedAgent.value = created
    } else if (selectedAgent.value) {
      const updated = await api.updateAgent(selectedAgent.value.name, editForm.value)
      selectedAgent.value = updated
    }
    isEditing.value = false
    await loadAgents()
  } catch (e) {
    error.value = e instanceof Error ? e.message : 'Failed to save agent'
  }
}

async function deleteAgent() {
  if (!selectedAgent.value || isBuiltin.value) return
  if (!confirm(`Delete agent "${selectedAgent.value.name}"? This cannot be undone.`)) return

  error.value = null
  try {
    await api.deleteAgent(selectedAgent.value.name)
    selectedAgent.value = null
    await loadAgents()
  } catch (e) {
    error.value = e instanceof Error ? e.message : 'Failed to delete agent'
  }
}

function scopeColor(scope: string): 'green' | 'emerald' | 'amber' | 'gray' {
  switch (scope) {
    case 'builtin': return 'gray'
    case 'global': return 'emerald'
    case 'project': return 'green'
    default: return 'amber'
  }
}

onMounted(loadAgents)
</script>

<template>
  <div class="agents-view">
    <!-- Header -->
    <div class="agents-header">
      <div class="agents-header-info">
        <h1 class="agents-title">Agents</h1>
        <p class="agents-subtitle">Manage agent definitions, system prompts, and delegation rules</p>
      </div>
      <button class="btn btn-primary" @click="startNewAgent">
        <Icon name="plus" :size="15" />
        New Agent
      </button>
    </div>

    <!-- Error banner -->
    <div v-if="error" class="error-banner">
      <Icon name="alert-triangle" :size="14" />
      {{ error }}
    </div>

    <!-- Two-panel layout -->
    <div class="agents-layout">
      <!-- ─── Agent list ───────────────────────────────── -->
      <div class="agent-list-panel">
        <template v-if="isLoading">
          <div class="agent-list-empty">
            <div class="loading-spinner" />
            <p>Loading agents...</p>
          </div>
        </template>
        <EmptyState
          v-else-if="agents.length === 0"
          icon="robot"
          title="No agents found"
          description="Create your first agent to get started."
          action-label="New Agent"
          :on-action="startNewAgent"
        />
        <div v-else class="agent-list-content">
          <!-- Main agents section -->
          <div class="agent-section">
            <div class="agent-section-header">Agents</div>
            <div
              v-for="agent in mainAgents"
              :key="agent.name"
              class="agent-item"
              :class="{ selected: selectedAgent?.name === agent.name }"
              @click="selectAgent(agent)"
            >
              <div class="agent-item-top">
                <span class="agent-item-name">{{ agent.name }}</span>
                <Badge :variant="scopeColor(agent.scope)" size="sm">{{ agent.scope }}</Badge>
              </div>
              <div class="agent-item-model">{{ agent.model }}</div>
              <div v-if="agent.description" class="agent-item-desc">{{ agent.description }}</div>
            </div>
          </div>

          <!-- Subagents section -->
          <div v-if="subagents.length > 0" class="agent-section">
            <div class="agent-section-header">Subagents</div>
            <div
              v-for="agent in subagents"
              :key="agent.name"
              class="agent-item"
              :class="{ selected: selectedAgent?.name === agent.name }"
              @click="selectAgent(agent)"
            >
              <div class="agent-item-top">
                <span class="agent-item-name">{{ agent.name }}</span>
                <Badge :variant="scopeColor(agent.scope)" size="sm">{{ agent.scope }}</Badge>
              </div>
              <div class="agent-item-model">{{ agent.model }}</div>
              <div v-if="agent.description" class="agent-item-desc">{{ agent.description }}</div>
            </div>
          </div>
        </div>
      </div>

      <!-- ─── Agent detail / editor ────────────────────── -->
      <div class="agent-detail-panel">
        <!-- View mode -->
        <div v-if="!isEditing && selectedAgent" class="agent-detail">
          <div class="detail-header">
            <div class="detail-header-info">
              <h2 class="detail-title">{{ selectedAgent.name }}</h2>
              <Badge :variant="scopeColor(selectedAgent.scope)" size="sm">{{ selectedAgent.scope }}</Badge>
            </div>
            <div class="detail-actions">
              <button class="btn btn-secondary btn-sm" @click="startEdit" :disabled="isBuiltin">
                {{ isBuiltin ? 'Clone to Edit' : 'Edit' }}
              </button>
              <button
                v-if="!isBuiltin"
                class="btn btn-ghost btn-sm detail-delete"
                @click="deleteAgent"
              >
                Delete
              </button>
            </div>
          </div>

          <div v-if="selectedAgent.description" class="detail-description">
            {{ selectedAgent.description }}
          </div>

          <div class="detail-meta">
            <div class="meta-row">
              <span class="meta-label">Model</span>
              <span class="meta-value">{{ selectedAgent.model }}</span>
            </div>
            <div class="meta-row">
              <span class="meta-label">Temperature</span>
              <span class="meta-value">{{ selectedAgent.temperature }}</span>
            </div>
            <div class="meta-row">
              <span class="meta-label">Max tokens</span>
              <span class="meta-value">{{ selectedAgent.max_tokens.toLocaleString() }}</span>
            </div>
            <div class="meta-row">
              <span class="meta-label">Max turns</span>
              <span class="meta-value">{{ selectedAgent.max_turns }}</span>
            </div>
            <div class="meta-row">
              <span class="meta-label">Max depth</span>
              <span class="meta-value">{{ selectedAgent.max_depth === 0 ? 'leaf (no delegation)' : selectedAgent.max_depth }}</span>
            </div>
            <div class="meta-row">
              <span class="meta-label">Tools</span>
              <span class="meta-value">{{ selectedAgent.tools.join(', ') || '—' }}</span>
            </div>
            <div v-if="selectedAgent.can_spawn.length > 0" class="meta-row">
              <span class="meta-label">Can spawn</span>
              <span class="meta-value">{{ selectedAgent.can_spawn.join(', ') }}</span>
            </div>
          </div>

          <div class="detail-prompt">
            <div class="prompt-label">System Prompt</div>
            <pre class="prompt-content">{{ selectedAgent.system_prompt }}</pre>
          </div>
        </div>

        <!-- Edit form -->
        <div v-else-if="isEditing" class="agent-editor">
          <h2 class="editor-title">{{ isNew ? 'New Agent' : `Edit: ${selectedAgent?.name}` }}</h2>

          <div class="form-group">
            <label class="form-label">Name</label>
            <input
              v-model="editForm.name"
              type="text"
              class="form-input"
              placeholder="my-agent"
              :disabled="!isNew"
            />
          </div>

          <div class="form-group">
            <label class="form-label">Description</label>
            <input
              v-model="editForm.description"
              type="text"
              class="form-input"
              placeholder="What does this agent do?"
            />
          </div>

          <div class="form-row">
            <div class="form-group">
              <label class="form-label">Model</label>
              <input v-model="editForm.model" type="text" class="form-input" placeholder="gpt-5" />
            </div>
            <div class="form-group">
              <label class="form-label">Temperature</label>
              <input v-model.number="editForm.temperature" type="number" step="0.1" min="0" max="2" class="form-input" />
            </div>
          </div>

          <div class="form-row">
            <div class="form-group">
              <label class="form-label">Max tokens</label>
              <input v-model.number="editForm.max_tokens" type="number" class="form-input" />
            </div>
            <div class="form-group">
              <label class="form-label">Max turns</label>
              <input v-model.number="editForm.max_turns" type="number" class="form-input" />
            </div>
            <div class="form-group">
              <label class="form-label">Max depth (0 = leaf)</label>
              <input v-model.number="editForm.max_depth" type="number" min="0" max="5" class="form-input" />
            </div>
          </div>

          <div class="form-group">
            <label class="form-label">Tools (comma-separated)</label>
            <input v-model="toolsInput" type="text" class="form-input" placeholder="filesystem, git, cargo" @blur="parseToolsInput" />
          </div>

          <div class="form-group">
            <label class="form-label">Can spawn (comma-separated, requires max_depth > 0)</label>
            <input v-model="canSpawnInput" type="text" class="form-input" placeholder="researcher, explorer" @blur="parseCanSpawnInput" />
          </div>

          <div class="form-group">
            <label class="form-label">Scope</label>
            <select v-model="editForm.scope" class="form-input">
              <option value="project">Project</option>
              <option value="global">Global</option>
            </select>
          </div>

          <div class="form-group">
            <label class="form-label">System Prompt (Markdown)</label>
            <textarea
              v-model="editForm.system_prompt"
              rows="15"
              class="form-input prompt-editor"
              placeholder="You are a ... agent. Your job is to ..."
            ></textarea>
          </div>

          <div class="editor-actions">
            <button class="btn btn-primary" @click="saveAgent">Save</button>
            <button class="btn btn-secondary" @click="cancelEdit">Cancel</button>
          </div>
        </div>

        <!-- Empty state -->
        <EmptyState
          v-else
          icon="robot"
          title="Select an agent"
          description="Choose an agent from the list, or create a new one to get started."
          action-label="New Agent"
          :on-action="startNewAgent"
        />
      </div>
    </div>
  </div>
</template>

<style scoped>
.agents-view {
  display: flex;
  flex-direction: column;
  height: 100%;
  padding: var(--space-6);
  overflow: hidden;
}

/* ─── Header ──────────────────────────────────────────────────── */

.agents-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--space-5);
  flex-shrink: 0;
}

.agents-header-info {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.agents-title {
  font-size: 22px;
  font-weight: 600;
  color: var(--text-primary);
  letter-spacing: -0.02em;
}

.agents-subtitle {
  font-size: 13px;
  color: var(--text-muted);
}

.error-banner {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-3) var(--space-4);
  border-radius: var(--radius-md);
  background: rgba(239, 68, 68, 0.1);
  color: var(--error);
  font-size: 13px;
  margin-bottom: var(--space-4);
  flex-shrink: 0;
}

/* ─── Layout ──────────────────────────────────────────────────── */

.agents-layout {
  display: grid;
  grid-template-columns: 320px 1fr;
  gap: var(--space-5);
  flex: 1;
  min-height: 0;
}

/* ─── Agent list panel ────────────────────────────────────────── */

.agent-list-panel {
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  overflow-y: auto;
  display: flex;
  flex-direction: column;
}

.agent-list-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  flex: 1;
  padding: var(--space-8);
  gap: var(--space-3);
  color: var(--text-muted);
  font-size: 13px;
}

.agent-list-content {
  display: flex;
  flex-direction: column;
}

.agent-section {
  display: flex;
  flex-direction: column;
}

.agent-section + .agent-section {
  border-top: 1px solid var(--border-subtle);
}

.agent-section-header {
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-muted);
  padding: var(--space-3) var(--space-4);
  position: sticky;
  top: 0;
  background: var(--bg-surface);
  z-index: 1;
}

.agent-item {
  padding: var(--space-3) var(--space-4);
  cursor: pointer;
  transition: background var(--transition-fast);
  border-left: 3px solid transparent;
}

.agent-item:hover {
  background: var(--bg-hover);
}

.agent-item.selected {
  background: var(--bg-elevated);
  border-left-color: var(--primary);
}

.agent-item-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
}

.agent-item-name {
  font-size: 14px;
  font-weight: 500;
  color: var(--text-primary);
}

.agent-item-model {
  font-size: 11px;
  color: var(--text-muted);
  font-family: var(--font-mono);
  margin-top: 2px;
}

.agent-item-desc {
  font-size: 12px;
  color: var(--text-secondary);
  margin-top: var(--space-1);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

/* ─── Detail panel ────────────────────────────────────────────── */

.agent-detail-panel {
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  overflow-y: auto;
  padding: var(--space-6);
}

.detail-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--space-4);
}

.detail-header-info {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.detail-title {
  font-size: 20px;
  font-weight: 600;
  color: var(--text-primary);
  letter-spacing: -0.01em;
}

.detail-actions {
  display: flex;
  gap: var(--space-2);
}

.detail-delete {
  color: var(--error);
}

.detail-delete:hover {
  background: rgba(239, 68, 68, 0.1);
}

.detail-description {
  font-size: 14px;
  color: var(--text-secondary);
  margin-bottom: var(--space-5);
  line-height: 1.5;
}

.detail-meta {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  margin-bottom: var(--space-5);
  padding: var(--space-4);
  background: var(--bg-elevated);
  border-radius: var(--radius-md);
}

.meta-row {
  display: flex;
  align-items: center;
  gap: var(--space-4);
}

.meta-label {
  width: 120px;
  font-size: 12px;
  text-transform: uppercase;
  letter-spacing: 0.03em;
  color: var(--text-muted);
  flex-shrink: 0;
}

.meta-value {
  font-size: 14px;
  color: var(--text-primary);
  font-family: var(--font-mono);
}

.detail-prompt {
  margin-top: var(--space-2);
}

.prompt-label {
  font-size: 11px;
  font-weight: 600;
  text-transform: uppercase;
  letter-spacing: 0.05em;
  color: var(--text-muted);
  margin-bottom: var(--space-2);
}

.prompt-content {
  background: var(--bg-base);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  padding: var(--space-4);
  font-family: var(--font-mono);
  font-size: 13px;
  line-height: 1.6;
  white-space: pre-wrap;
  word-break: break-word;
  max-height: 400px;
  overflow-y: auto;
  color: var(--text-secondary);
}

/* ─── Editor ──────────────────────────────────────────────────── */

.agent-editor {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.editor-title {
  font-size: 20px;
  font-weight: 600;
  color: var(--text-primary);
  letter-spacing: -0.01em;
  margin-bottom: var(--space-2);
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.form-label {
  font-size: 12px;
  font-weight: 500;
  text-transform: uppercase;
  letter-spacing: 0.03em;
  color: var(--text-muted);
}

.form-input {
  padding: var(--space-2) var(--space-3);
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  color: var(--text-primary);
  font-size: 13px;
  font-family: inherit;
  outline: none;
  transition: border-color var(--transition-fast);
}

.form-input:focus {
  border-color: var(--primary);
  box-shadow: 0 0 0 2px var(--primary-muted);
}

.form-input:disabled {
  opacity: 0.5;
}

.form-input::placeholder {
  color: var(--text-muted);
}

.form-row {
  display: grid;
  grid-template-columns: 1fr 1fr 1fr;
  gap: var(--space-3);
}

.prompt-editor {
  font-family: var(--font-mono);
  resize: vertical;
  min-height: 200px;
  line-height: 1.5;
}

.editor-actions {
  display: flex;
  gap: var(--space-2);
  margin-top: var(--space-2);
}

/* ─── Empty state ─────────────────────────────────────────────── */

.empty-detail {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: var(--text-muted);
  gap: var(--space-3);
  font-size: 14px;
}

.empty-icon {
  opacity: 0.3;
}

/* ─── Responsive ──────────────────────────────────────────────── */

@media (max-width: 1023px) {
  .agents-layout {
    grid-template-columns: 280px 1fr;
  }
}

@media (max-width: 767px) {
  .agents-view {
    padding: var(--space-4);
  }

  .agents-layout {
    grid-template-columns: 1fr;
    grid-template-rows: auto 1fr;
  }

  .agent-list-panel {
    max-height: 200px;
  }

  .form-row {
    grid-template-columns: 1fr;
  }
}
</style>
