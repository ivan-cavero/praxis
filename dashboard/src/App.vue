<script setup lang="ts">
import { ref, onMounted, watch } from 'vue'

// ─── Configuration ─────────────────────────────────────

interface Provider {
  name: string
  baseUrl: string
  apiKey: string
  models: string[]
}

interface Message {
  id: string
  role: 'user' | 'assistant' | 'system'
  content: string
  timestamp: Date
  model?: string
  tokens?: { input: number; output: number }
}

// ─── State ─────────────────────────────────────────────

const currentView = ref('chat')
const sidebarCollapsed = ref(false)

// Providers
const providers = ref<Provider[]>([])
const activeProvider = ref<string>('')
const activeModel = ref<string>('')

// Chat
const messages = ref<Message[]>([])
const inputMessage = ref('')
const isLoading = ref(false)

// Metrics
const totalTokens = ref(0)
const totalCost = ref(0)
const sessionCount = ref(1)

// Config panel
const showConfig = ref(false)
const newProvider = ref({ name: '', baseUrl: '', apiKey: '' })

// ─── Init ──────────────────────────────────────────────

onMounted(() => {
  // Load saved config from localStorage
  const saved = localStorage.getItem('project-x-config')
  if (saved) {
    try {
      const config = JSON.parse(saved)
      providers.value = config.providers || []
      if (providers.value.length > 0) {
        activeProvider.value = providers.value[0].name
        activeModel.value = providers.value[0].models[0] || ''
      }
    } catch {}
  }
})

// ─── Save Config ───────────────────────────────────────

function saveConfig() {
  localStorage.setItem('project-x-config', JSON.stringify({
    providers: providers.value,
    activeProvider: activeProvider.value,
    activeModel: activeModel.value,
  }))
}

// ─── Provider Management ───────────────────────────────

function addProvider() {
  if (newProvider.value.name && newProvider.value.baseUrl) {
    providers.value.push({
      name: newProvider.value.name,
      baseUrl: newProvider.value.baseUrl,
      apiKey: newProvider.value.apiKey,
      models: [],
    })
    if (!activeProvider.value) activeProvider.value = newProvider.value.name
    saveConfig()
    newProvider.value = { name: '', baseUrl: '', apiKey: '' }
    showConfig.value = false
  }
}

function removeProvider(name: string) {
  providers.value = providers.value.filter(p => p.name !== name)
  if (activeProvider.value === name) {
    activeProvider.value = providers.value[0]?.name || ''
    activeModel.value = providers.value[0]?.models[0] || ''
  }
  saveConfig()
}

// ─── Chat ──────────────────────────────────────────────

