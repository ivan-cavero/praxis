<script setup lang="ts">
/**
 * ProjectChatView — Multi-agent chat with tabs per agent (Slack channels style).
 *
 * Each agent configured in the project gets its own chat tab.
 * Messages are isolated per agent. WebSocket events route to the correct tab.
 */
import { ref, computed, inject, onMounted, watch } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useAppStore } from '../stores/app'
import { useApi, type Project, type RoleDetail, type ProviderDetail } from '../composables/useApi'
import { useWebSocket, getEventPayload, type AgentOutputEvent } from '../composables/useWebSocket'
import Icon from '../components/ui/Icon.vue'

const openSettings = inject<() => void>('openSettings')

const route = useRoute()
const router = useRouter()
const store = useAppStore()
const api = useApi()
const ws = useWebSocket()

// ─── Project ───────────────────────────────────────────────────────

const project = ref<Project | null>(null)
const isLoading = ref(true)

const projectId = computed(() => route.params.id as string)

// ─── Agent roles from project config ───────────────────────────────

const agentRoles = ref<Record<string, RoleDetail>>({})
const agentList = computed(() => Object.keys(agentRoles.value))

// ─── Forge config providers (from the project's [providers] section) ──

const forgeProviders = ref<Record<string, ProviderDetail>>({})

// ─── Vault keys — which providers have API keys configured ─────────

const vaultProviders = ref<Set<string>>(new Set())

/**
 * Resolve provider info for each agent using forge config providers first,
 * falling back to model prefix detection only as a last resort.
 */
const agentProviderInfo = computed(() => {
  const info: Record<string, { provider: string; status: 'ok' | 'missing' | 'unknown' }> = {}
  const entries = Object.entries(forgeProviders.value)

  for (const [name, role] of Object.entries(agentRoles.value)) {
    // 1. Try exact default_model match against forge providers
    let matchedProvider: string | null = null
    for (const [pName, pDetail] of entries) {
      if (pDetail.default_model === role.model) {
        matchedProvider = pName
        break
      }
    }

    // 2. If no exact match, use the first forge provider (serves all models
    //    via a common API — typical for OpenAI-compatible proxies like NaN)
    if (matchedProvider === null && entries.length > 0) {
      matchedProvider = entries[0][0]
    }

    if (matchedProvider !== null) {
      info[name] = {
        provider: matchedProvider,
        status: vaultProviders.value.has(matchedProvider) ? 'ok' : 'missing',
      }
    } else {
      // 3. Fallback: prefix-based detection (no forge providers configured)
      const detected = detectProvider(role.model)
      info[name] = {
        provider: detected,
        status: detected === 'unknown' ? 'unknown'
          : vaultProviders.value.has(detected) ? 'ok' : 'missing',
      }
    }
  }
  return info
})

/** Prefix-based detection (fallback when forge config has no providers). */
const PROVIDER_PREFIXES: Record<string, string> = {
  'gpt-': 'openai',
  'text-embedding-': 'openai',
  'claude-': 'anthropic',
  'gemini-': 'gemini',
  'deepseek-': 'nan',
  'llama-': 'ollama',
  'mistral-': 'ollama',
  'qwen-': 'ollama',
  'codellama-': 'ollama',
}

function detectProvider(model: string): string {
  const lower = model.toLowerCase()
  const matched = Object.entries(PROVIDER_PREFIXES).find(([prefix]) => lower.startsWith(prefix))
  return matched ? matched[1] : 'unknown'
}

// ─── Tabs ──────────────────────────────────────────────────────────

const activeAgent = ref('')
const tabOrder = computed(() => ['all', ...agentList.value])

// ─── Chat messages per agent ───────────────────────────────────────

interface ChatMessage {
  id: string
  role: 'user' | 'assistant' | 'system'
  content: string
  timestamp: string
  agent?: string
}

const agentMessages = ref<Map<string, ChatMessage[]>>(new Map())

