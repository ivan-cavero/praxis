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
    <div class="agents-header">
      <h1 class="title">Agents</h1>
      <button class="btn-primary" @click="startNewAgent">
        + New Agent
      </button>
    </div>

    <div v-if="error" class="error-banner">
      {{ error }}
    </div>

    <div class="agents-layout">
      <!-- Agent list -->
      <div class="agent-list-panel">
        <div v-if="isLoading" class="loading">Loading...</div>
        <div v-else-if="agents.length === 0" class="empty">No agents found.</div>
        <div v-else class="agent-items">
          <div
            v-for="agent in agents"
            :key="agent.name"
            class="agent-item"
            :class="{ selected: selectedAgent?.name === agent.name }"
            @click="selectAgent(agent)"
          >
            <div class="agent-item-name">{{ agent.name }}</div>
            <div class="agent-item-meta">
              <span class="agent-item-model">{{ agent.model }}</span>
              <Badge :variant="scopeColor(agent.scope)" size="sm">{{ agent.scope }}</Badge>
            </div>
            <div class="agent-item-desc">{{ agent.description }}</div>
            <div v-if="agent.max_depth > 0" class="agent-item-spawn">
              can spawn: {{ agent.can_spawn.join(', ') }}
            </div>
          </div>
        </div>
      </div>

      <!-- Agent detail / editor -->
      <div class="agent-detail-panel">
        <div v-if="!isEditing && selectedAgent" class="agent-detail">
          <div class="detail-header">
            <h2 class="detail-title">{{ selectedAgent.name }}</h2>
            <div class="detail-actions">
              <button class="btn-secondary" @click="startEdit" :disabled="isBuiltin">
                {{ isBuiltin ? 'Clone to Edit' : 'Edit' }}
              </button>
              <button
                class="btn-danger"
                @click="deleteAgent"
                :disabled="isBuiltin"
                v-if="!isBuiltin"
              >
                Delete
              </button>
            </div>
          </div>

          <div class="detail-meta">
            <div class="meta-row">
              <span class="meta-label">Scope</span>
              <Badge :variant="scopeColor(selectedAgent.scope)" size="sm">{{ selectedAgent.scope }}</Badge>
            </div>
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
              <span class="meta-value">{{ selectedAgent.max_tokens }}</span>
            </div>
            <div class="meta-row">
              <span class="meta-label">Max turns</span>
              <span class="meta-value">{{ selectedAgent.max_turns }}</span>
            </div>
            <div class="meta-row">
              <span class="meta-label">Max depth</span>
              <span class="meta-value">{{ selectedAgent.max_depth }}</span>
            </div>
            <div class="meta-row">
              <span class="meta-label">Tools</span>
              <span class="meta-value">{{ selectedAgent.tools.join(', ') || '—' }}</span>
            </div>
            <div class="meta-row">
              <span class="meta-label">Can spawn</span>
              <span class="meta-value">{{ selectedAgent.can_spawn.join(', ') || '—' }}</span>
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
            <label>Name</label>
            <input
              v-model="editForm.name"
              type="text"
              placeholder="my-agent"
              :disabled="!isNew"
            />
          </div>

          <div class="form-group">
            <label>Description</label>
            <input
              v-model="editForm.description"
              type="text"
              placeholder="What does this agent do?"
            />
          </div>

          <div class="form-row">
            <div class="form-group">
              <label>Model</label>
              <input v-model="editForm.model" type="text" placeholder="gpt-5" />
            </div>
            <div class="form-group">
              <label>Temperature</label>
              <input v-model.number="editForm.temperature" type="number" step="0.1" min="0" max="2" />
            </div>
          </div>

          <div class="form-row">
            <div class="form-group">
              <label>Max tokens</label>
              <input v-model.number="editForm.max_tokens" type="number" />
            </div>
            <div class="form-group">
              <label>Max turns</label>
              <input v-model.number="editForm.max_turns" type="number" />
            </div>
            <div class="form-group">
              <label>Max depth (0 = leaf)</label>
              <input v-model.number="editForm.max_depth" type="number" min="0" max="5" />
            </div>
          </div>

          <div class="form-group">
            <label>Tools (comma-separated)</label>
            <input v-model="toolsInput" type="text" placeholder="filesystem, git, cargo" @blur="parseToolsInput" />
          </div>

          <div class="form-group">
            <label>Can spawn (comma-separated, requires max_depth > 0)</label>
            <input v-model="canSpawnInput" type="text" placeholder="researcher, explorer" @blur="parseCanSpawnInput" />
          </div>

          <div class="form-group">
            <label>Scope</label>
            <select v-model="editForm.scope">
              <option value="project">Project</option>
              <option value="global">Global</option>
            </select>
          </div>

          <div class="form-group">
            <label>System Prompt (Markdown)</label>
            <textarea
              v-model="editForm.system_prompt"
              rows="15"
              placeholder="You are a ... agent. Your job is to ..."
              class="prompt-editor"
            ></textarea>
          </div>

          <div class="editor-actions">
            <button class="btn-primary" @click="saveAgent">Save</button>
            <button class="btn-secondary" @click="cancelEdit">Cancel</button>
          </div>
        </div>

        <!-- Empty state -->
        <div v-else class="empty-detail">
          <Icon name="robot" :size="48" class="empty-icon" />
          <p>Select an agent from the list, or create a new one.</p>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.agents-view {
  padding: 24px;
  max-width: 1400px;
  margin: 0 auto;
}

