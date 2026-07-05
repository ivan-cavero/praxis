<script setup lang="ts">
/**
 * SettingsDialog — Full settings panel as a modal overlay.
 *
 * Opens on top of the main content area with a backdrop.
 * Close via Esc, backdrop click, or the close button.
 */

import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useApi, type LimitsDetail } from '../composables/useApi'
import { useAppStore } from '../stores/app'
import { useUpdater } from '../composables/useUpdater'
import { storeToRefs } from 'pinia'
import Icon from '../components/ui/Icon.vue'
import AgentsConfig from '../components/dashboard/AgentsConfig.vue'
import RemoteConnections from '../components/dashboard/RemoteConnections.vue'

const emit = defineEmits<{
  close: []
}>()

const api = useApi()
const store = useAppStore()
const updater = useUpdater()
const { version, uptime } = storeToRefs(store)

// ─── Tabs (computed — 'limits' only shows when a project is active) ──

const activeTab = ref('general')

const tabs = computed(() => {
  const base = [
    { id: 'general', label: 'General', icon: 'settings' },
    { id: 'model-settings', label: 'Model Settings', icon: 'server' },
    { id: 'skills', label: 'Skills', icon: 'terminal' },
    { id: 'subagents', label: 'Subagents', icon: 'robot' },
    ...(store.activeProject?.id
      ? [{ id: 'limits', label: 'Limits', icon: 'database' }]
      : []),
    { id: 'remote', label: 'Remote', icon: 'globe' },
    { id: 'usage', label: 'Usage', icon: 'chart' },
  ]
  return base
})

// ─── Project Limits ──────────────────────────────────────────────

const limitsForm = ref<LimitsDetail>({
  max_iterations_per_goal: 25,
  max_iterations_per_phase: 10,
  session_ttl_seconds: 3600,
  phase_timeout_seconds: 300,
})
const limitsSaving = ref(false)
const limitsSaved = ref(false)
const limitsError = ref<string | null>(null)

/** Load limits from the active project's config. */
function loadLimits() {
  if (store.activeConfig?.limits) {
    limitsForm.value = { ...store.activeConfig.limits }
  }
}