/** Messages for the currently active tab. */
const currentMessages = computed(() => {
  if (activeAgent.value === 'all') {
    // Merge all agent messages sorted by timestamp
    const all: ChatMessage[] = []
    for (const msgs of agentMessages.value.values()) {
      for (const m of msgs) all.push(m)
    }
    return all.sort((a, b) => a.timestamp.localeCompare(b.timestamp))
  }
  return agentMessages.value.get(activeAgent.value) || []
})

const inputText = ref('')
const isSending = ref(false)

/** Push a message to a specific agent's message list. */
function pushMessage(agent: string, msg: ChatMessage) {
  const key = agent || 'all'
  const existing = agentMessages.value.get(key) || []
  agentMessages.value = new Map(agentMessages.value).set(key, [...existing, msg])
}

// ─── Git branch ────────────────────────────────────────────────────

const gitBranch = ref('main')

onMounted(async () => {
  isLoading.value = true
  try {
    if (projectId.value) {
      project.value = await api.getProject(projectId.value)
      store.selectProject(projectId.value)

      // Load vault keys to know which providers are configured
      try {
        const vault = await api.getVaultKeys()
        vaultProviders.value = new Set(vault.providers.filter(p => p.has_key).map(p => p.provider))
      } catch {
        // Vault might not be accessible
      }

      // Load project config to get agent roles + providers
      try {
        const config = await api.getProjectConfig(projectId.value)
        agentRoles.value = config.roles
        forgeProviders.value = config.providers
        // Default to first agent tab
        if (agentList.value.length > 0) activeAgent.value = agentList.value[0]
      } catch {
        // No config yet
      }
    }
  } catch {
    // project not found
  }
  isLoading.value = false
})

// ─── Send a goal to the orchestrator ────────────────────────────────
//
// The orchestrator receives the goal and distributes work across agents
// through phases (plan → implement → review → test → consolidate).
// You never send to individual agents — one goal, full pipeline.

async function sendMessage() {
  if (!inputText.value.trim() || !projectId.value || !activeAgent.value) return
  const text = inputText.value.trim()
  inputText.value = ''

  // "All" tab = send to the orchestrator via the first agent entry point.
  // A single goal triggers the full multi-agent pipeline.
  const target = activeAgent.value === 'all' ? agentList.value[0] : activeAgent.value

  pushMessage(target, {
    id: crypto.randomUUID(),
    role: 'user',
    content: text,
    timestamp: new Date().toISOString(),
    agent: target,
  })

  isSending.value = true
  try {
    await api.sendInject(target, 'goal', text)
    pushMessage(target, {
      id: crypto.randomUUID(),
      role: 'system',
      content: `Goal dispatched — orchestrator distributing across agents`,
      timestamp: new Date().toISOString(),
    })
  } catch {
    pushMessage(target, {
      id: crypto.randomUUID(),
      role: 'system',
      content: 'Failed to dispatch goal. Is the backend running?',
      timestamp: new Date().toISOString(),
    })
  }
  isSending.value = false
}

// ─── Listen for agent output via WebSocket ─────────────────────────

watch(() => ws.events.value, (allEvents) => {
  for (const event of allEvents) {
    const agentOut = getEventPayload<AgentOutputEvent>(event, 'AgentOutput')
    if (agentOut && agentOut.delta && agentOut.agent) {
      pushMessage(agentOut.agent, {
        id: crypto.randomUUID(),
        role: 'assistant',
        content: agentOut.delta,
        timestamp: event.timestamp,
        agent: agentOut.agent,
      })
    }
  }
}, { deep: true })
</script>

