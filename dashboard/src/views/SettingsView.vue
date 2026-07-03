<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useApi } from '../composables/useApi'
import Icon from '../components/ui/Icon.vue'

const emit = defineEmits<{
  back: []
}>()

const api = useApi()

interface ProviderKey {
  provider: string
  key_masked: string
  has_key: boolean
}

interface ModelInfo {
  name: string
  tokens: string
  usage?: number
  limit?: number
}

const providers = ref<ProviderKey[]>([])
const loading = ref(false)
const saving = ref<string | null>(null)
const selectedProvider = ref<string | null>(null)
const activeSettingsTab = ref('model-settings')

// Form state
const newProvider = ref('')
const newApiKey = ref('')
const showInput = ref(false)

// Known provider templates
const knownProviders = [
  { name: 'nan', label: 'Nan Builders', placeholder: 'sk-nan-...', desc: 'qwen3.6 model' },
  { name: 'openai', label: 'OpenAI', placeholder: 'sk-proj-...', desc: 'GPT models' },
  { name: 'anthropic', label: 'Anthropic', placeholder: 'sk-ant-...', desc: 'Claude models' },
  { name: 'gemini', label: 'Google Gemini', placeholder: 'AIza... or API key', desc: 'Gemini models' },
]

// Models data
const models = ref<ModelInfo[]>([
  { name: 'GLM-5.2', tokens: '1M', usage: 3000000, limit: 3000000 },
  { name: 'GLM-5-Turbo', tokens: '200K', usage: 2000000, limit: 2000000 },
])

const settingsNavItems = [
  { id: 'general', label: 'General', icon: 'settings' },
  { id: 'code-preview', label: 'Code preview', icon: 'code' },
  { id: 'model-settings', label: 'Model settings', icon: 'server' },
  { id: 'skills', label: 'Skills', icon: 'code' },
  { id: 'subagents', label: 'Subagents', icon: 'robot' },
  { id: 'mcp-servers', label: 'MCP Servers', icon: 'terminal' },
  { id: 'plugins', label: 'Plugins', icon: 'plug' },
  { id: 'commands', label: 'Commands', icon: 'command' },
  { id: 'indexing', label: 'Indexing', icon: 'search' },
  { id: 'usage', label: 'Usage', icon: 'chart' },
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
    console.error('Failed to load providers:', e)
  } finally {
    loading.value = false
  }
}

async function saveKey() {
  if (!newProvider.value || !newApiKey.value) return
  saving.value = newProvider.value

  try {
    await api.post('/vault/keys', {
      provider: newProvider.value,
      api_key: newApiKey.value,
    })
    newProvider.value = ''
    newApiKey.value = ''
    showInput.value = false
    await loadProviders()
  } catch (e) {
    console.error('Failed to save key:', e)
  } finally {
    saving.value = null
  }
}

function selectKnownProvider(name: string) {
  newProvider.value = name
  if (!showInput.value) showInput.value = true
}

onMounted(() => {
  loadProviders()
})
</script>

