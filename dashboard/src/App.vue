<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'

// ─── State ────────────────────────────────────────────────────

const connected = ref(false)
const currentView = ref('status')
const ws = ref<WebSocket | null>(null)

// Dashboard data
const status = ref({
  version: '0.1.0',
  uptime: 0,
  activeSessions: 0,
  totalTokens: 0,
  avgAsiScore: 100.0,
  contextPressure: 0.0,
})

const sessions = ref<any[]>([])
const agents = ref<any[]>([])

// ─── WebSocket ────────────────────────────────────────────────

function connectWebSocket() {
  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
  ws.value = new WebSocket(`${protocol}//${window.location.host}/ws/global`)

  ws.value.onopen = () => { connected.value = true }
  ws.value.onclose = () => {
    connected.value = false
    setTimeout(connectWebSocket, 3000)
  }
  ws.value.onmessage = (event) => {
    try {
      const data = JSON.parse(event.data)
      if (data.type === 'status') {
        Object.assign(status.value, data.payload)
      }
    } catch {}
  }
}

onMounted(() => { connectWebSocket() })
onMounted(() => {
  // Simulated data for demo
  status.value = {
    version: '0.1.0',
    uptime: 3847,
    activeSessions: 2,
    totalTokens: 153924,
    avgAsiScore: 87.3,
    contextPressure: 0.42,
  }
  agents.value = [
    { id: 'architect', model: 'claude-4-opus', status: 'idle', asi: 95, pressure: 0.2, tokens: 0 },
    { id: 'coder', model: 'gpt-5', status: 'running', asi: 88, pressure: 0.65, tokens: 45200 },
    { id: 'reviewer', model: 'gemini-2.5-pro', status: 'idle', asi: 92, pressure: 0.3, tokens: 12800 },
    { id: 'security', model: 'claude-4-haiku', status: 'idle', asi: 98, pressure: 0.15, tokens: 0 },
    { id: 'tester', model: 'gpt-5', status: 'idle', asi: 90, pressure: 0.25, tokens: 8400 },
  ]
  sessions.value = [
    { id: 's1', name: 'API Development', goal: 'Create REST API', phase: 'Implementing', progress: 65 },
    { id: 's2', name: 'Auth Module', goal: 'JWT authentication', phase: 'Designing', progress: 30 },
  ]
})

onUnmounted(() => { ws.value?.close() })

// ─── Computed ─────────────────────────────────────────────────

const sidebarItems = computed(() => [
  { id: 'status', label: 'Status', icon: '◉' },
  { id: 'usage', label: 'Usage', icon: '◎' },
  { id: 'sessions', label: 'Sessions', icon: '▦' },
  { id: 'agents', label: 'Agents', icon: '⬡' },
  { id: 'context', label: 'Context', icon: '⊞' },
  { id: 'memory', label: 'Memory', icon: '⬢' },
  { id: 'logs', label: 'Logs', icon: '▤' },
  { id: 'config', label: 'Config', icon: '⚙' },
])

function formatUptime(seconds: number) {
  const h = Math.floor(seconds / 3600)
  const m = Math.floor((seconds % 3600) / 60)
  return `${h}h ${m}m`
}

function formatTokens(n: number) {
  if (n >= 1000000) return `${(n / 1000000).toFixed(1)}M`
  if (n >= 1000) return `${(n / 1000).toFixed(1)}K`
  return n.toString()
}

function pressureColor(p: number) {
  if (p > 0.9) return 'text-red-400'
  if (p > 0.7) return 'text-amber-400'
  if (p > 0.5) return 'text-yellow-400'
  return 'text-green-400'
}

function statusColor(s: string) {
  if (s === 'running') return 'bg-emerald-500'
  if (s === 'idle') return 'bg-zinc-500'
  if (s === 'error') return 'bg-red-500'
  return 'bg-zinc-600'
}
</script>