<template>
  <div class="chat-view">
    <!-- Header -->
    <div class="chat-header">
      <button class="chat-back" @click="router.push('/')" title="Back to Dashboard">
        <Icon name="arrow-left" :size="18" />
      </button>
      <div class="chat-header-info">
        <h1 class="chat-title" v-if="project">{{ project.name }}</h1>
        <p class="chat-subtitle" v-if="project">{{ project.description || 'No description' }}</p>
        <p class="chat-subtitle" v-else-if="isLoading">Loading project...</p>
      </div>

      <!-- Settings + Git branch -->
      <div class="chat-header-actions">
        <button
          v-if="openSettings"
          class="chat-settings-btn"
          @click="openSettings()"
          title="Settings"
        >
          <Icon name="settings" :size="15" />
        </button>
        <div class="chat-git">
          <Icon name="git-branch" :size="14" />
          <input
            v-model="gitBranch"
            class="chat-git-input"
            placeholder="branch"
            title="Git branch"
          />
        </div>
      </div>
    </div>

    <!-- Agent tabs (compact — provider status indicator) -->
    <div v-if="agentList.length > 0" class="chat-tabs">
      <button
        v-for="tab in tabOrder"
        :key="tab"
        class="chat-tab"
        :class="{
          active: activeAgent === tab,
          'tab-provider-missing': tab !== 'all' && agentProviderInfo[tab]?.status === 'missing',
          'tab-provider-unknown': tab !== 'all' && agentProviderInfo[tab]?.status === 'unknown',
        }"
        :title="tab === 'all'
          ? 'Show all messages'
          : `${tab} — ${agentRoles[tab]?.model || 'no model'} via ${agentProviderInfo[tab]?.provider || '?'} (${agentProviderInfo[tab]?.status === 'ok' ? '✅ key configured' : agentProviderInfo[tab]?.status === 'missing' ? '⚠️ no API key' : '❓ unknown provider'})`"
        @click="activeAgent = tab"
      >
        <template v-if="tab === 'all'">
          <Icon name="list" :size="12" />
          All
        </template>
        <template v-else>
          <!-- Warning dot if provider missing -->
          <span
            v-if="agentProviderInfo[tab]?.status === 'missing'"
            class="tab-warn-dot"
            title="No API key configured for this provider"
          />
          <span>{{ tab }}</span>
        </template>
      </button>
    </div>
    <div v-else class="chat-tabs chat-tabs-empty">
      <span class="chat-no-agents">
        <Icon name="robot" :size="12" />
        No agents configured — configure providers and roles in Settings
      </span>
    </div>

    <!-- Orchestrator hint (shown in 'All' tab) -->
    <div v-if="activeAgent === 'all'" class="chat-orch-hint">
      <Icon name="list" :size="12" />
      One goal → orchestrator → agents → done
    </div>

    <!-- Messages -->
    <div class="chat-messages">
      <div v-if="currentMessages.length === 0" class="chat-empty">
        <Icon name="message" :size="48" class="empty-icon" />
        <template v-if="activeAgent === 'all'">
          <p>No messages yet</p>
          <p class="empty-hint">Send a goal and the orchestrator will distribute it across agents</p>
        </template>
        <template v-else>
          <p>Chat with <strong>{{ activeAgent }}</strong></p>
          <p class="empty-hint">Send a goal and this agent will work through it</p>
        </template>
      </div>
      <div v-else v-for="msg in currentMessages" :key="msg.id" class="chat-msg" :class="msg.role">
        <div class="msg-avatar">
          <template v-if="msg.role === 'user'">U</template>
          <template v-else-if="msg.role === 'assistant'">{{ (msg.agent || 'A')[0].toUpperCase() }}</template>
          <template v-else>●</template>
        </div>
        <div class="msg-body">
          <div class="msg-header">
            <span class="msg-role">{{ msg.role === 'user' ? 'You' : msg.role === 'assistant' ? (msg.agent || 'Agent') : 'System' }}</span>
            <span class="msg-time">{{ msg.timestamp.slice(11, 19) }}</span>
          </div>
          <div class="msg-content">{{ msg.content }}</div>
        </div>
      </div>
    </div>

    <!-- Input — send to single agent or all agents -->
    <div class="chat-input">
      <input
        v-model="inputText"
        class="chat-input-field"
        :placeholder="activeAgent === 'all' ? 'Describe what you want to build...' : activeAgent ? `Message ${activeAgent}...` : 'Select an agent tab to chat'"
        :disabled="isSending || !activeAgent"
        @keydown.enter="sendMessage"
      />
      <button
        class="chat-send-btn"
        :disabled="!inputText.trim() || isSending || !activeAgent || !projectId"
        @click="sendMessage"
      >
        <Icon v-if="isSending" name="refresh" :size="16" class="animate-spin" />
        <Icon v-else name="send" :size="16" />
      </button>
    </div>
  </div>