/** Update the `[limits]` section in the forge TOML and save. */
async function saveLimits() {
  if (!store.activeProject?.id) return
  limitsSaving.value = true
  limitsSaved.value = false
  limitsError.value = null

  const newLimits = limitsForm.value
  const toml = store.activeConfig?.raw || store.activeProject.forge_toml || ''
  const limitsSection =
    `[limits]\n` +
    `max_iterations_per_goal = ${newLimits.max_iterations_per_goal}\n` +
    `max_iterations_per_phase = ${newLimits.max_iterations_per_phase}\n` +
    `session_ttl_seconds = ${newLimits.session_ttl_seconds}\n` +
    `phase_timeout_seconds = ${newLimits.phase_timeout_seconds}\n`

  let updatedToml: string
  if (/\[limits\]/.test(toml)) {
    updatedToml = toml.replace(/\[limits\][\s\S]*?(?=\n\[|$)/, limitsSection)
  } else {
    updatedToml = toml + '\n' + limitsSection + '\n'
  }

  try {
    await store.saveProjectConfig(store.activeProject.id, updatedToml)
    // Refresh in-memory limits from the saved config
    if (store.activeConfig?.limits) {
      limitsForm.value = { ...store.activeConfig.limits }
    }
    limitsSaved.value = true
    setTimeout(() => { limitsSaved.value = false }, 2000)
  } catch (caughtError: unknown) {
    limitsError.value = caughtError instanceof Error ? caughtError.message : 'Failed to save limits'
  } finally {
    limitsSaving.value = false
  }
}

// ─── Providers ────────────────────────────────────────────────────

interface ProviderKey {
  provider: string
  key_masked: string
  has_key: boolean
}

const providers = ref<ProviderKey[]>([])
const loading = ref(false)
const saving = ref<string | null>(null)
const saveError = ref<string | null>(null)
const selectedProvider = ref<string | null>(null)
const newProvider = ref('')
const newApiKey = ref('')
const showInput = ref(false)

const knownProviders = [
  { name: 'nan', label: 'Nan Builders', placeholder: 'sk-nan-...', desc: 'qwen3.6 model' },
  { name: 'openai', label: 'OpenAI', placeholder: 'sk-proj-...', desc: 'GPT models' },
  { name: 'anthropic', label: 'Anthropic', placeholder: 'sk-ant-...', desc: 'Claude models' },
  { name: 'gemini', label: 'Google Gemini', placeholder: 'AIza... or API key', desc: 'Gemini models' },
  { name: 'ollama', label: 'Ollama', placeholder: 'http://localhost:11434', desc: 'Local models' },
]

async function loadProviders() {
  loading.value = true
  try {
    const data = await api.get<{ providers: ProviderKey[] }>('/vault/keys')
    providers.value = data.providers || []
    if (providers.value.length > 0 && !selectedProvider.value) {
      selectedProvider.value = providers.value[0].provider
    }
  } catch (e) {
    saveError.value = 'Failed to load providers'
  } finally {
    loading.value = false
  }
}

async function saveKey() {
  if (!newProvider.value || !newApiKey.value) return
  saving.value = newProvider.value
  saveError.value = null

  try {
    await api.post('/vault/keys', {
      provider: newProvider.value,
      api_key: newApiKey.value,
    })
    newProvider.value = ''
    newApiKey.value = ''
    showInput.value = false
    await loadProviders()
  } catch (caughtError: unknown) {
    saveError.value = caughtError instanceof Error ? caughtError.message : 'Failed to save key'
  } finally {
    saving.value = null
  }
}

async function deleteKey(provider: string) {
  saving.value = provider
  saveError.value = null
  try {
    await api.del(`/vault/keys/${provider}`)
    if (selectedProvider.value === provider) selectedProvider.value = null
    await loadProviders()
  } catch (caughtError: unknown) {
    saveError.value = caughtError instanceof Error ? caughtError.message : 'Failed to delete key'
  } finally {
    saving.value = null
  }
}

function selectKnownProvider(name: string) {
  newProvider.value = name
  if (!showInput.value) showInput.value = true
}

// ─── Keyboard ─────────────────────────────────────────────────────

function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    emit('close')
  }
}

onMounted(() => {
  loadProviders()
  if (store.activeProject?.id) loadLimits()
  document.addEventListener('keydown', handleKeydown)
})

onUnmounted(() => {
  document.removeEventListener('keydown', handleKeydown)
})

// ─── Manual Update ───────────────────────────────────────────────

const isCheckingUpdate = ref(false)
const updateMessage = ref<string | null>(null)

async function handleCheckUpdate() {
  isCheckingUpdate.value = true
  updateMessage.value = null
  try {
    await updater.checkForUpdates()
    if (updater.updateAvailable.value) {
      updateMessage.value = `Update ${updater.updateVersion.value} available`
    } else {
      updateMessage.value = 'You are on the latest version'
    }
  } catch {
    updateMessage.value = 'Could not check for updates'
  } finally {
    isCheckingUpdate.value = false
  }
}
</script>