.agents-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 24px;
}

.title {
  font-size: 24px;
  font-weight: 700;
}

.btn-primary, .btn-secondary, .btn-danger {
  padding: 6px 16px;
  border-radius: 6px;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  border: 1px solid var(--border-color, #333);
}

.btn-primary {
  background: var(--accent, #6a9fd6);
  color: white;
  border-color: var(--accent, #6a9fd6);
}

.btn-secondary {
  background: var(--bg-elevated, #2a2a3e);
  color: var(--text-primary, #eee);
}

.btn-danger {
  background: var(--danger, #c44);
  color: white;
  border-color: var(--danger, #c44);
}

.btn-primary:disabled, .btn-secondary:disabled, .btn-danger:disabled {
  opacity: 0.4;
  cursor: not-allowed;
}

.error-banner {
  padding: 8px 12px;
  background: rgba(200, 60, 60, 0.1);
  border: 1px solid rgba(200, 60, 60, 0.3);
  border-radius: 6px;
  color: #ff6666;
  margin-bottom: 16px;
  font-size: 13px;
}

.agents-layout {
  display: grid;
  grid-template-columns: 350px 1fr;
  gap: 16px;
  min-height: 600px;
}

.agent-list-panel {
  border: 1px solid var(--border-color, #333);
  border-radius: 8px;
  overflow-y: auto;
  max-height: 700px;
}

.loading, .empty {
  padding: 24px;
  text-align: center;
  color: var(--text-muted, #666);
}

.agent-item {
  padding: 12px 16px;
  border-bottom: 1px solid var(--border-color, #2a2a3e);
  cursor: pointer;
  transition: background 0.15s;
}

.agent-item:hover {
  background: var(--bg-elevated, #1a1a2e);
}

.agent-item.selected {
  background: rgba(106, 159, 214, 0.1);
  border-left: 3px solid var(--accent, #6a9fd6);
}

.agent-item-name {
  font-weight: 600;
  font-size: 14px;
  color: var(--text-primary, #eee);
}

.agent-item-meta {
  display: flex;
  align-items: center;
  gap: 8px;
  margin-top: 4px;
}

.agent-item-model {
  font-size: 11px;
  color: var(--text-muted, #888);
  font-family: monospace;
}

.agent-item-desc {
  font-size: 12px;
  color: var(--text-secondary, #aaa);
  margin-top: 4px;
}

.agent-item-spawn {
  font-size: 11px;
  color: var(--text-accent, #6a9fd6);
  margin-top: 2px;
}

.agent-detail-panel {
  border: 1px solid var(--border-color, #333);
  border-radius: 8px;
  padding: 20px;
  overflow-y: auto;
  max-height: 700px;
}

.detail-header {
  display: flex;
  justify-content: space-between;
  align-items: center;
  margin-bottom: 20px;
}

.detail-title {
  font-size: 20px;
  font-weight: 700;
}

.detail-actions {
  display: flex;
  gap: 8px;
}

.detail-meta {
  display: flex;
  flex-direction: column;
  gap: 8px;
  margin-bottom: 20px;
}

.meta-row {
  display: flex;
  align-items: center;
  gap: 12px;
}

.meta-label {
  width: 120px;
  font-size: 12px;
  text-transform: uppercase;
  color: var(--text-muted, #666);
}

.meta-value {
  font-size: 14px;
  color: var(--text-primary, #eee);
}

.detail-prompt {
  margin-top: 16px;
}

.prompt-label {
  font-size: 12px;
  text-transform: uppercase;
  color: var(--text-muted, #666);
  margin-bottom: 8px;
}

.prompt-content {
  background: var(--bg-tertiary, #12121f);
  border: 1px solid var(--border-color, #333);
  border-radius: 6px;
  padding: 12px;
  font-family: 'Fira Code', monospace;
  font-size: 13px;
  white-space: pre-wrap;
  word-break: break-word;
  max-height: 400px;
  overflow-y: auto;
  color: var(--text-primary, #ddd);
}

.agent-editor {
  display: flex;
  flex-direction: column;
  gap: 12px;
}

.editor-title {
  font-size: 20px;
  font-weight: 700;
  margin-bottom: 8px;
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.form-group label {
  font-size: 12px;
  text-transform: uppercase;
  color: var(--text-muted, #666);
}

.form-group input, .form-group select, .form-group textarea {
  padding: 6px 10px;
  background: var(--bg-tertiary, #12121f);
  border: 1px solid var(--border-color, #333);
  border-radius: 4px;
  color: var(--text-primary, #eee);
  font-size: 13px;
}

.form-group input:disabled {
  opacity: 0.5;
}

.form-row {
  display: grid;
  grid-template-columns: 1fr 1fr 1fr;
  gap: 12px;
}

.prompt-editor {
  font-family: 'Fira Code', monospace;
  resize: vertical;
  min-height: 200px;
}

.editor-actions {
  display: flex;
  gap: 8px;
  margin-top: 12px;
}

.empty-detail {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  height: 100%;
  color: var(--text-muted, #666);
  gap: 12px;
}

.empty-icon {
  opacity: 0.3;
}
</style>
