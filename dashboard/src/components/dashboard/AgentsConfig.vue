<script setup lang="ts">
/**
 * AgentsConfig — Configure agent roles (models, prompts, tools).
 *
 * Reads/writes agent role settings via the project config (forge.toml) API.
 */

import { ref, computed, onMounted } from 'vue'
import { useApi, type Project, type ProjectConfig, type RoleDetail, type AgentDefinition } from '../../composables/useApi'
import Icon from '../ui/Icon.vue'

const api = useApi()

// ─── State ──────────────────────────────────────────────────────

const projects = ref<Project[]>([])
const selectedProjectId = ref<string | null>(null)
const config = ref<ProjectConfig | null>(null)
const isSaving = ref(false)
const saveMessage = ref<string | null>(null)
const expandedAgent = ref<string | null>(null)
const isLoading = ref(false)
const agentDefinitions = ref<AgentDefinition[]>([])

// Icon mapping for known agent types (agent .md files don't carry icons)
const AGENT_ICONS: Record<string, string> = {
  architect: 'brain',
  coder: 'code',
  reviewer: 'eye',
  security: 'shield',
  tester: 'check',
  git: 'terminal',
  researcher: 'search',
  explorer: 'compass',
}

/** Build known roles from agent definitions loaded from the API. */
const knownRoles = computed<Record<string, { label: string; description: string; icon: string }>>(() => {
  const roles: Record<string, { label: string; description: string; icon: string }> = {}
  for (const def of agentDefinitions.value) {
    roles[def.name] = {
      label: def.name.charAt(0).toUpperCase() + def.name.slice(1),
      description: def.description || '',
      icon: AGENT_ICONS[def.name] || 'robot',
    }
  }
  return roles
})

const agentKeys = computed(() => Object.keys(knownRoles.value))

/** Map agent name → system prompt from the .md definitions. */
const systemPromptDefaults = computed<Record<string, string>>(() => {
  const map: Record<string, string> = {}
  for (const def of agentDefinitions.value) {
    map[def.name] = def.system_prompt
  }
  return map
})

/** Current roles from config, or empty object if not loaded. */
const roles = computed<Record<string, RoleDetail>>(() =>
  config.value?.roles ?? {}
)

function getRole(agentKey: string): RoleDetail | null {
  return roles.value[agentKey] ?? null
}

// ─── Loading ────────────────────────────────────────────────────

async function loadProjects() {
  try {
    projects.value = await api.getProjects()
  } catch {
    // silent
  }
}

async function loadConfig() {
  if (!selectedProjectId.value) return
  isLoading.value = true
  try {
    const cfg = await api.getProjectConfig(selectedProjectId.value)
    config.value = cfg
  } catch {
    config.value = null
  }
  isLoading.value = false
}

// ─── Editing helpers ────────────────────────────────────────────

function updateRoleField(agentKey: string, field: keyof RoleDetail, value: string | number | string[]) {
  if (!config.value) return
  const updated = { ...(config.value.roles[agentKey] ?? createDefaultRole(agentKey)) }
  ;(updated as any)[field] = value
  config.value = {
    ...config.value,
    roles: { ...config.value.roles, [agentKey]: updated },
  }
}

function createDefaultRole(agentKey: string): RoleDetail {
  const known = knownRoles.value[agentKey]
  const defaultPrompt = systemPromptDefaults.value[agentKey]
    || `You are the ${known?.label ?? agentKey} agent. ${known?.description ?? ''}`
  return {
    model: 'gpt-4o',
    temperature: 0.7,
    max_tokens: 4096,
    system_prompt: defaultPrompt,
    tools: [],
    description: known?.description ?? '',
  }
}

/** Add a role that doesn't exist yet (initializes with defaults). */
function addRole(agentKey: string) {
  if (!config.value) return
  config.value = {
    ...config.value,
    roles: { ...config.value.roles, [agentKey]: createDefaultRole(agentKey) },
  }
  expandedAgent.value = agentKey
}

/** Remove a role from config. */
function removeRole(agentKey: string) {
  if (!config.value) return
  const updated = { ...config.value.roles }
  delete updated[agentKey]
  config.value = {
    ...config.value,
    roles: updated,
  }
}