<template>
  <div class="settings-overlay" @click.self="emit('close')">
    <div class="settings-dialog">
      <!-- Dialog Header -->
      <div class="dialog-header">
        <h2 class="dialog-title">Settings</h2>
        <button class="dialog-close" @click="emit('close')" title="Close">
          <Icon name="x" :size="20" />
        </button>
      </div>

      <div class="dialog-body">
        <!-- Sidebar Nav -->
        <nav class="dialog-nav">
          <button
            v-for="tab in tabs"
            :key="tab.id"
            class="dialog-nav-item"
            :class="{ active: activeTab === tab.id }"
            @click="activeTab = tab.id"
          >
            <Icon :name="tab.icon" :size="16" class="nav-icon" />
            <span>{{ tab.label }}</span>
          </button>
        </nav>

        <!-- Content -->
        <div class="dialog-content">
          <!-- ═══ General ═══ -->
          <template v-if="activeTab === 'general'">
            <div class="content-header">
              <h1 class="content-title">General</h1>
              <p class="content-subtitle">App information and preferences</p>
            </div>

            <div class="section-card">
              <div class="section-card-header">
                <Icon name="info" :size="20" class="section-icon" />
                <div>
                  <h3 class="section-title">About praxis</h3>
                  <p class="section-desc">Version and system information</p>
                </div>
              </div>
              <div class="info-grid">
                <div class="info-row">
                  <span class="info-label">Version</span>
                  <span class="info-value mono">{{ version || '--' }}</span>
                </div>
                <div class="info-row">
                  <span class="info-label">Uptime</span>
                  <span class="info-value">{{ uptime }}</span>
                </div>
                <div class="info-row">
                  <span class="info-label">Backend</span>
                  <span class="info-value">Connected</span>
                </div>
              </div>
            </div>

            <div class="section-card">
              <div class="section-card-header">
                <Icon name="download" :size="20" class="section-icon" />
                <div>
                  <h3 class="section-title">Updates</h3>
                  <p class="section-desc">Check for new versions manually</p>
                </div>
              </div>
              <div class="update-row">
                <button
                  class="btn btn-secondary"
                  :disabled="isCheckingUpdate"
                  @click="handleCheckUpdate()"
                >
                  <Icon v-if="isCheckingUpdate" name="refresh" :size="14" class="animate-spin" />
                  <Icon v-else name="refresh" :size="14" />
                  {{ isCheckingUpdate ? 'Checking...' : 'Check for Updates' }}
                </button>
                <span v-if="updateMessage" class="update-message">{{ updateMessage }}</span>
              </div>
              <div v-if="updater.updateAvailable.value" class="update-actions">
                <button
                  v-if="!updater.downloading.value && !updater.installDone.value"
                  class="btn btn-primary"
                  @click="updater.installUpdate()"
                >
                  Install {{ updater.updateVersion.value }}
                </button>
              </div>
            </div>
          </template>

          <!-- ═══ Model Settings (Providers) ═══ -->
          <template v-else-if="activeTab === 'model-settings'">
            <div class="content-header">
              <h1 class="content-title">Model Settings</h1>
              <p class="content-subtitle">Manage API providers and their models</p>
            </div>

            <!-- Error Toast -->
            <div v-if="saveError" class="toast toast-error">
              <Icon name="alert-circle" :size="14" />
              <span>{{ saveError }}</span>
              <button class="toast-dismiss" @click="saveError = null">
                <Icon name="x" :size="12" />
              </button>
            </div>

            <div class="model-settings-grid">
              <!-- Providers List -->
              <div class="providers-panel">
                <div class="panel-header">Providers</div>

                <div class="providers-list">
                  <!-- Configured providers -->
                  <div
                    v-for="provider in providers"
                    :key="provider.provider"
                    class="provider-card"
                    :class="{ active: selectedProvider === provider.provider }"
                    @click="selectedProvider = provider.provider"
                  >
                    <div class="provider-card-info">
                      <div class="provider-status-dot" :class="provider.has_key ? 'enabled' : 'disabled'" />
                      <span class="provider-card-name">{{ provider.provider }}</span>
                    </div>
                    <button
                      class="provider-card-delete"
                      :disabled="saving === provider.provider"
                      @click.stop="deleteKey(provider.provider)"
                      title="Delete key"
                    >
                      <Icon name="trash" :size="14" />
                    </button>
                  </div>

                  <!-- Available (unconfigured) providers -->
                  <div
                    v-for="kp in knownProviders.filter(k => !providers.find(p => p.provider === k.name))"
                    :key="kp.name"
                    class="provider-card"
                    :class="{ active: selectedProvider === kp.name }"
                    @click="selectKnownProvider(kp.name)"
                  >
                    <div class="provider-card-info">
                      <div class="provider-status-dot disabled" />
                      <span class="provider-card-name">{{ kp.label }}</span>
                    </div>
                    <span class="provider-badge">Add</span>
                  </div>
                </div>

                <button class="add-provider-btn" @click="showInput = true">
                  <Icon name="plus" :size="16" />
                  <span>Custom Provider</span>
                </button>
              </div>

              <!-- Provider Details / Add Form -->
              <div class="provider-details">
                <!-- Add Provider Form -->
                <div v-if="showInput" class="add-key-card">
                  <h3 class="add-key-title">Add API Key</h3>
                  <div class="input-group">
                    <label class="input-label">Provider</label>
                    <input
                      v-model="newProvider"
                      class="input"
                      placeholder="e.g. nan, openai, anthropic"
                      autofocus
                    />
                  </div>
                  <div class="input-group">
                    <label class="input-label">API Key</label>
                    <input
                      v-model="newApiKey"
                      type="password"
                      class="input"
                      placeholder="sk-xxx..."
                      @keydown.enter="saveKey"
                    />
                  </div>
                  <div class="add-key-actions">
                    <button class="btn btn-ghost" @click="showInput = false">Cancel</button>
                    <button
                      class="btn btn-primary"
                      :disabled="!newProvider || !newApiKey || saving !== null"
                      @click="saveKey"
                    >
                      <Icon v-if="saving" name="refresh" :size="14" class="animate-spin" />
                      <Icon v-else name="check" :size="14" />
                      {{ saving ? 'Saving...' : 'Save Key' }}
                    </button>
                  </div>
                </div>

                <!-- Provider Info (when selected) -->
                <div v-else-if="selectedProvider" class="provider-info-card">
                  <div class="provider-info-header">
                    <div class="provider-info-name">{{ selectedProvider }}</div>
                    <span class="badge badge-green">Configured</span>
                  </div>
                  <div class="provider-info-body">
                    <p class="provider-info-desc">
                      {{ knownProviders.find(k => k.name === selectedProvider)?.desc || 'Custom provider' }}
                    </p>
                    <div class="provider-info-actions">
                      <button class="btn btn-secondary btn-sm" @click="showInput = true">
                        <Icon name="edit" :size="12" />
                        Change Key
                      </button>
                      <button
                        class="btn btn-ghost btn-sm btn-danger-text"
                        @click="deleteKey(selectedProvider)"
                        :disabled="saving === selectedProvider"
                      >
                        <Icon name="trash" :size="12" />
                        Remove
                      </button>
                    </div>
                  </div>
                </div>

                <!-- Empty state -->
                <div v-else class="provider-empty">
                  <Icon name="server" :size="40" class="placeholder-icon" />
                  <p>Select a provider or add a new one</p>
                </div>
              </div>
            </div>
          </template>

          <!-- ═══ Skills ═══ -->
          <template v-else-if="activeTab === 'skills'">
            <div class="content-header">
              <h1 class="content-title">Skills</h1>
              <p class="content-subtitle">Configure agent roles, models, and tools</p>
            </div>
            <AgentsConfig />
          </template>

          <!-- ═══ Subagents ═══ -->
          <template v-else-if="activeTab === 'subagents'">
            <div class="content-header">
              <h1 class="content-title">Subagents</h1>
              <p class="content-subtitle">Configure specialized sub-agent roles</p>
            </div>
            <div class="section-card">
              <div class="section-card-header">
                <Icon name="robot" :size="20" class="section-icon" />
                <div>
                  <h3 class="section-title">Subagent System</h3>
                  <p class="section-desc">Delegated agents with specialized capabilities</p>
                </div>
              </div>
              <div class="info-grid">
                <div class="info-row">
                  <span class="info-label">Status</span>
                  <span class="info-value">
                    <span class="badge badge-green">Active</span>
                  </span>
                </div>
                <div class="info-row">
                  <span class="info-label">Backend</span>
                  <span class="info-value">ractor actors with mailboxes</span>
                </div>
                <div class="info-row">
                  <span class="info-label">Delegation</span>
                  <span class="info-value">DELEGATE:agent_type:task protocol</span>
                </div>
                <div class="info-row">
                  <span class="info-label">Budget propagation</span>
                  <span class="info-value">Parent → child narrowing</span>
                </div>
              </div>
              <p class="section-note">
                Subagents are implemented in praxis-core as ractor actors.
                Each agent runs in its own mailbox and can delegate to sub-agents
                using the <code>DELEGATE:agent_type:task</code> protocol.
                Budgets are propagated from parent to child with authority narrowing.
                <br /><br />
                To configure custom agents, use the <strong>Agents</strong> tab
                in the sidebar or run <code>praxis agent add</code> from the CLI.
              </p>
            </div>
          </template>

          <!-- ═══ Remote ═══ -->
          <template v-else-if="activeTab === 'remote'">
            <div class="content-header">
              <h1 class="content-title">Remote Connections</h1>
              <p class="content-subtitle">Connect to remote praxis servers via QR pairing</p>
            </div>
            <RemoteConnections />
          </template>

          <!-- ═══ Limits (per-project) ═══ -->
          <template v-else-if="activeTab === 'limits'">
            <div class="content-header">
              <h1 class="content-title">Project Limits</h1>
              <p class="content-subtitle">
                Execution boundaries for
                <strong>{{ store.activeProject?.name || 'current project' }}</strong>
              </p>
            </div>

            <!-- Success Toast -->
            <div v-if="limitsSaved" class="toast toast-success">
              <Icon name="check" :size="14" />
              <span>Limits saved!</span>
            </div>

            <!-- Error Toast -->
            <div v-if="limitsError" class="toast toast-error">
              <Icon name="alert-circle" :size="14" />
              <span>{{ limitsError }}</span>
              <button class="toast-dismiss" @click="limitsError = null">
                <Icon name="x" :size="12" />
              </button>
            </div>

            <div class="section-card">
              <div class="section-card-header">
                <Icon name="database" :size="20" class="section-icon" />
                <div>
                  <h3 class="section-title">Iteration Limits</h3>
                  <p class="section-desc">Control how many cycles each goal and phase can run</p>
                </div>
              </div>
              <div class="limits-form">
                <div class="input-group">
                  <label class="input-label">Max iterations per goal</label>
                  <input
                    v-model.number="limitsForm.max_iterations_per_goal"
                    type="number"
                    class="input"
                    min="1"
                    max="999"
                  />
                  <p class="input-hint">Total iterations before a goal is considered complete</p>
                </div>
                <div class="input-group">
                  <label class="input-label">Max iterations per phase</label>
                  <input
                    v-model.number="limitsForm.max_iterations_per_phase"
                    type="number"
                    class="input"
                    min="1"
                    max="100"
                  />
                  <p class="input-hint">Iterations per phase (plan → implement → review → etc.)</p>
                </div>
              </div>
            </div>

            <div class="section-card">
              <div class="section-card-header">
                <Icon name="clock" :size="20" class="section-icon" />
                <div>
                  <h3 class="section-title">Timeouts</h3>
                  <p class="section-desc">Session and phase time limits</p>
                </div>
              </div>
              <div class="limits-form">
                <div class="input-group">
                  <label class="input-label">Session TTL (seconds)</label>
                  <input
                    v-model.number="limitsForm.session_ttl_seconds"
                    type="number"
                    class="input"
                    min="60"
                    max="86400"
                    step="60"
                  />
                  <p class="input-hint">How long a session lives before being recycled (1h – 24h)</p>
                </div>
                <div class="input-group">
                  <label class="input-label">Phase timeout (seconds)</label>
                  <input
                    v-model.number="limitsForm.phase_timeout_seconds"
                    type="number"
                    class="input"
                    min="30"
                    max="3600"
                    step="10"
                  />
                  <p class="input-hint">Max wall-clock time for a single phase before timeout</p>
                </div>
              </div>

              <div class="limits-actions">
                <button
                  class="btn btn-primary"
                  :disabled="limitsSaving"
                  @click="saveLimits"
                >
                  <Icon v-if="limitsSaving" name="refresh" :size="14" class="animate-spin" />
                  <Icon v-else name="check" :size="14" />
                  {{ limitsSaving ? 'Saving...' : 'Save Limits' }}
                </button>
                <button
                  class="btn btn-ghost"
                  @click="loadLimits()"
                >
                  <Icon name="refresh" :size="14" />
                  Reset
                </button>
              </div>
            </div>
          </template>

          <!-- ═══ Usage ═══ -->
          <template v-else-if="activeTab === 'usage'">
            <div class="content-header">
              <h1 class="content-title">Usage</h1>
              <p class="content-subtitle">Token usage and cost statistics</p>
            </div>
            <div class="section-card">
              <div class="section-card-header">
                <Icon name="info" :size="20" class="section-icon" />
                <div>
                  <h3 class="section-title">System Status</h3>
                  <p class="section-desc">Current backend and session information</p>
                </div>
              </div>
              <div class="info-grid">
                <div class="info-row">
                  <span class="info-label">Version</span>
                  <span class="info-value mono">{{ version || '--' }}</span>
                </div>
                <div class="info-row">
                  <span class="info-label">Uptime</span>
                  <span class="info-value">{{ uptime }}</span>
                </div>
                <div class="info-row">
                  <span class="info-label">Projects</span>
                  <span class="info-value">{{ store.projects?.length ?? 0 }}</span>
                </div>
                <div class="info-row">
                  <span class="info-label">Providers</span>
                  <span class="info-value">{{ providers.length }}</span>
                </div>
              </div>
            </div>
            <div class="section-card">
              <div class="section-card-header">
                <Icon name="chart" :size="20" class="section-icon" />
                <div>
                  <h3 class="section-title">Cost Analysis</h3>
                  <p class="section-desc">View detailed cost breakdown in the Cost Analysis page</p>
                </div>
              </div>
              <p class="section-note">
                Token usage and cost data are tracked per session.
                Visit the <strong>Cost</strong> tab in the sidebar for detailed charts
                and per-session cost breakdowns.
              </p>
            </div>
          </template>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