<template>
  <div class="settings-layout">
    <!-- Settings Navigation -->
    <nav class="settings-nav">
      <button
        class="settings-nav-item"
        @click="emit('back')"
      >
        <Icon name="chevron-left" :size="16" class="nav-icon" />
        <span>Back to workspace</span>
      </button>

      <div style="height: var(--space-md);" />

      <button
        v-for="item in settingsNavItems"
        :key="item.id"
        class="settings-nav-item"
        :class="{ active: activeSettingsTab === item.id }"
        @click="activeSettingsTab = item.id"
      >
        <Icon :name="item.icon" :size="16" class="nav-icon" />
        <span>{{ item.label }}</span>
      </button>

      <div style="flex: 1;" />

      <!-- Onboard button -->
      <button class="settings-nav-item onboard-btn">
        <Icon name="robot" :size="16" class="nav-icon" />
        <span>Onboard</span>
      </button>
    </nav>

    <!-- Settings Content -->
    <div class="settings-content">
      <!-- Model Settings View -->
      <template v-if="activeSettingsTab === 'model-settings'">
        <div class="settings-header">
          <h1 class="settings-title">Model settings</h1>
          <p class="settings-subtitle">Manage custom model providers. Once configured, they can be selected during chat.</p>
        </div>

        <div class="model-settings-grid">
          <!-- Providers List -->
          <div class="providers-panel">
            <div class="panel-header">Providers</div>
            
            <div class="providers-list">
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
              </div>

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
              </div>
            </div>

            <div class="panel-section-title">Custom providers</div>
            <button class="add-provider-btn" @click="showInput = true">
              <Icon name="plus" :size="16" />
              <span>Add provider</span>
            </button>
          </div>

          <!-- Provider Details -->
          <div class="provider-details" v-if="selectedProvider">
            <!-- Provider Header -->
            <div class="provider-detail-header">
              <div class="provider-detail-title">
                <span class="provider-detail-name">{{ selectedProvider }}</span>
                <span class="badge badge-green">Enabled</span>
              </div>
              <div class="provider-detail-actions">
                <span class="detail-label">Connection mode</span>
                <select class="detail-select">
                  <option>Coding Plan</option>
                  <option>API Plan</option>
                </select>
              </div>
            </div>

            <!-- Plan Info -->
            <div class="plan-card">
              <div class="plan-header">
                <div>
                  <div class="plan-name">Start plan</div>
                  <div class="plan-meta">Expires Jul 6</div>
                </div>
                <div class="plan-actions">
                  <button class="plan-action">Manage</button>
                  <button class="plan-action">Unlink</button>
                  <button class="btn btn-sm btn-secondary">
                    <Icon name="upload" :size="12" />
                    Upgrade
                  </button>
                  <span class="badge badge-amber">150% Quota</span>
                </div>
              </div>

              <!-- Today's Balance -->
              <div class="quota-section">
                <div class="quota-header">
                  <span class="quota-title">Today's balance</span>
                  <span class="quota-meta">Expires Jul 6</span>
                </div>

                <div class="quota-grid">
                  <div v-for="model in models" :key="model.name" class="quota-card">
                    <div class="quota-card-header">
                      <span class="model-name">{{ model.name }}</span>
                      <span class="quota-percentage">100%</span>
                    </div>
                    <div class="quota-time">17:59</div>
                    <div class="progress-bar">
                      <div class="progress-bar-fill" style="width: 100%;" />
                    </div>
                    <div class="quota-usage">
                      {{ model.usage?.toLocaleString() }} / {{ model.limit?.toLocaleString() }}
                    </div>
                  </div>
                </div>
              </div>

              <!-- Model List -->
              <div class="model-list-section">
                <div class="model-list-title">Model list</div>
                <div class="model-list">
                  <div v-for="model in models" :key="model.name" class="model-card">
                    <span class="model-name">{{ model.name }}</span>
                    <span class="model-tokens">{{ model.tokens }}</span>
                  </div>
                </div>
              </div>
            </div>
          </div>
        </div>

        <!-- Add Provider Form Modal -->
        <div v-if="showInput" class="modal-overlay" @click.self="showInput = false">
          <div class="modal-card">
            <div class="modal-header">
              <h3 class="modal-title">Add Provider Key</h3>
              <button class="modal-close" @click="showInput = false">
                <Icon name="x" :size="18" />
              </button>
            </div>
            
            <div class="modal-body">
              <div class="input-group">
                <label class="input-label">PROVIDER</label>
                <input
                  v-model="newProvider"
                  class="input"
                  placeholder="Provider name (e.g., nan, openai)"
                />
              </div>

              <div class="input-group">
                <label class="input-label">API KEY</label>
                <input
                  v-model="newApiKey"
                  type="password"
                  class="input"
                  placeholder="sk-xxx..."
                  @keydown.enter="saveKey"
                />
              </div>
            </div>

            <div class="modal-footer">
              <button class="btn btn-ghost" @click="showInput = false">Cancel</button>
              <button
                class="btn btn-primary"
                @click="saveKey"
                :disabled="!newProvider || !newApiKey || saving !== null"
              >
                <Icon v-if="!saving" name="check" :size="14" />
                <Icon v-else name="refresh" :size="14" class="animate-spin" />
                {{ saving ? 'Saving...' : 'Save Key' }}
              </button>
            </div>
          </div>
        </div>
      </template>

      <!-- General Settings -->
      <template v-else-if="activeSettingsTab === 'general'">
        <div class="settings-header">
          <h1 class="settings-title">General</h1>
          <p class="settings-subtitle">Configure general application settings.</p>
        </div>
        <div class="settings-placeholder">
          <Icon name="settings" :size="48" class="opacity-40" />
          <p>General settings coming soon</p>
        </div>
      </template>

      <!-- Code Preview Settings -->
      <template v-else-if="activeSettingsTab === 'code-preview'">
        <div class="settings-header">
          <h1 class="settings-title">Code preview</h1>
          <p class="settings-subtitle">Configure how code is displayed and highlighted.</p>
        </div>
        <div class="settings-placeholder">
          <Icon name="code" :size="48" class="opacity-40" />
          <p>Code preview settings coming soon</p>
        </div>
      </template>

      <!-- Skills Settings -->
      <template v-else-if="activeSettingsTab === 'skills'">
        <div class="settings-header">
          <h1 class="settings-title">Skills</h1>
          <p class="settings-subtitle">Manage agent skills and capabilities.</p>
        </div>
        <div class="settings-placeholder">
          <Icon name="code" :size="48" class="opacity-40" />
          <p>Skills management coming soon</p>
        </div>
      </template>

      <!-- Other settings views -->
      <template v-else>
        <div class="settings-header">
          <h1 class="settings-title">{{ settingsNavItems.find(i => i.id === activeSettingsTab)?.label }}</h1>
          <p class="settings-subtitle">This section is under development.</p>
        </div>
        <div class="settings-placeholder">
          <Icon :name="settingsNavItems.find(i => i.id === activeSettingsTab)?.icon || 'settings'" :size="48" class="opacity-40" />
          <p>Coming soon</p>
        </div>
      </template>
    </div>
  </div>