// ─── Saving ─────────────────────────────────────────────────────

/** Serialize the current roles object back into the raw TOML config. */
function serializeRolesToToml(raw: string, roles: Record<string, RoleDetail>): string {
  // Remove existing [roles.*] sections from raw TOML
  const lines = raw.split('\n')
  const cleanedLines: string[] = []
  let inRolesSection = false
  for (const line of lines) {
    if (/^\[roles\./.test(line.trim())) {
      inRolesSection = true
      continue
    }
    if (inRolesSection) {
      if (line.trim().startsWith('[') || line.trim() === '') {
        inRolesSection = false
        // Don't skip — this blank line or next section should be kept
      } else {
        continue
      }
    }
    cleanedLines.push(line)
  }

  // Append the new roles section at the end
  const roleLines: string[] = []
  for (const [key, role] of Object.entries(roles)) {
    roleLines.push(`[roles.${key}]`)
    roleLines.push(`model = "${role.model}"`)
    roleLines.push(`temperature = ${role.temperature}`)
    roleLines.push(`max_tokens = ${role.max_tokens}`)
    roleLines.push(`system_prompt = """${role.system_prompt}"""`)
    if (role.tools.length > 0) {
      roleLines.push(`tools = [${role.tools.map(t => `"${t}"`).join(', ')}]`)
    }
    roleLines.push(`description = "${role.description}"`)
    roleLines.push('')
  }

  return [...cleanedLines, '', ...roleLines].join('\n')
}

async function saveConfig() {
  if (!selectedProjectId.value || !config.value) return
  isSaving.value = true
  saveMessage.value = null
  try {
    // Serialize roles back to TOML and update the config
    const updatedToml = serializeRolesToToml(config.value.raw, config.value.roles)
    const result = await api.updateProjectConfig(selectedProjectId.value, updatedToml)
    config.value = result
    saveMessage.value = 'Agent settings saved'
  } catch (cause: any) {
    saveMessage.value = `Error: ${cause.message}`
  } finally {
    isSaving.value = false
    setTimeout(() => { saveMessage.value = null }, 3000)
  }
}

onMounted(() => {
  loadProjects()
  loadAgentDefinitions()
})

async function loadAgentDefinitions() {
  try {
    agentDefinitions.value = await api.getAgents()
  } catch {
    // Agent definitions endpoint might not be available — fall back to empty
  }
}
</script>

<template>
  <div class="agents-config">
    <div class="config-header">
      <h3 class="config-title">Agent Roles</h3>
      <p class="config-subtitle">Configure each agent's model, system prompt, and tools.</p>
    </div>

    <!-- Project selector -->
    <div class="config-selector">
      <select v-model="selectedProjectId" class="project-select" @change="loadConfig">
        <option value="" disabled>Select a project...</option>
        <option v-for="project in projects" :key="project.id" :value="project.id">
          {{ project.name }}
        </option>
      </select>
    </div>

    <!-- Loading state -->
    <div v-if="isLoading" class="loading-state">
      <Icon name="refresh" :size="18" class="spin" />
      <span>Loading agent configuration...</span>
    </div>

    <!-- Agent cards -->
    <div v-else-if="config" class="agents-list">
      <div
        v-for="agentKey in agentKeys"
        :key="agentKey"
        class="agent-card"
        :class="{ expanded: expandedAgent === agentKey }"
      >
        <!-- Header -->
        <div
          class="agent-header"
          @click="expandedAgent = expandedAgent === agentKey ? null : agentKey"
        >
          <div class="agent-info">
            <Icon :name="knownRoles[agentKey].icon" :size="18" class="agent-icon" />
            <div class="agent-meta">
              <span class="agent-name">{{ knownRoles[agentKey].label }}</span>
              <span class="agent-model" v-if="getRole(agentKey)">
                {{ getRole(agentKey)!.model }}
              </span>
              <span class="agent-model absent" v-else>Not configured</span>
            </div>
          </div>
          <div class="agent-header-actions">
            <button
              v-if="getRole(agentKey)"
              class="btn-remove"
              title="Remove role"
              @click.stop="removeRole(agentKey)"
            >
              <Icon name="x" :size="14" />
            </button>
            <Icon name="chevron-right" :size="16" class="chevron" />
          </div>
        </div>

        <!-- Expanded detail -->
        <div v-if="expandedAgent === agentKey" class="agent-detail">
          <div v-if="!getRole(agentKey)" class="add-role-prompt">
            <p>This agent is not configured. Add it with default settings?</p>
            <button class="btn-add-role" @click="addRole(agentKey)">Add agent</button>
          </div>

          <template v-if="getRole(agentKey)">
            <div class="field-row">
              <div class="field">
                <label class="field-label">Model</label>
                <input
                  class="field-input"
                  :value="getRole(agentKey)!.model"
                  @input="(e) => updateRoleField(agentKey, 'model', (e.target as HTMLInputElement).value)"
                  placeholder="gpt-4o, claude-3-opus, etc."
                />
              </div>
              <div class="field">
                <label class="field-label">Temperature</label>
                <input
                  class="field-input field-input-narrow"
                  type="number"
                  min="0"
                  max="2"
                  step="0.1"
                  :value="getRole(agentKey)!.temperature"
                  @input="(e) => updateRoleField(agentKey, 'temperature', parseFloat((e.target as HTMLInputElement).value) || 0)"
                />
              </div>
              <div class="field">
                <label class="field-label">Max Tokens</label>
                <input
                  class="field-input field-input-narrow"
                  type="number"
                  min="1"
                  step="1"
                  :value="getRole(agentKey)!.max_tokens"
                  @input="(e) => updateRoleField(agentKey, 'max_tokens', parseInt((e.target as HTMLInputElement).value) || 0)"
                />
              </div>
            </div>

            <div class="field">
              <label class="field-label">System Prompt</label>
              <textarea
                class="field-textarea"
                rows="4"
                :value="getRole(agentKey)!.system_prompt"
                @input="(e) => updateRoleField(agentKey, 'system_prompt', (e.target as HTMLTextAreaElement).value)"
                placeholder="Instructions for this agent..."
              />
            </div>

            <div class="field">
              <label class="field-label">Tools (comma-separated)</label>
              <input
                class="field-input"
                :value="(getRole(agentKey)!.tools || []).join(', ')"
                @input="(e) => updateRoleField(agentKey, 'tools', (e.target as HTMLInputElement).value.split(',').map(t => t.trim()).filter(Boolean))"
                placeholder="filesystem, git, github, web-search"
              />
            </div>
          </template>
        </div>
      </div>

      <!-- Save button -->
      <div class="save-bar">
        <button
          class="btn-save"
          :disabled="isSaving"
          @click="saveConfig()"
        >
          <Icon v-if="isSaving" name="refresh" :size="16" class="spin" />
          <span>{{ isSaving ? 'Saving...' : 'Save Agent Settings' }}</span>
        </button>
        <span v-if="saveMessage" class="save-message" :class="{ error: saveMessage.startsWith('Error') }">
          {{ saveMessage }}
        </span>
      </div>
    </div>

    <!-- No config state -->
    <div v-else-if="selectedProjectId && !isLoading" class="empty-state">
      <p>Could not load configuration for this project.</p>
    </div>

    <!-- No project selected -->
    <div v-else-if="!selectedProjectId" class="empty-state">
      <Icon name="robot" :size="32" class="empty-icon" />
      <p>Select a project to configure agent roles.</p>
    </div>
  </div>
</template>

<style scoped>
.agents-config {
  padding: var(--space-4);
}

.config-header {
  margin-bottom: var(--space-4);
}

.config-title {
  font-size: 16px;
  font-weight: 600;
  color: var(--text-primary);
  margin: 0;
}

.config-subtitle {
  font-size: 13px;
  color: var(--text-muted);
  margin: var(--space-1) 0 0;
}

.config-selector {
  margin-bottom: var(--space-4);
}

.project-select {
  width: 100%;
  max-width: 320px;
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-md);
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  color: var(--text-primary);
  font-size: 13px;
  font-family: inherit;
  cursor: pointer;
}