/* ═══ Overlay ═══ */

.settings-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.6);
  backdrop-filter: blur(4px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 150;
  animation: overlayFadeIn 0.15s ease-out;
}

@keyframes overlayFadeIn {
  from { opacity: 0; }
  to { opacity: 1; }
}

/* ═══ Dialog ═══ */

.settings-dialog {
  width: 900px;
  max-width: 95vw;
  height: 700px;
  max-height: 85vh;
  background: var(--bg-surface);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-xl);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  box-shadow: var(--shadow-lg), 0 0 60px rgba(0, 0, 0, 0.3);
  animation: dialogSlideIn 0.2s ease-out;
}

@keyframes dialogSlideIn {
  from { opacity: 0; transform: translateY(12px) scale(0.98); }
  to { opacity: 1; transform: translateY(0) scale(1); }
}

.dialog-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-4) var(--space-5);
  border-bottom: 1px solid var(--border-subtle);
  flex-shrink: 0;
}

.dialog-title {
  font-size: 16px;
  font-weight: 600;
  color: var(--text-primary);
}

.dialog-close {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border: none;
  border-radius: var(--radius-md);
  background: transparent;
  color: var(--text-muted);
  cursor: pointer;
  transition: all 0.15s;
}

.dialog-close:hover {
  color: var(--text-primary);
  background: var(--bg-hover);
}