<template>
  <div class="min-h-screen bg-zinc-950 text-zinc-100 font-mono flex">

    <!-- Sidebar -->
    <aside class="w-56 bg-zinc-900 border-r border-zinc-800 flex flex-col shrink-0">
      <div class="p-4 border-b border-zinc-800">
        <div class="flex items-center gap-2">
          <div class="w-3 h-3 rounded-full bg-emerald-500"></div>
          <span class="text-sm font-bold tracking-wide">PROJECT-X</span>
          <span class="text-xs text-zinc-500 ml-auto">v{{ status.version }}</span>
        </div>
      </div>

      <nav class="flex-1 p-2 space-y-0.5">
        <button v-for="item in sidebarItems" :key="item.id"
          @click="currentView = item.id"
          class="w-full flex items-center gap-3 px-3 py-2 text-sm rounded-md transition-colors"
          :class="currentView === item.id ? 'bg-zinc-800 text-white' : 'text-zinc-400 hover:bg-zinc-800/50 hover:text-zinc-200'">
          <span class="text-base">{{ item.icon }}</span>
          <span>{{ item.label }}</span>
        </button>
      </nav>

      <div class="p-4 border-t border-zinc-800 space-y-2">
        <div class="text-xs text-zinc-500">PROJECTS</div>
        <div v-for="s in sessions" :key="s.id" class="flex items-center gap-2 text-sm text-zinc-400 hover:text-zinc-200 cursor-pointer">
          <span class="w-1.5 h-1.5 rounded-full bg-emerald-500"></span>
          <span class="truncate">{{ s.name }}</span>
        </div>
      </div>
    </aside>

    <!-- Main Content -->
    <main class="flex-1 overflow-auto">

      <!-- Top Bar -->
      <header class="sticky top-0 z-10 bg-zinc-950/80 backdrop-blur border-b border-zinc-800 px-6 py-3 flex items-center justify-between">
        <div class="flex items-center gap-4">
          <h1 class="text-lg font-bold">{{ currentView.toUpperCase() }}</h1>
          <span class="text-xs px-2 py-0.5 rounded-full"
            :class="connected ? 'bg-emerald-500/20 text-emerald-400' : 'bg-red-500/20 text-red-400'">
            {{ connected ? '● CONNECTED' : '● DISCONNECTED' }}
          </span>
        </div>
        <div class="flex items-center gap-3 text-xs text-zinc-500">
          <span>UPTIME {{ formatUptime(status.uptime) }}</span>
          <span>SESSIONS {{ status.activeSessions }}</span>
        </div>
      </header>

      <!-- Status View -->
      <div v-if="currentView === 'status'" class="p-6 space-y-6">

        <!-- Active Sessions -->
        <section>
          <h2 class="text-xs font-semibold text-zinc-500 tracking-widest mb-3">ACTIVE SESSIONS</h2>
          <div class="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-3">
            <div v-for="s in sessions" :key="s.id"
              class="bg-zinc-900 border border-zinc-800 rounded-lg p-4 hover:border-zinc-700 transition-colors cursor-pointer">
              <div class="flex items-center justify-between mb-2">
                <span class="text-sm font-medium">{{ s.name }}</span>
                <span class="text-xs px-2 py-0.5 rounded bg-emerald-500/20 text-emerald-400">{{ s.phase }}</span>
              </div>
              <p class="text-xs text-zinc-400 mb-3">{{ s.goal }}</p>
              <div class="w-full bg-zinc-800 rounded-full h-1.5">
                <div class="bg-emerald-500 h-1.5 rounded-full transition-all" :style="{ width: s.progress + '%' }"></div>
              </div>
              <div class="text-xs text-zinc-500 mt-1 text-right">{{ s.progress }}%</div>
            </div>
          </div>
        </section>

        <!-- Key Metrics -->
        <section>
          <h2 class="text-xs font-semibold text-zinc-500 tracking-widest mb-3">METRICS</h2>
          <div class="grid grid-cols-2 md:grid-cols-4 gap-3">
            <div class="bg-zinc-900 border border-zinc-800 rounded-lg p-4">
              <div class="text-xs text-zinc-500 mb-1">TOTAL TOKENS</div>
              <div class="text-2xl font-bold">{{ formatTokens(status.totalTokens) }}</div>
            </div>
            <div class="bg-zinc-900 border border-zinc-800 rounded-lg p-4">
              <div class="text-xs text-zinc-500 mb-1">ASI SCORE</div>
              <div class="text-2xl font-bold" :class="status.avgAsiScore > 80 ? 'text-emerald-400' : status.avgAsiScore > 60 ? 'text-amber-400' : 'text-red-400'">
                {{ status.avgAsiScore.toFixed(1) }}
              </div>
            </div>
            <div class="bg-zinc-900 border border-zinc-800 rounded-lg p-4">
              <div class="text-xs text-zinc-500 mb-1">CONTEXT PRESSURE</div>
              <div class="text-2xl font-bold" :class="pressureColor(status.contextPressure)">
                {{ (status.contextPressure * 100).toFixed(0) }}%
              </div>
            </div>
            <div class="bg-zinc-900 border border-zinc-800 rounded-lg p-4">
              <div class="text-xs text-zinc-500 mb-1">ACTIVE SESSIONS</div>
              <div class="text-2xl font-bold">{{ status.activeSessions }}</div>
            </div>
          </div>
        </section>

        <!-- Agents -->
        <section>
          <h2 class="text-xs font-semibold text-zinc-500 tracking-widest mb-3">AGENTS</h2>
          <div class="bg-zinc-900 border border-zinc-800 rounded-lg overflow-hidden">
            <table class="w-full text-sm">
              <thead>
                <tr class="text-xs text-zinc-500 border-b border-zinc-800">
                  <th class="text-left px-4 py-2 font-medium">AGENT</th>
                  <th class="text-left px-4 py-2 font-medium">MODEL</th>
                  <th class="text-left px-4 py-2 font-medium">STATUS</th>
                  <th class="text-left px-4 py-2 font-medium">ASI</th>
                  <th class="text-left px-4 py-2 font-medium">PRESSURE</th>
                  <th class="text-left px-4 py-2 font-medium">TOKENS</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="a in agents" :key="a.id" class="border-t border-zinc-800/50 hover:bg-zinc-800/30">
                  <td class="px-4 py-2.5 font-medium">{{ a.id }}</td>
                  <td class="px-4 py-2.5 text-zinc-400">{{ a.model }}</td>
                  <td class="px-4 py-2.5">
                    <span class="flex items-center gap-2">
                      <span class="w-2 h-2 rounded-full" :class="statusColor(a.status)"></span>
                      <span class="capitalize text-zinc-300">{{ a.status }}</span>
                    </span>
                  </td>
                  <td class="px-4 py-2.5" :class="a.asi > 80 ? 'text-emerald-400' : a.asi > 60 ? 'text-amber-400' : 'text-red-400'">
                    {{ a.asi.toFixed(1) }}
                  </td>
                  <td class="px-4 py-2.5" :class="pressureColor(a.pressure)">
                    {{ (a.pressure * 100).toFixed(0) }}%
                  </td>
                  <td class="px-4 py-2.5 text-zinc-400">{{ formatTokens(a.tokens) }}</td>
                </tr>
              </tbody>
            </table>
          </div>
        </section>

        <!-- Logs -->
        <section>
          <h2 class="text-xs font-semibold text-zinc-500 tracking-widest mb-3">RECENT EVENTS</h2>
          <div class="bg-zinc-900 border border-zinc-800 rounded-lg p-4 font-mono text-xs text-zinc-400 space-y-1 max-h-48 overflow-auto">
            <div class="text-zinc-600">[14:32:01] SystemEvent::PhaseChanged: Idle → Planning</div>
            <div class="text-zinc-600">[14:32:02] TokenUsed: openai gpt-5 input=1240 output=580</div>
            <div class="text-zinc-600">[14:32:05] AgentEvent::Coder: writing src/main.rs</div>
            <div class="text-zinc-600">[14:32:08] ToolCalled: filesystem write_file success</div>
            <div class="text-emerald-500/70">[14:32:10] GateResult: review.pass PASSED</div>
            <div class="text-zinc-600">[14:32:12] PhaseChanged: Planning → Implementing</div>
            <div class="text-zinc-600">[14:32:15] TokenUsed: openai gpt-5 input=2100 output=1200</div>
            <div class="text-zinc-600">[14:32:18] ToolCalled: filesystem write_file success</div>
          </div>
        </section>
      </div>

      <!-- Placeholder views -->
      <div v-else class="p-6">
        <div class="bg-zinc-900 border border-zinc-800 rounded-lg p-8 text-center text-zinc-500">
          <div class="text-4xl mb-4">⬡</div>
          <div class="text-lg font-medium mb-2">{{ currentView.toUpperCase() }}</div>
          <div class="text-sm">This view is under development</div>
        </div>
      </div>
    </main>
  </div>
</template>