</template>

<style scoped>
.settings-layout {
  display: grid;
  grid-template-columns: 220px 1fr;
  gap: var(--space-xl);
  height: 100%;
  padding: var(--space-lg);
}

.settings-nav {
  display: flex;
  flex-direction: column;
  gap: 2px;
  padding-right: var(--space-md);
}

.settings-nav-item {
  display: flex;
  align-items: center;
  gap: var(--space-md);
  padding: 10px var(--space-md);
  border-radius: var(--radius);
  border: none;
  background: transparent;
  color: var(--clr-text-secondary);
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.15s var(--ease);
  text-align: left;
  width: 100%;
  font-family: inherit;
}

.settings-nav-item:hover {
  color: var(--clr-text);
  background: var(--clr-surface-hover);
}

.settings-nav-item.active {
  background: var(--clr-surface-raised);
  color: var(--clr-text);
}

.onboard-btn {
  border: 1px dashed var(--clr-border);
  margin-top: var(--space-md);
}

.onboard-btn:hover {
  border-color: var(--clr-primary);
  color: var(--clr-primary);
  background: var(--clr-primary-glow);
}

.nav-icon {
  opacity: 0.7;
}

.settings-nav-item:hover .nav-icon,
.settings-nav-item.active .nav-icon {
  opacity: 1;
}

.settings-content {
  overflow-y: auto;
  padding-right: var(--space-md);
}

.settings-header {
  margin-bottom: var(--space-xl);
}

.settings-title {
  font-size: 24px;
  font-weight: 600;
  color: var(--clr-text);
  margin-bottom: var(--space-sm);
}

.settings-subtitle {
  font-size: 14px;
  color: var(--clr-text-muted);
}

.settings-placeholder {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: var(--space-3xl);
  text-align: center;
  color: var(--clr-text-muted);
  gap: var(--space-md);
}

.model-settings-grid {
  display: grid;
  grid-template-columns: 200px 1fr;
  gap: var(--space-xl);
}

/* Providers Panel */
.providers-panel {
  display: flex;
  flex-direction: column;
  gap: var(--space-sm);
}

.panel-header {
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.1em;
  text-transform: uppercase;
  color: var(--clr-text-muted);
  padding: var(--space-sm);
  margin-bottom: var(--space-xs);
}

.panel-section-title {
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.1em;
  text-transform: uppercase;
  color: var(--clr-text-muted);
  padding: var(--space-sm);
  margin-top: var(--space-md);
  margin-bottom: var(--space-xs);
}