async function sendMessage() {
  if (!inputMessage.value.trim() || isLoading.value) return
  if (!activeProvider.value) {
    alert('Please add a provider first (Config → Add Provider)')
    return
  }

  const provider = providers.value.find(p => p.name === activeProvider.value)
  if (!provider) return

  const userMsg: Message = {
    id: Date.now().toString(),
    role: 'user',
    content: inputMessage.value,
    timestamp: new Date(),
  }
  messages.value.push(userMsg)
  inputMessage.value = ''
  isLoading.value = true

  try {
    const response = await fetch(`${provider.baseUrl}/chat/completions`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${provider.apiKey}`,
      },
      body: JSON.stringify({
        model: activeModel.value || provider.models[0],
        messages: messages.value.map(m => ({
          role: m.role,
          content: m.content,
        })),
        stream: false,
      }),
    })

    if (!response.ok) throw new Error(`API error: ${response.status}`)

    const data = await response.json()
    const assistantMsg: Message = {
      id: Date.now().toString(),
      role: 'assistant',
      content: data.choices?.[0]?.message?.content || 'No response',
      timestamp: new Date(),
      model: data.model,
      tokens: data.usage ? {
        input: data.usage.prompt_tokens,
        output: data.usage.completion_tokens,
      } : undefined,
    }

    messages.value.push(assistantMsg)

    // Update metrics
    if (data.usage) {
      totalTokens.value += data.usage.total_tokens || 0
    }
  } catch (error: any) {
    const errorMsg: Message = {
      id: Date.now().toString(),
      role: 'system',
      content: `Error: ${error.message}`,
      timestamp: new Date(),
    }
    messages.value.push(errorMsg)
  } finally {
    isLoading.value = false
  }
}

function clearChat() {
  messages.value = []
}

// ─── Navigation ────────────────────────────────────────

const navItems = [
  { id: 'chat', label: 'Chat', icon: '💬' },
  { id: 'agents', label: 'Agents', icon: '🤖' },
  { id: 'sessions', label: 'Sessions', icon: '📋' },
  { id: 'context', label: 'Context', icon: '🧠' },
  { id: 'config', label: 'Config', icon: '⚙️' },
  { id: 'logs', label: 'Logs', icon: '📊' },
]

// ─── Auto-scroll ───────────────────────────────────────

const chatContainer = ref<HTMLElement>()
watch(messages, () => {
  setTimeout(() => {
    chatContainer.value?.scrollTo(0, chatContainer.value.scrollHeight)
  }, 100)
}, { deep: true })
</script>

<template>
  <div class="h-screen flex bg-[#0a0a0a] text-[#e5e5e5] font-sans antialiased">

    <!-- Sidebar -->
    <aside
      class="bg-[#111111] border-r border-[#1f1f1f] flex flex-col transition-all duration-300"
      :class="sidebarCollapsed ? 'w-16' : 'w-64'"
    >
      <!-- Logo -->
      <div class="h-14 border-b border-[#1f1f1f] flex items-center px-4">
        <div class="w-8 h-8 rounded-lg bg-gradient-to-br from-green-500 to-emerald-600 flex items-center justify-center text-white font-bold text-sm">X</div>
        <div v-if="!sidebarCollapsed" class="ml-3">
          <div class="text-sm font-semibold text-white">Project-X</div>
          <div class="text-[10px] text-gray-500">v1.0.0</div>
        </div>
      </div>

      <!-- Nav -->
      <nav class="flex-1 py-4 px-2">
        <button
          v-for="item in navItems"
          :key="item.id"
          @click="currentView = item.id"
          class="w-full flex items-center gap-3 px-3 py-2.5 rounded-lg text-sm transition-all mb-1"
          :class="currentView === item.id
            ? 'bg-green-500/10 text-green-400'
            : 'text-gray-400 hover:bg-white/5 hover:text-white'"
        >
          <span class="text-lg w-6 text-center">{{ item.icon }}</span>
          <span v-if="!sidebarCollapsed">{{ item.label }}</span>
        </button>
      </nav>

      <!-- Footer -->
      <div class="p-4 border-t border-[#1f1f1f]">
        <div class="flex items-center gap-2 text-xs text-gray-500">
          <div class="w-2 h-2 rounded-full bg-green-500"></div>
          <span v-if="!sidebarCollapsed">Online</span>
        </div>
      </div>
    </aside>

    <!-- Main Content -->
    <div class="flex-1 flex flex-col min-w-0">

      <!-- Top Bar -->
      <header class="h-14 border-b border-[#1f1f1f] bg-[#111111]/50 backdrop-blur-sm flex items-center px-6 justify-between">
        <div class="flex items-center gap-4">
          <button @click="sidebarCollapsed = !sidebarCollapsed" class="text-gray-400 hover:text-white">
            <svg class="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
              <path stroke-linecap="round" stroke-linejoin="round" stroke-width="2" d="M4 6h16M4 12h16M4 18h16"/>
            </svg>
          </button>
          <span class="text-sm font-medium">{{ navItems.find(n => n.id === currentView)?.label }}</span>
        </div>
        <div class="flex items-center gap-6 text-xs text-gray-500">
          <span>Tokens: <span class="text-white">{{ totalTokens.toLocaleString() }}</span></span>
          <span>Cost: <span class="text-green-400">${{ totalCost.toFixed(4) }}</span></span>
          <span>Sessions: <span class="text-white">{{ sessionCount }}</span></span>
        </div>
      </header>

      <!-- Views -->
      <main class="flex-1 overflow-hidden">

        <!-- ═══ CHAT VIEW ═══ -->
        <div v-if="currentView === 'chat'" class="h-full flex flex-col">
          <!-- Messages -->
          <div ref="chatContainer" class="flex-1 overflow-y-auto p-6 space-y-4">
            <div v-if="messages.length === 0" class="h-full flex items-center justify-center">
              <div class="text-center">
                <div class="text-4xl mb-4">💬</div>
                <div class="text-lg text-gray-400 mb-2">Start a conversation</div>
                <div class="text-sm text-gray-600">
                  {{ activeProvider ? `Using ${activeModel} via ${activeProvider}` : 'Add a provider in Config to get started' }}
                </div>
              </div>
            </div>

            <div v-for="msg in messages" :key="msg.id" class="max-w-3xl mx-auto">
              <div class="flex gap-3" :class="msg.role === 'assistant' ? 'flex-row' : 'flex-row-reverse'">
                <!-- Avatar -->
                <div class="w-8 h-8 rounded-full flex items-center justify-center shrink-0 text-sm"
                  :class="msg.role === 'user' ? 'bg-green-500/20 text-green-400' : msg.role === 'system' ? 'bg-red-500/20 text-red-400' : 'bg-blue-500/20 text-blue-400'">
                  {{ msg.role === 'user' ? '👤' : msg.role === 'system' ? '⚠' : '🤖' }}
                </div>
                <!-- Content -->
                <div class="flex-1 min-w-0">
                  <div class="text-[10px] text-gray-500 mb-1 flex items-center gap-2">
                    <span>{{ msg.role }}</span>
                    <span v-if="msg.model" class="text-gray-600">• {{ msg.model }}</span>
                    <span class="text-gray-600">• {{ msg.timestamp.toLocaleTimeString() }}</span>
                  </div>
                  <div class="bg-[#1a1a1a] rounded-xl px-4 py-3 text-sm leading-relaxed whitespace-pre-wrap"
                    :class="msg.role === 'system' ? 'border border-red-500/20 text-red-300' : ''">
                    {{ msg.content }}
                  </div>
                  <div v-if="msg.tokens" class="text-[10px] text-gray-600 mt-1">
                    {{ msg.tokens.input }} in / {{ msg.tokens.output }} out
                  </div>
                </div>
              </div>
            </div>

            <!-- Loading indicator -->
            <div v-if="isLoading" class="max-w-3xl mx-auto flex gap-3">
              <div class="w-8 h-8 rounded-full bg-blue-500/20 flex items-center justify-center text-sm">🤖</div>
              <div class="bg-[#1a1a1a] rounded-xl px-4 py-3">
                <div class="flex gap-1">
                  <div class="w-2 h-2 bg-gray-500 rounded-full animate-bounce" style="animation-delay: 0s"></div>
                  <div class="w-2 h-2 bg-gray-500 rounded-full animate-bounce" style="animation-delay: 0.1s"></div>
                  <div class="w-2 h-2 bg-gray-500 rounded-full animate-bounce" style="animation-delay: 0.2s"></div>
                </div>
              </div>
            </div>
          </div>

          <!-- Input -->
          <div class="border-t border-[#1f1f1f] p-4">
            <div class="max-w-3xl mx-auto flex gap-3">
              <input
                v-model="inputMessage"
                @keyup.enter="sendMessage"
                :placeholder="activeProvider ? `Message ${activeModel}...` : 'Add a provider first (Config)'"
                class="flex-1 bg-[#1a1a1a] border border-[#2a2a2a] rounded-xl px-4 py-3 text-sm text-white placeholder-gray-500 focus:outline-none focus:border-green-500/50 transition-colors"
                :disabled="isLoading || !activeProvider"
              />
              <button
                @click="sendMessage"
                :disabled="isLoading || !inputMessage.trim() || !activeProvider"
                class="px-5 py-3 bg-green-600 hover:bg-green-500 disabled:bg-gray-700 disabled:text-gray-500 rounded-xl text-sm font-medium transition-colors"
              >
                Send
              </button>
              <button @click="clearChat" class="px-3 py-3 bg-[#1a1a1a] hover:bg-[#252525] rounded-xl text-gray-400 text-sm transition-colors">
                Clear
              </button>
            </div>
          </div>
        </div>

        <!-- ═══ CONFIG VIEW ═══ -->
        <div v-else-if="currentView === 'config'" class="p-8 overflow-y-auto h-full">
          <div class="max-w-2xl mx-auto space-y-8">

            <!-- Active Provider -->
            <section>
              <h2 class="text-lg font-semibold mb-4">Active Provider</h2>
              <div class="bg-[#111111] border border-[#1f1f1f] rounded-2xl p-6">
                <div class="grid grid-cols-2 gap-4 mb-4">
                  <div>
                    <label class="text-xs text-gray-500 block mb-1">Provider</label>
                    <select v-model="activeProvider" @change="saveConfig"
                      class="w-full bg-[#1a1a1a] border border-[#2a2a2a] rounded-lg px-3 py-2.5 text-sm text-white focus:border-green-500/50">
                      <option value="">Select provider</option>
                      <option v-for="p in providers" :key="p.name" :value="p.name">{{ p.name }}</option>
                    </select>
                  </div>
                  <div>
                    <label class="text-xs text-gray-500 block mb-1">Model</label>
                    <select v-model="activeModel" @change="saveConfig"
                      class="w-full bg-[#1a1a1a] border border-[#2a2a2a] rounded-lg px-3 py-2.5 text-sm text-white focus:border-green-500/50">
                      <option value="">Select model</option>
                      <option v-for="m in providers.find(p => p.name === activeProvider)?.models || []" :key="m" :value="m">{{ m }}</option>
                    </select>
                  </div>
                </div>
                <div v-if="activeModel" class="text-xs text-gray-500">
                  Active: <span class="text-green-400">{{ activeModel }}</span> via <span class="text-white">{{ activeProvider }}</span>
                </div>
              </div>
            </section>

            <!-- Providers -->
            <section>
              <div class="flex items-center justify-between mb-4">
                <h2 class="text-lg font-semibold">Providers</h2>
                <button @click="showConfig = !showConfig"
                  class="px-4 py-2 bg-green-600 hover:bg-green-500 rounded-lg text-sm font-medium transition-colors">
                  {{ showConfig ? 'Cancel' : '+ Add Provider' }}
                </button>
              </div>

              <!-- Provider cards -->
              <div class="space-y-3">
                <div v-for="p in providers" :key="p.name"
                  class="bg-[#111111] border border-[#1f1f1f] rounded-2xl p-5">
                  <div class="flex items-center justify-between mb-2">
                    <div class="flex items-center gap-2">
                      <div class="w-2 h-2 rounded-full bg-green-500"></div>
                      <span class="font-medium">{{ p.name }}</span>
                    </div>
                    <button @click="removeProvider(p.name)" class="text-xs text-gray-500 hover:text-red-400">Remove</button>
                  </div>
                  <div class="text-xs text-gray-500 font-mono mb-2">{{ p.baseUrl }}</div>
                  <div class="flex gap-2 flex-wrap">
                    <span v-for="m in p.models" :key="m"
                      class="text-xs px-2.5 py-1 rounded-full bg-white/5 text-gray-400">{{ m }}</span>
                  </div>
                </div>

                <!-- Empty state -->
                <div v-if="providers.length === 0 && !showConfig" class="text-center py-12 text-gray-500">
                  <div class="text-4xl mb-3">🔌</div>
                  <div class="text-sm">No providers configured</div>
                  <div class="text-xs mt-1 text-gray-600">Click "Add Provider" to connect to an AI API</div>
                </div>
              </div>

              <!-- Add Provider Form -->
              <div v-if="showConfig" class="mt-4 bg-[#111111] border border-green-500/30 rounded-2xl p-6 space-y-4">
                <h3 class="font-medium">Add Provider</h3>
                <div class="text-xs text-gray-500 mb-2">Any OpenAI-compatible API works</div>
                <input v-model="newProvider.name" placeholder="Name (e.g., nan, deepseek, openai)"
                  class="w-full bg-[#1a1a1a] border border-[#2a2a2a] rounded-lg px-4 py-3 text-sm text-white placeholder-gray-500 focus:border-green-500/50" />
                <input v-model="newProvider.baseUrl" placeholder="Base URL (https://api.example.com/v1)"
                  class="w-full bg-[#1a1a1a] border border-[#2a2a2a] rounded-lg px-4 py-3 text-sm text-white placeholder-gray-500 focus:border-green-500/50" />
                <input v-model="newProvider.apiKey" placeholder="API Key" type="password"
                  class="w-full bg-[#1a1a1a] border border-[#2a2a2a] rounded-lg px-4 py-3 text-sm text-white placeholder-gray-500 focus:border-green-500/50" />
                <button @click="addProvider"
                  class="w-full py-3 bg-green-600 hover:bg-green-500 rounded-lg text-sm font-medium transition-colors">
                  Save Provider
                </button>
              </div>
            </section>
          </div>
        </div>

        <!-- ═══ OTHER VIEWS (placeholder) ═══ -->
        <div v-else class="h-full flex items-center justify-center">
          <div class="text-center">
            <div class="text-4xl mb-4 text-gray-700">{{ navItems.find(n => n.id === currentView)?.icon }}</div>
            <div class="text-lg text-gray-400">{{ navItems.find(n => n.id === currentView)?.label }}</div>
            <div class="text-sm text-gray-600 mt-2">Coming soon</div>
          </div>
        </div>

      </main>
    </div>
  </div>
</template>