/* ═══ Body ═══ */

.dialog-body {
  display: flex;
  flex: 1;
  min-height: 0;
}

.dialog-nav {
  width: 180px;
  flex-shrink: 0;
  padding: var(--space-3);
  display: flex;
  flex-direction: column;
  gap: 1px;
  overflow-y: auto;
  border-right: 1px solid var(--border-subtle);
}

.dialog-nav-item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-md);
  border: none;
  background: transparent;
  color: var(--text-secondary);
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.15s;
  text-align: left;
  width: 100%;
  font-family: inherit;
  white-space: nowrap;
}

.dialog-nav-item:hover {
  color: var(--text-primary);
  background: var(--bg-hover);
}

.dialog-nav-item.active {
  color: var(--text-primary);
  background: var(--bg-elevated);
}

.dialog-nav-item .nav-icon {
  opacity: 0.6;
  flex-shrink: 0;
}

.dialog-nav-item:hover .nav-icon,
.dialog-nav-item.active .nav-icon {
  opacity: 1;
}

.dialog-content {
  flex: 1;
  padding: var(--space-5);
  overflow-y: auto;
  min-width: 0;
}

/* ═══ Content Header ═══ */

.content-header {
  margin-bottom: var(--space-5);
}

.content-title {
  font-size: 22px;
  font-weight: 600;
  color: var(--text-primary);
  letter-spacing: -0.02em;
  margin-bottom: var(--space-1);
}