</template>

<style scoped>
.chat-view {
  display: flex;
  flex-direction: column;
  height: 100%;
  width: 100%;
  background: var(--bg-base);
}

/* ─── Header ─────────────────────────────────────────────────────── */

.chat-header {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-3) var(--space-4);
  background: var(--bg-surface);
  border-bottom: 1px solid var(--border-subtle);
  flex-shrink: 0;
}

.chat-back {
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
  transition: all var(--transition-fast);
}
.chat-back:hover {
  color: var(--text-primary);
  background: var(--bg-hover);
}

.chat-header-info { flex: 1; min-width: 0; }
.chat-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--text-primary);
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}
.chat-subtitle {
  font-size: 12px;
  color: var(--text-muted);
  margin-top: 1px;
  white-space: nowrap;
  overflow: hidden;
  text-overflow: ellipsis;
}

.chat-header-controls {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.chat-no-agents {
  display: inline-flex;
  align-items: center;
  gap: 4px;
  font-size: 11px;
  color: var(--text-disabled);
  padding: 2px 8px;
  border-radius: var(--radius-full);
  background: var(--bg-elevated);
  border: 1px dashed var(--border-subtle);
}

/* ─── Agent Tabs ──────────────────────────────────────────── */

.chat-tabs {
  display: flex;
  align-items: stretch;
  gap: 0;
  padding: 0 var(--space-4);
  background: var(--bg-surface);
  border-bottom: 1px solid var(--border-subtle);
  flex-shrink: 0;
  overflow-x: auto;
}
.chat-tabs-empty {
  padding: var(--space-2) var(--space-4);
  align-items: center;
}

.chat-tab {
  display: inline-flex;
  align-items: center;
  gap: var(--space-1);
  padding: var(--space-1) var(--space-2);
  border: none;
  background: transparent;
  color: var(--text-muted);
  font-size: 12px;
  font-family: inherit;
  cursor: pointer;
  white-space: nowrap;
  border-bottom: 2px solid transparent;
  margin-bottom: -1px;
  transition: all var(--transition-fast);
  position: relative;
}
.chat-tab:hover {
  color: var(--text-secondary);
  background: var(--bg-hover);
}
.chat-tab.active {
  color: var(--text-primary);
  border-bottom-color: var(--primary);
  font-weight: 500;
}

.tab-model {
  font-family: var(--font-mono, monospace);
  font-size: 10px;
  color: var(--text-disabled);
  margin-left: 2px;
}

/* Provider warning states */
.tab-provider-missing {
  color: var(--warning) !important;
}
.tab-provider-missing:hover {
  background: color-mix(in srgb, var(--warning) 10%, transparent) !important;
}
.tab-provider-missing.active {
  border-bottom-color: var(--warning) !important;
}

.tab-warn-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--warning);
  flex-shrink: 0;
  animation: warnPulse 2s infinite;
}
@keyframes warnPulse {
  0%, 100% { opacity: 1; }
  50% { opacity: 0.4; }
}