.providers-list {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.provider-card {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 10px var(--space-md);
  border-radius: var(--radius);
  background: transparent;
  cursor: pointer;
  transition: all 0.15s var(--ease);
}

.provider-card:hover {
  background: var(--clr-surface-hover);
}

.provider-card.active {
  background: var(--clr-surface-raised);
}

.provider-card-info {
  display: flex;
  align-items: center;
  gap: var(--space-md);
}

.provider-status-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  flex-shrink: 0;
}

.provider-status-dot.enabled {
  background: var(--clr-primary);
  box-shadow: 0 0 8px var(--clr-primary-glow);
}

.provider-status-dot.disabled {
  background: var(--clr-text-muted);
}

.provider-card-name {
  font-size: 13px;
  font-weight: 500;
  color: var(--clr-text);
}

.add-provider-btn {
  display: flex;
  align-items: center;
  gap: var(--space-sm);
  padding: 10px var(--space-md);
  border-radius: var(--radius);
  background: transparent;
  border: 1px dashed var(--clr-border);
  color: var(--clr-text-secondary);
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  transition: all 0.15s var(--ease);
  font-family: inherit;
}

.add-provider-btn:hover {
  border-color: var(--clr-primary);
  color: var(--clr-primary);
  background: var(--clr-primary-glow);
}

/* Provider Details */
.provider-details {
  display: flex;
  flex-direction: column;
  gap: var(--space-lg);
}

.provider-detail-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding-bottom: var(--space-lg);
  border-bottom: 1px solid var(--clr-border-subtle);
}

.provider-detail-title {
  display: flex;
  align-items: center;
  gap: var(--space-md);
}

.provider-detail-name {
  font-size: 18px;
  font-weight: 600;
  color: var(--clr-text);
}

.provider-detail-actions {
  display: flex;
  align-items: center;
  gap: var(--space-md);
}

.detail-label {
  font-size: 12px;
  color: var(--clr-text-muted);
}

.detail-select {
  padding: 6px 12px;
  border-radius: var(--radius);
  background: var(--clr-surface-raised);
  border: 1px solid var(--clr-border);
  color: var(--clr-text);
  font-size: 13px;
  font-family: inherit;
  cursor: pointer;
}

.detail-select:focus {
  outline: none;
  border-color: var(--clr-primary);
}

/* Plan Card */
.plan-card {
  border: 1px solid var(--clr-border-subtle);
  border-radius: var(--radius-lg);
  background: var(--clr-surface);
  overflow: hidden;
}

.plan-header {
  display: flex;
  align-items: flex-start;
  justify-content: space-between;
  padding: var(--space-lg);
  border-bottom: 1px solid var(--clr-border-subtle);
}

.plan-name {
  font-size: 16px;
  font-weight: 600;
  color: var(--clr-text);
  margin-bottom: var(--space-xs);
}

.plan-meta {
  font-size: 12px;
  color: var(--clr-text-muted);
}

.plan-actions {
  display: flex;
  align-items: center;
  gap: var(--space-md);
}

.plan-action {
  background: none;
  border: none;
  color: var(--clr-text-secondary);
  cursor: pointer;
  font-size: 13px;
  font-family: inherit;
  padding: 0;
}

.plan-action:hover {
  color: var(--clr-text);
}

/* Quota Section */
.quota-section {
  padding: var(--space-lg);
  border-bottom: 1px solid var(--clr-border-subtle);
}

.quota-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--space-lg);
}

.quota-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--clr-text);
}

.quota-meta {
  font-size: 12px;
  color: var(--clr-text-muted);
}

.quota-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(200px, 1fr));
  gap: var(--space-md);
}

.quota-card {
  padding: var(--space-md);
  border: 1px solid var(--clr-border-subtle);
  border-radius: var(--radius);
  background: var(--clr-surface-raised);
}

.quota-card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--space-xs);
}

.model-name {
  font-size: 13px;
  font-weight: 500;
  color: var(--clr-text);
}

.quota-percentage {
  font-size: 14px;
  font-weight: 600;
  color: var(--clr-primary);
}

.quota-time {
  font-size: 11px;
  color: var(--clr-text-muted);
  margin-bottom: var(--space-sm);
}

.progress-bar {
  height: 4px;
  background: var(--clr-border);
  border-radius: 2px;
  overflow: hidden;
  margin-bottom: var(--space-sm);
}

.progress-bar-fill {
  height: 100%;
  background: var(--clr-primary);
  border-radius: 2px;
  transition: width 0.3s var(--ease);
}