.content-subtitle {
  font-size: 14px;
  color: var(--text-muted);
}

/* ═══ Section Card (General) ═══ */

.section-card {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  background: var(--bg-base);
  padding: var(--space-5);
  margin-bottom: var(--space-4);
}

.section-card-header {
  display: flex;
  align-items: flex-start;
  gap: var(--space-3);
  margin-bottom: var(--space-4);
}

.section-icon {
  color: var(--primary);
  flex-shrink: 0;
  margin-top: 2px;
}

.section-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--text-primary);
  margin-bottom: var(--space-1);
}

.section-desc {
  font-size: 13px;
  color: var(--text-muted);
}

.section-note {
  font-size: 13px;
  line-height: 1.6;
  color: var(--text-muted);
  padding: var(--space-3);
  margin-top: var(--space-2);
  background: var(--bg-surface);
  border-radius: var(--radius-md);
  border: 1px solid var(--border-subtle);
}

.info-grid {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.info-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-2) 0;
  border-bottom: 1px solid var(--border-subtle);
}

.info-row:last-child {
  border-bottom: none;
}

.info-label {
  font-size: 13px;
  color: var(--text-muted);
}

.info-value {
  font-size: 13px;
  font-weight: 500;
  color: var(--text-primary);
}

.mono {
  font-family: var(--font-mono);
}

