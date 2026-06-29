<script setup lang="ts">
import { ref, onMounted } from 'vue'
import { useApi } from '../composables/useApi'
import Card from '../components/ui/Card.vue'
import Button from '../components/ui/Button.vue'
import Input from '../components/ui/Input.vue'
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
  <div class="flex-1 overflow-y-auto px-6 py-5">
    <div class="max-w-3xl space-y-5 stagger">

      <!-- Header -->
      <div class="flex items-center justify-between">
        <h2 class="text-sm font-semibold tracking-wide">SETTINGS</h2>
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
      <Card v-if="showInput" glow="cyan" class="anim-slide-up">
        <div class="space-y-4">
          <div>
            <label class="text-[10px] font-mono text-[var(--text-muted)] tracking-widest">PROVIDER</label>
            <div class="flex gap-2 mt-1">
              <Input
                v-model="newProvider"
                placeholder="Provider name (e.g., nan, openai)"
                class="flex-1"
              />
            </div>
          </div>

          <div>
            <label class="text-[10px] font-mono text-[var(--text-muted)] tracking-widest">API KEY</label>
            <Input
              v-model="newApiKey"
              type="password"
              placeholder="sk-xxx..."
              class="mt-1"
              @keyup.enter="saveKey"
            />
          </div>

          <div class="flex items-center gap-2 pt-2">
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
        <div class="grid grid-cols-2 gap-2">
          <button
            v-for="p in knownProviders"
            :key="p.name"
            @click="selectKnownProvider(p.name)"
            class="flex items-center gap-3 p-3 rounded-lg border border-[var(--border-subtle)] hover:border-[var(--cyan)] hover:bg-[var(--cyan-ghost)] transition-all text-left group"
          >
            <Icon name="shield" :size="16" class="text-[var(--text-ghost)] group-hover:text-[var(--cyan)] transition-colors" />
            <div>
              <div class="text-xs font-semibold text-[var(--text-primary)]">{{ p.label }}</div>
              <div class="text-[10px] text-[var(--text-muted)] font-mono">{{ p.desc }}</div>
            </div>
          </button>
        </div>
      </Card>

      <!-- Stored Keys -->
      <Card title="STORED KEYS" :subtitle="`${providers.length} configured`">
        <div v-if="loading" class="text-xs text-[var(--text-muted)] font-mono py-4 text-center">
          Loading...
        </div>

        <div v-else-if="providers.length === 0" class="text-xs text-[var(--text-muted)] font-mono py-8 text-center">
          <Icon name="shield-off" :size="20" class="mx-auto mb-2 text-[var(--text-ghost)]" />
          No provider keys stored. Add one above.
        </div>

        <div v-else class="space-y-2">
          <div
            v-for="p in providers"
            :key="p.provider"
            class="flex items-center justify-between p-3 rounded-lg border border-[var(--border-subtle)] hover:border-[var(--border)] transition-colors"
          >
            <div class="flex items-center gap-3">
              <Icon
                :name="p.has_key ? 'shield-check' : 'shield-off'"
                :size="16"
                :color="p.has_key ? 'var(--emerald)' : 'var(--text-ghost)'"
              />
              <div>
                <div class="text-xs font-semibold text-[var(--text-primary)] capitalize">{{ p.provider }}</div>
                <div class="text-[10px] font-mono text-[var(--text-muted)]">{{ p.key_masked || 'No key' }}</div>
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
                class="p-1.5 rounded-md text-[var(--text-ghost)] hover:text-[var(--crimson)] hover:bg-[var(--crimson-glow)] transition-all"
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
          <Icon name="shield-lock" :size="18" class="text-[var(--amber)] shrink-0 mt-0.5" />
          <div>
            <div class="text-xs font-semibold text-[var(--text-primary)] mb-1">Security</div>
            <div class="text-[10px] text-[var(--text-muted)] leading-relaxed">
              API keys are encrypted with AES-256-GCM and stored in
              <code class="text-[var(--cyan)] font-mono">.forge/credentials.vault.json</code>.
              Keys are never logged or transmitted in plaintext. To enable encryption, set the
              <code class="text-[var(--cyan)] font-mono">VAULT_PASSWORD</code> environment variable before starting the server.
            </div>
          </div>
        </div>
      </Card>

    </div>
  </div>
</template>
