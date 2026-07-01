<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useApi } from '../composables/useApi'
import Card from '../components/ui/Card.vue'
import Button from '../components/ui/Button.vue'
import Input from '../components/ui/Input.vue'
import Badge from '../components/ui/Badge.vue'
import Icon from '../components/ui/Icon.vue'

const api = useApi()

interface ProviderKey {
  provider: string
  key_masked: string
  has_key: boolean
}

const providers = ref<ProviderKey[]>([])
const loading = ref(false)
const saving = ref<string | null>(null)

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

async function loadProviders() {
  loading.value = true
  try {
    const data = await api.get<{ providers: ProviderKey[] }>('/vault/keys')
    providers.value = data.providers || []
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

async function deleteKey(provider: string) {
  if (!confirm(`Delete API key for "${provider}"?`)) return
  try {
    await api.del(`/vault/keys/${provider}`)
    await loadProviders()
  } catch (e) {
    console.error('Failed to delete key:', e)
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
  <div class="settings-content">
    <div class="max-w-3xl space-y-5 stagger">

      <!-- Header -->
      <div class="flex items-center justify-between">
        <h2 class="text-sm font-semibold tracking-wide mb-3">SETTINGS</h2>
        <Button
          v-if="!showInput"
          @click="showInput = true"
          variant="cyan"
          size="sm"
        >
          <Icon name="plus" :size="14" />
          Add Provider Key
        </Button>
      </div>

      <!-- Add Key Form -->
      <Card glow="cyan" v-if="showInput" class="anim-slide-up">
        <div class="space-y-4">
          <div>
            <label class="data-label mb-1">PROVIDER</label>
            <Input
              v-model="newProvider"
              placeholder="Provider name (e.g., nan, openai)"
            />
          </div>

          <div>
            <label class="data-label mb-1">API KEY</label>
            <Input
              v-model="newApiKey"
              type="password"
              placeholder="sk-xxx..."
              @keyup.enter="saveKey"
            />
          </div>

          <div class="flex items-center gap-3 pt-2">
            <Button
              @click="saveKey"
              variant="cyan"
              size="sm"
              :disabled="!newProvider || !newApiKey || saving !== null"
            >
              <Icon v-if="!saving" name="check" :size="14" />
              <Icon v-else name="loader" :size="14" class="animate-spin" />
              {{ saving ? 'Saving...' : 'Save Key' }}
            </Button>
            <Button @click="showInput = false" variant="ghost" size="sm">Cancel</Button>
          </div>
        </div>
      </Card>

      <!-- Known Providers Reference -->
      <Card title="KNOWN PROVIDERS" subtitle="click to auto-fill">
        <div class="known-providers-grid">
          <button
            v-for="p in knownProviders"
            :key="p.name"
            @click="selectKnownProvider(p.name)"
            class="known-provider-btn"
          >
            <Icon name="shield" :size="16" class="provider-icon" />
            <div>
              <div class="provider-label">{{ p.label }}</div>
              <div class="provider-desc">{{ p.desc }}</div>
            </div>
          </button>
        </div>
      </Card>

      <!-- Stored Keys -->
      <Card title="STORED KEYS" :subtitle="`${providers.length} configured`">
        <div v-if="loading" class="text-xs text-muted font-mono py-4 text-center">
          Loading...
        </div>

        <div v-else-if="providers.length === 0" class="text-xs text-muted font-mono py-8 text-center">
          <Icon name="shield-off" :size="20" class="mx-auto mb-2 opacity-40" />
          No provider keys stored. Add one above.
        </div>

        <div v-else class="space-y-2">
          <div
            v-for="p in providers"
            :key="p.provider"
            class="provider-row"
          >
            <div class="flex items-center gap-3">
              <Icon
                :name="p.has_key ? 'shield-check' : 'shield-off'"
                :size="16"
                :color="p.has_key ? 'var(--clr-emerald)' : 'var(--clr-text-muted)'"
              />
              <div>
                <div class="text-xs font-semibold text-primary capitalize">{{ p.provider }}</div>
                <div class="font-10 font-mono text-muted">{{ p.key_masked || 'No key' }}</div>
              </div>
            </div>
            <div class="flex items-center gap-2">
              <Badge
                :variant="p.has_key ? 'green' : 'gray'"
                size="sm"
              >
                {{ p.has_key ? 'Configured' : 'Missing' }}
              </Badge>
              <button
                @click="deleteKey(p.provider)"
                class="btn-icon-danger"
                title="Delete key"
              >
                <Icon name="trash" :size="14" />
              </button>
            </div>
          </div>
        </div>
      </Card>

      <!-- Security Notice -->
      <Card glow="amber" class="anim-slide-up">
        <div class="flex items-start gap-3">
          <Icon name="shield-lock" :size="18" class="text-amber shrink-0 mt-0.5" />
          <div>
            <div class="text-xs font-semibold text-primary mb-1">Security</div>
            <div class="font-10 text-muted leading-relaxed">
              API keys are encrypted with AES-256-GCM and stored in
              <code class="text-primary font-mono">.forge/credentials.vault.json</code>.
              Keys are never logged or transmitted in plaintext. To enable encryption, set the
              <code class="text-primary font-mono">VAULT_PASSWORD</code> environment variable before starting the server.
            </div>
          </div>
        </div>
      </Card>

    </div>
  </div>
</template>

<style scoped>
.settings-content {
  flex: 1;
  overflow-y: auto;
  padding: var(--space-xl);
}

.max-w-3xl {
  max-width: 720px;
}

.space-y-5 {
  display: flex;
  flex-direction: column;
  gap: var(--space-md);
}

.space-y-4 {
  display: flex;
  flex-direction: column;
  gap: var(--space-md);
}

.space-y-2 {
  display: flex;
  flex-direction: column;
  gap: var(--space-sm);
}

/* Known providers grid */
.known-providers-grid {
  display: grid;
  grid-template-columns: repeat(2, 1fr);
  gap: var(--space-sm);
}

.known-provider-btn {
  display: flex;
  align-items: center;
  gap: var(--space-md);
  padding: var(--space-md);
  border-radius: var(--radius);
  border: 1px solid var(--clr-border-subtle);
  background: transparent;
  cursor: pointer;
  transition: all 0.15s var(--ease);
  text-align: left;
}

.known-provider-btn:hover {
  border-color: var(--clr-primary);
  background: var(--clr-primary-glow);
}

.provider-icon {
  opacity: 0.4;
  transition: opacity 0.15s;
  color: var(--clr-text-muted);
}

.known-provider-btn:hover .provider-icon {
  opacity: 1;
  color: var(--clr-primary);
}

.provider-label {
  font-size: 12px;
  font-weight: 600;
  color: var(--clr-text);
}

.provider-desc {
  font-size: 10px;
  font-family: var(--font-mono);
  color: var(--clr-text-muted);
}

/* Provider row */
.provider-row {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-md);
  border-radius: var(--radius);
  border: 1px solid var(--clr-border-subtle);
  transition: border-color 0.15s, background 0.15s;
}

.provider-row:hover {
  border-color: var(--clr-border);
  background: rgba(255, 255, 255, 0.015);
}

.btn-icon-danger {
  padding: 6px;
  border-radius: 4px;
  background: transparent;
  border: none;
  color: var(--clr-text-muted);
  cursor: pointer;
  transition: all 0.15s;
  display: inline-flex;
  align-items: center;
}

.btn-icon-danger:hover {
  color: var(--clr-crimson);
  background: var(--clr-crimson-glow);
}

.mb-3 { margin-bottom: var(--space-md); }
.mb-2 { margin-bottom: var(--space-sm); }
.mb-1 { margin-bottom: var(--space-xs); }
.mx-auto { margin-left: auto; margin-right: auto; }
.text-center { text-align: center; }
.py-4 { padding-top: var(--space-md); padding-bottom: var(--space-md); }
.py-8 { padding-top: var(--space-xl); padding-bottom: var(--space-xl); }
.shrink-0 { flex-shrink: 0; }
.ml-auto { margin-left: auto; }
.text-amber { color: var(--clr-amber); }
</style>