.update-row {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  margin-bottom: var(--space-3);
}

.update-message {
  font-size: 13px;
  color: var(--primary);
}

.update-actions {
  margin-top: var(--space-2);
}

/* ═══ Toast ═══ */

.toast {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-3) var(--space-4);
  border-radius: var(--radius-md);
  font-size: 13px;
  margin-bottom: var(--space-4);
  animation: toastSlideIn 0.2s ease-out;
}

@keyframes toastSlideIn {
  from { opacity: 0; transform: translateY(-8px); }
  to { opacity: 1; transform: translateY(0); }
}

.toast-error {
  background: rgba(239, 68, 68, 0.1);
  color: var(--error);
  border: 1px solid rgba(239, 68, 68, 0.2);
}

.toast-success {
  background: rgba(34, 197, 94, 0.1);
  color: var(--success, #22c55e);
  border: 1px solid rgba(34, 197, 94, 0.2);
}

.toast-dismiss {
  display: flex;
  margin-left: auto;
  background: none;
  border: none;
  color: inherit;
  cursor: pointer;
  opacity: 0.6;
  padding: 2px;
}

.toast-dismiss:hover {
  opacity: 1;
}

/* ═══ Model Settings Grid ═══ */

.model-settings-grid {
  display: grid;
  grid-template-columns: 200px 1fr;
  gap: var(--space-5);
  min-height: 300px;
}

/* Providers Panel */
.providers-panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.panel-header {
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: var(--text-muted);
  padding: var(--space-2) var(--space-3);
}

.providers-list {
  display: flex;
  flex-direction: column;
  gap: 1px;
}

.provider-card {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-md);
  cursor: pointer;
  transition: all 0.15s;
}

.provider-card:hover {
  background: var(--bg-hover);
}

.provider-card.active {
  background: var(--bg-elevated);
}

.provider-card-info {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  min-width: 0;
}

.provider-status-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  flex-shrink: 0;
}

.provider-status-dot.enabled {
  background: var(--primary);
  box-shadow: 0 0 6px var(--primary-glow);
}

.provider-status-dot.disabled {
  background: var(--text-disabled);
}

.provider-card-name {
  font-size: 13px;
  font-weight: 500;
  color: var(--text-primary);
}

.provider-card-delete {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border-radius: var(--radius-sm);
  background: none;
  border: none;
  color: var(--text-muted);
  cursor: pointer;
  opacity: 0;
  transition: all 0.15s;
}

.provider-card:hover .provider-card-delete {
  opacity: 0.6;
}

.provider-card-delete:hover {
  opacity: 1 !important;
  color: var(--error);
  background: rgba(239, 68, 68, 0.1);
}

.provider-badge {
  font-size: 11px;
  font-weight: 600;
  color: var(--primary);
  opacity: 0;
  transition: opacity 0.15s;
}

.provider-card:hover .provider-badge {
  opacity: 1;
}