.quota-usage {
  font-size: 11px;
  font-family: var(--font-mono);
  color: var(--clr-text-muted);
}

/* Model List */
.model-list-section {
  padding: var(--space-lg);
}

.model-list-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--clr-text);
  margin-bottom: var(--space-md);
}

.model-list {
  display: flex;
  flex-direction: column;
  gap: var(--space-sm);
}

.model-card {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-md) var(--space-lg);
  border: 1px solid var(--clr-border-subtle);
  border-radius: var(--radius);
  background: var(--clr-surface);
}

.model-tokens {
  font-size: 12px;
  font-family: var(--font-mono);
  color: var(--clr-text-muted);
  padding: 4px 10px;
  background: var(--clr-surface-raised);
  border-radius: var(--radius);
}

/* Modal */
.modal-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.6);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
  backdrop-filter: blur(4px);
}

.modal-card {
  width: 400px;
  background: var(--clr-surface);
  border: 1px solid var(--clr-border);
  border-radius: var(--radius-xl);
  overflow: hidden;
}

.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-lg);
  border-bottom: 1px solid var(--clr-border-subtle);
}

.modal-title {
  font-size: 16px;
  font-weight: 600;
  color: var(--clr-text);
}

.modal-close {
  background: none;
  border: none;
  color: var(--clr-text-muted);
  cursor: pointer;
  padding: var(--space-xs);
  border-radius: var(--radius);
  transition: all 0.15s var(--ease);
}

.modal-close:hover {
  color: var(--clr-text);
  background: var(--clr-surface-hover);
}

.modal-body {
  padding: var(--space-lg);
  display: flex;
  flex-direction: column;
  gap: var(--space-md);
}

.modal-footer {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-sm);
  padding: var(--space-lg);
  border-top: 1px solid var(--clr-border-subtle);
}

.input-group {
  display: flex;
  flex-direction: column;
  gap: var(--space-xs);
}

.input-label {
  font-size: 11px;
  font-weight: 500;
  letter-spacing: 0.05em;
  text-transform: uppercase;
  color: var(--clr-text-muted);
}

.input {
  background: var(--clr-surface-raised);
  border: 1px solid var(--clr-border);
  border-radius: var(--radius);
  padding: 10px var(--space-md);
  font-size: 13px;
  font-family: var(--font-sans);
  color: var(--clr-text);
  transition: all 0.15s var(--ease);
}

.input:focus {
  outline: none;
  border-color: var(--clr-primary);
  box-shadow: 0 0 0 3px var(--clr-primary-glow);
}

.input::placeholder {
  color: var(--clr-text-muted);
}

.btn {
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-sm);
  padding: 10px var(--space-md);
  border-radius: var(--radius);
  font-size: 13px;
  font-weight: 500;
  font-family: inherit;
  cursor: pointer;
  border: 1px solid transparent;
  transition: all 0.15s var(--ease);
  white-space: nowrap;
}

.btn:active {
  transform: scale(0.98);
}

.btn-primary {
  background: var(--clr-primary);
  color: var(--clr-bg);
  border: none;
}

.btn-primary:hover {
  background: var(--clr-primary-dim);
}

.btn-primary:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.btn-secondary {
  background: var(--clr-surface-raised);
  color: var(--clr-text-secondary);
  border-color: var(--clr-border);
}

.btn-secondary:hover {
  color: var(--clr-text);
  background: var(--clr-surface-hover);
}

.btn-ghost {
  background: transparent;
  color: var(--clr-text-secondary);
  border-color: transparent;
}

.btn-ghost:hover {
  color: var(--clr-text);
  background: var(--clr-surface-hover);
}

.btn-sm {
  padding: 6px var(--space-sm);
  font-size: 12px;
}

.badge {
  display: inline-flex;
  align-items: center;
  padding: 4px 10px;
  border-radius: 100px;
  font-size: 11px;
  font-weight: 500;
}

.badge-green {
  background: var(--clr-primary-glow);
  color: var(--clr-primary);
}

.badge-amber {
  background: var(--clr-amber-glow);
  color: var(--clr-amber);
}

.animate-spin {
  animation: spin 1s linear infinite;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.opacity-40 {
  opacity: 0.4;
}
</style>