.project-select:focus {
  outline: none;
  border-color: var(--primary);
}

/* Loading */
.loading-state {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-8);
  color: var(--text-muted);
  font-size: 13px;
  justify-content: center;
}

.spin {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

/* Agent cards */
.agents-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.agent-card {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: var(--bg-elevated);
  overflow: hidden;
  transition: border-color var(--transition-fast);
}

.agent-card.expanded {
  border-color: var(--border-default);
}

.agent-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-3) var(--space-4);
  cursor: pointer;
  user-select: none;
  transition: background var(--transition-fast);
}

.agent-header:hover {
  background: var(--bg-hover);
}

.agent-info {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.agent-icon {
  color: var(--text-secondary);
  flex-shrink: 0;
}

.agent-meta {
  display: flex;
  flex-direction: column;
  gap: 1px;
}

.agent-name {
  font-size: 13px;
  font-weight: 500;
  color: var(--text-primary);
}

.agent-model {
  font-size: 11px;
  font-family: var(--font-mono);
  color: var(--text-muted);
}

.agent-model.absent {
  color: var(--text-disabled);
  font-style: italic;
}

.agent-header-actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.btn-remove {
  padding: var(--space-1);
  border: none;
  background: transparent;
  color: var(--text-disabled);
  cursor: pointer;
  border-radius: var(--radius-sm);
  transition: all var(--transition-fast);
}

.btn-remove:hover {
  color: var(--error, #ef4444);
  background: rgba(239, 68, 68, 0.1);
}

.chevron {
  color: var(--text-disabled);
  transition: transform var(--transition-fast);
}

.agent-card.expanded .chevron {
  transform: rotate(90deg);
}

/* Detail panel */
.agent-detail {
  padding: 0 var(--space-4) var(--space-4);
  border-top: 1px solid var(--border-subtle);
}

.add-role-prompt {
  padding: var(--space-4);
  text-align: center;
  color: var(--text-muted);
  font-size: 13px;
}

.btn-add-role {
  margin-top: var(--space-3);
  padding: var(--space-2) var(--space-4);
  border: 1px solid var(--primary);
  border-radius: var(--radius-md);
  background: transparent;
  color: var(--primary);
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: all var(--transition-fast);
}

.btn-add-role:hover {
  background: var(--primary-muted);
}

/* Fields */
.field-row {
  display: flex;
  gap: var(--space-3);
  margin-bottom: var(--space-3);
  padding-top: var(--space-3);
}

.field {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.field-label {
  font-size: 11px;
  font-weight: 500;
  letter-spacing: 0.03em;
  text-transform: uppercase;
  color: var(--text-muted);
}

.field-input {
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-sm);
  background: var(--bg-base);
  border: 1px solid var(--border-subtle);
  color: var(--text-primary);
  font-size: 13px;
  font-family: var(--font-mono);
  transition: border-color var(--transition-fast);
}

.field-input:focus {
  outline: none;
  border-color: var(--primary);
}

.field-input-narrow {
  max-width: 100px;
}

.field-textarea {
  width: 100%;
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-sm);
  background: var(--bg-base);
  border: 1px solid var(--border-subtle);
  color: var(--text-primary);
  font-size: 13px;
  font-family: var(--font-mono);
  resize: vertical;
  min-height: 80px;
  transition: border-color var(--transition-fast);
}

.field-textarea:focus {
  outline: none;
  border-color: var(--primary);
}

/* Save bar */
.save-bar {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding-top: var(--space-4);
  margin-top: var(--space-2);
  border-top: 1px solid var(--border-subtle);
}

.btn-save {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-4);
  border: none;
  border-radius: var(--radius-md);
  background: var(--primary);
  color: var(--bg-base);
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: background var(--transition-fast);
}

.btn-save:hover:not(:disabled) {
  background: var(--primary-hover);
}

.btn-save:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.save-message {
  font-size: 12px;
  color: var(--primary);
}

.save-message.error {
  color: var(--error, #ef4444);
}

/* Empty state */
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-8);
  color: var(--text-muted);
  font-size: 13px;
  text-align: center;
}

.empty-icon {
  opacity: 0.4;
  margin-bottom: var(--space-2);
}
</style>