.add-provider-btn {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-md);
  background: transparent;
  border: 1px dashed var(--border-default);
  color: var(--text-secondary);
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.15s;
  margin-top: var(--space-2);
  font-family: inherit;
}

.add-provider-btn:hover {
  border-color: var(--primary);
  color: var(--primary);
  background: var(--primary-muted);
}

/* Provider Details */
.provider-details {
  display: flex;
  flex-direction: column;
}

.add-key-card {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  padding: var(--space-5);
  background: var(--bg-base);
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.add-key-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--text-primary);
}

.add-key-actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-2);
  margin-top: var(--space-2);
}

.provider-info-card {
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  overflow: hidden;
}

.provider-info-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-4) var(--space-5);
  border-bottom: 1px solid var(--border-subtle);
}

.provider-info-name {
  font-size: 16px;
  font-weight: 600;
  color: var(--text-primary);
}

.provider-info-body {
  padding: var(--space-4) var(--space-5);
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.provider-info-desc {
  font-size: 13px;
  color: var(--text-muted);
}

.provider-info-actions {
  display: flex;
  gap: var(--space-2);
}

.provider-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: var(--space-8);
  color: var(--text-muted);
  gap: var(--space-3);
}

.placeholder-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: var(--space-12) var(--space-8);
  color: var(--text-muted);
  gap: var(--space-4);
  border: 1px dashed var(--border-subtle);
  border-radius: var(--radius-lg);
}

.placeholder-icon {
  opacity: 0.3;
}

/* ═══ Shared ═══ */

.btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-4);
  border-radius: var(--radius-md);
  font-size: 13px;
  font-weight: 500;
  font-family: inherit;
  cursor: pointer;
  border: 1px solid transparent;
  transition: all 0.15s;
  white-space: nowrap;
  line-height: 1.4;
}

.btn:active {
  transform: scale(0.98);
}

.btn-primary {
  background: var(--primary);
  color: var(--bg-base);
  border: none;
}

.btn-primary:hover {
  background: var(--primary-hover);
}

.btn-primary:disabled {
  opacity: 0.5;
  cursor: not-allowed;
  transform: none;
}

.btn-secondary {
  background: var(--bg-elevated);
  color: var(--text-secondary);
  border-color: var(--border-subtle);
}

.btn-secondary:hover {
  color: var(--text-primary);
  background: var(--bg-hover);
  border-color: var(--border-default);
}

.btn-ghost {
  background: transparent;
  color: var(--text-secondary);
  border-color: transparent;
}

.btn-ghost:hover {
  color: var(--text-primary);
  background: var(--bg-hover);
}

.btn-sm {
  padding: var(--space-1) var(--space-3);
  font-size: 12px;
}

.btn-danger-text {
  color: var(--error);
}

.btn-danger-text:hover {
  background: rgba(239, 68, 68, 0.1) !important;
  color: var(--error) !important;
}

.badge {
  display: inline-flex;
  align-items: center;
  padding: var(--space-1) var(--space-3);
  border-radius: var(--radius-full);
  font-size: 11px;
  font-weight: 600;
}

.badge-green {
  background: var(--primary-muted);
  color: var(--primary);
}

.input-group {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.input-hint {
  font-size: 12px;
  color: var(--text-muted);
  margin: 0;
  line-height: 1.4;
}

/* ═══ Limits Form ═══ */

.limits-form {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
  padding: var(--space-3) 0;
}

.limits-actions {
  display: flex;
  gap: var(--space-2);
  margin-top: var(--space-3);
  padding-top: var(--space-4);
  border-top: 1px solid var(--border-subtle);
}

.input-label {
  font-size: 11px;
  font-weight: 500;
  letter-spacing: 0.03em;
  text-transform: uppercase;
  color: var(--text-muted);
}

.input {
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  padding: var(--space-2) var(--space-3);
  font-size: 13px;
  font-family: var(--font-sans);
  color: var(--text-primary);
  transition: all 0.15s;
}

.input:hover {
  border-color: var(--border-default);
}

.input:focus {
  outline: none;
  border-color: var(--primary);
  box-shadow: 0 0 0 3px var(--primary-muted);
}

.input::placeholder {
  color: var(--text-muted);
}

.animate-spin {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}
</style>