.chat-header-actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.chat-settings-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 28px;
  height: 28px;
  border: none;
  border-radius: var(--radius-md);
  background: transparent;
  color: var(--text-muted);
  cursor: pointer;
  transition: all var(--transition-fast);
}
.chat-settings-btn:hover {
  color: var(--text-primary);
  background: var(--bg-hover);
}

.chat-git {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  padding: var(--space-1) var(--space-2);
  border-radius: var(--radius-md);
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  color: var(--text-muted);
}
.chat-git-input {
  width: 70px;
  background: transparent;
  border: none;
  color: var(--text-secondary);
  font-size: 12px;
  font-family: inherit;
  outline: none;
}
.chat-git-input::placeholder { color: var(--text-disabled); }

/* ─── Messages ───────────────────────────────────────────────────── */

.chat-messages {
  flex: 1;
  overflow-y: auto;
  padding: var(--space-4);
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
  min-height: 0;
}

.chat-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  flex: 1;
  color: var(--text-muted);
  gap: var(--space-3);
  text-align: center;
}
.empty-icon { opacity: 0.3; }
.empty-hint { font-size: 13px; opacity: 0.6; }

.chat-msg {
  display: flex;
  gap: var(--space-3);
  animation: msgFadeIn 0.2s ease-out;
}

@keyframes msgFadeIn {
  from { opacity: 0; transform: translateY(4px); }
  to { opacity: 1; transform: translateY(0); }
}

.msg-avatar {
  width: 28px;
  height: 28px;
  border-radius: var(--radius-full);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 11px;
  font-weight: 700;
  flex-shrink: 0;
  margin-top: 2px;
}
.chat-msg.user .msg-avatar {
  background: var(--primary);
  color: var(--bg-base);
}
.chat-msg.assistant .msg-avatar {
  background: var(--bg-elevated);
  color: var(--text-primary);
}
.chat-msg.system .msg-avatar {
  background: transparent;
  color: var(--text-muted);
  font-size: 16px;
}

.msg-body { flex: 1; min-width: 0; }
.msg-header {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin-bottom: var(--space-1);
}
.msg-role {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-primary);
}
.msg-time {
  font-size: 11px;
  color: var(--text-disabled);
}
.msg-content {
  font-size: 14px;
  line-height: 1.6;
  color: var(--text-secondary);
  white-space: pre-wrap;
  word-break: break-word;
}
.chat-msg.user .msg-content {
  color: var(--text-primary);
}
.chat-msg.system .msg-content {
  color: var(--text-muted);
  font-size: 13px;
  font-style: italic;
}

/* ─── Input ──────────────────────────────────────────────────────── */

.chat-input {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-3) var(--space-4);
  background: var(--bg-surface);
  border-top: 1px solid var(--border-subtle);
  flex-shrink: 0;
}

.chat-input-field {
  flex: 1;
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-md);
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  color: var(--text-primary);
  font-size: 14px;
  font-family: inherit;
  transition: border-color var(--transition-fast);
}
.chat-input-field:focus {
  outline: none;
  border-color: var(--primary);
  box-shadow: 0 0 0 2px var(--primary-muted);
}
.chat-input-field::placeholder { color: var(--text-muted); }
.chat-input-field:disabled { opacity: 0.5; }

.chat-send-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  border: none;
  border-radius: var(--radius-full);
  background: var(--primary);
  color: var(--bg-base);
  cursor: pointer;
  transition: all var(--transition-fast);
  flex-shrink: 0;
}
.chat-send-btn:hover:not(:disabled) { background: var(--primary-hover); transform: scale(1.05); }
.chat-send-btn:disabled { opacity: 0.4; cursor: not-allowed; transform: none; }

/* ─── Orchestrator hint ─────────────────────────────────────────── */

.chat-orch-hint {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  padding: var(--space-1) var(--space-4);
  font-size: 12px;
  color: var(--text-muted);
  border-bottom: 1px solid var(--border-subtle);
  background: var(--bg-surface);
  flex-shrink: 0;
}
</style>
