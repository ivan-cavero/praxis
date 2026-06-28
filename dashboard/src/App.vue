<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'

const connected = ref(false)
const currentView = ref('status')
const ws = ref<WebSocket | null>(null)

const status = ref({
  version: '0.1.0',
  uptime: 0,
  activeSessions: 0,
  totalTokens: 0,
  avgAsiScore: 100.0,
  contextPressure: 0.0,
})

const sessions = ref<Array<{
  id: string; name: string; goal: string; phase: string; progress: number
}>>([])

const agents = ref<Array<{
  id: string; model: string; status: string; asi: number; pressure: number; tokens: number
}>>([])

const logs = ref<Array<{ time: string; message: string; type: string }>>([])

// Simulated data
onMounted(() => {
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
    { id: 's1', name: 'API Development', goal: 'Create REST API with auth', phase: 'Implementing', progress: 65 },
    { id: 's2', name: 'Auth Module', goal: 'JWT authentication system', phase: 'Designing', progress: 30 },
  ]
  logs.value = [
    { time: '14:32:01', message: 'PhaseChanged: Idle → Planning', type: 'system' },
    { time: '14:32:02', message: 'TokenUsed: openai gpt-5 input=1240 output=580', type: 'metrics' },
    { time: '14:32:05', message: 'Agent::Coder: writing src/main.rs', type: 'agent' },
    { time: '14:32:08', message: 'ToolCall: filesystem write_file ✓', type: 'tool' },
    { time: '14:32:10', message: 'GateResult: review.pass PASSED', type: 'gate' },
    { time: '14:32:12', message: 'PhaseChanged: Planning → Implementing', type: 'system' },
    { time: '14:32:15', message: 'TokenUsed: openai gpt-5 input=2100 output=1200', type: 'metrics' },
    { time: '14:32:18', message: 'ToolCall: filesystem write_file ✓', type: 'tool' },
  ]
})

const sidebarItems = [
  { id: 'status', label: 'Status', icon: '◉' },
  { id: 'usage', label: 'Usage', icon: '◎' },
  { id: 'sessions', label: 'Sessions', icon: '▦' },
  { id: 'agents', label: 'Agents', icon: '⬡' },
  { id: 'context', label: 'Context', icon: '⊞' },
  { id: 'memory', label: 'Memory', icon: '⬢' },
  { id: 'logs', label: 'Logs', icon: '▤' },
  { id: 'config', label: 'Config', icon: '⚙' },
]

function formatUptime(s: number) { const h = Math.floor(s/3600); const m = Math.floor((s%3600)/60); return `${h}h ${m}m` }
function formatTokens(n: number) { if (n >= 1e6) return `${(n/1e6).toFixed(1)}M`; if (n >= 1e3) return `${(n/1e3).toFixed(1)}K`; return n.toString() }
function pressureColor(p: number) { if (p > 0.9) return 'text-red-400'; if (p > 0.7) return 'text-amber-400'; if (p > 0.5) return 'text-yellow-400'; return 'text-green-400' }
function statusColor(s: string) { if (s === 'running') return 'bg-emerald-500'; if (s === 'idle') return 'bg-zinc-500'; if (s === 'error') return 'bg-red-500'; return 'bg-zinc-600' }
function logColor(t: string) { if (t === 'gate') return 'text-emerald-400'; if (t === 'system') return 'text-cyan-400'; if (t === 'agent') return 'text-blue-400'; return 'text-zinc-500' }
</script>

<template>
  <div class="h-screen bg-[#09090b] text-[#fafafa] flex overflow-hidden">

    <!-- Sidebar -->
    <aside class="w-56 bg-[#18181b] border-r border-[#27272a] flex flex-col shrink-0">
      <div class="p-4 border-b border-[#27272a]">
        <div class="flex items-center gap-2">
          <div class="w-2.5 h-2.5 rounded-full animate-pulse-dot" :class="connected ? 'bg-emerald-500' : 'bg-zinc-600'"></div>
          <span class="text-xs font-bold tracking-widest text-[#fafafa]">PROJECT-X</span>
          <span class="text-[10px] text-[#71717a] ml-auto">v{{ status.version }}</span>
        </div>
      </div>

      <nav class="flex-1 p-2 space-y-px">
        <button v-for="item in sidebarItems" :key="item.id"
          @click="currentView = item.id"
          class="w-full flex items-center gap-3 px-3 py-2 text-xs rounded transition-all duration-150"
          :class="currentView === item.id ? 'bg-[#27272a] text-[#fafafa]' : 'text-[#a1a1aa] hover:bg-[#27272a]/50 hover:text-[#d4d4d8]'">
          <span class="w-4 text-center opacity-60">{{ item.icon }}</span>
          <span>{{ item.label }}</span>
        </button>
      </nav>

      <div class="p-3 border-t border-[#27272a]">
        <div class="text-[10px] text-[#71717a] mb-2 tracking-wider">PROJECTS</div>
        <div v-for="s in sessions" :key="s.id" class="flex items-center gap-2 py-1 text-xs text-[#a1a1aa] hover:text-[#d4d4d8] cursor-pointer transition-colors">
          <span class="w-1.5 h-1.5 rounded-full bg-emerald-500"></span>
          <span class="truncate">{{ s.name }}</span>
        </div>
      </div>
    </aside>

    <!-- Main Content -->
    <main class="flex-1 overflow-y-auto">
      <!-- Top Bar -->
      <header class="sticky top-0 z-10 bg-[#09090b]/90 backdrop-blur-sm border-b border-[#27272a] px-6 py-3 flex items-center justify-between">
        <div class="flex items-center gap-4">
          <h1 class="text-sm font-semibold tracking-wide">{{ currentView.toUpperCase() }}</h1>
          <span class="text-[10px] px-2 py-0.5 rounded-full border"
            :class="connected ? 'border-emerald-500/30 text-emerald-400 bg-emerald-500/10' : 'border-red-500/30 text-red-400 bg-red-500/10'">
            {{ connected ? '● ONLINE' : '● OFFLINE' }}
          </span>
        </div>
        <div class="flex items-center gap-4 text-[10px] text-[#71717a]">
          <span>UPTIME {{ formatUptime(status.uptime) }}</span>
          <span>SESSIONS {{ status.activeSessions }}</span>
          <span>TOKENS {{ formatTokens(status.totalTokens) }}</span>
        </div>
      </header>

      <!-- Status View -->
      <div v-if="currentView === 'status'" class="p-6 space-y-6">

        <!-- Sessions -->
        <section>
          <h2 class="text-[10px] font-semibold text-[#71717a] tracking-widest mb-3">ACTIVE SESSIONS</h2>
          <div class="grid grid-cols-1 md:grid-cols-2 gap-3">
            <div v-for="s in sessions" :key="s.id"
              class="bg-[#18181b] border border-[#27272a] rounded-lg p-4 hover:border-[#3f3f46] transition-colors cursor-pointer">
              <div class="flex items-center justify-between mb-2">
                <span class="text-xs font-medium text-[#d4d4d8]">{{ s.name }}</span>
                <span class="text-[10px] px-2 py-0.5 rounded bg-emerald-500/10 text-emerald-400 border border-emerald-500/20">{{ s.phase }}</span>
              </div>
              <p class="text-[11px] text-[#a1a1aa] mb-3">{{ s.goal }}</p>
              <div class="w-full bg-[#27272a] rounded-full h-1">
                <div class="bg-emerald-500 h-1 rounded-full transition-all duration-500" :style="{ width: s.progress + '%' }"></div>
              </div>
              <div class="text-[10px] text-[#71717a] mt-1.5 text-right">{{ s.progress }}%</div>
            </div>
          </div>
        </section>

        <!-- Key Metrics -->
        <section>
          <h2 class="text-[10px] font-semibold text-[#71717a] tracking-widest mb-3">METRICS</h2>
          <div class="grid grid-cols-2 md:grid-cols-4 gap-3">
            <div class="bg-[#18181b] border border-[#27272a] rounded-lg p-4">
              <div class="text-[10px] text-[#71717a] mb-1">TOTAL TOKENS</div>
              <div class="text-xl font-bold text-[#fafafa]">{{ formatTokens(status.totalTokens) }}</div>
            </div>
            <div class="bg-[#18181b] border border-[#27272a] rounded-lg p-4">
              <div class="text-[10px] text-[#71717a] mb-1">ASI SCORE</div>
              <div class="text-xl font-bold" :class="status.avgAsiScore > 80 ? 'text-emerald-400' : status.avgAsiScore > 60 ? 'text-amber-400' : 'text-red-400'">
                {{ status.avgAsiScore.toFixed(1) }}
              </div>
            </div>
            <div class="bg-[#18181b] border border-[#27272a] rounded-lg p-4">
              <div class="text-[10px] text-[#71717a] mb-1">CONTEXT</div>
              <div class="text-xl font-bold" :class="pressureColor(status.contextPressure)">
                {{ (status.contextPressure * 100).toFixed(0) }}%
              </div>
            </div>
            <div class="bg-[#18181b] border border-[#27272a] rounded-lg p-4">
              <div class="text-[10px] text-[#71717a] mb-1">SESSIONS</div>
              <div class="text-xl font-bold text-[#fafafa]">{{ status.activeSessions }}</div>
            </div>
          </div>
        </section>

        <!-- Agents Table -->
        <section>
          <h2 class="text-[10px] font-semibold text-[#71717a] tracking-widest mb-3">AGENTS</h2>
          <div class="bg-[#18181b] border border-[#27272a] rounded-lg overflow-hidden">
            <table class="w-full text-xs">
              <thead>
                <tr class="text-[10px] text-[#71717a] border-b border-[#27272a]">
                  <th class="text-left px-4 py-2.5 font-medium">AGENT</th>
                  <th class="text-left px-4 py-2.5 font-medium">MODEL</th>
                  <th class="text-left px-4 py-2.5 font-medium">STATUS</th>
                  <th class="text-left px-4 py-2.5 font-medium">ASI</th>
                  <th class="text-left px-4 py-2.5 font-medium">PRESSURE</th>
                  <th class="text-left px-4 py-2.5 font-medium">TOKENS</th>
                </tr>
              </thead>
              <tbody>
                <tr v-for="a in agents" :key="a.id" class="border-t border-[#27272a]/50 hover:bg-[#27272a]/30 transition-colors">
                  <td class="px-4 py-2.5 font-medium text-[#d4d4d8]">{{ a.id }}</td>
                  <td class="px-4 py-2.5 text-[#a1a1aa] font-mono text-[11px]">{{ a.model }}</td>
                  <td class="px-4 py-2.5">
                    <span class="flex items-center gap-1.5">
                      <span class="w-1.5 h-1.5 rounded-full" :class="statusColor(a.status)"></span>
                      <span class="capitalize text-[#a1a1aa]">{{ a.status }}</span>
                    </span>
                  </td>
                  <td class="px-4 py-2.5" :class="a.asi > 80 ? 'text-emerald-400' : a.asi > 60 ? 'text-amber-400' : 'text-red-400'">
                    {{ a.asi.toFixed(1) }}
                  </td>
                  <td class="px-4 py-2.5" :class="pressureColor(a.pressure)">
                    {{ (a.pressure * 100).toFixed(0) }}%
                  </td>
                  <td class="px-4 py-2.5 text-[#a1a1aa] font-mono text-[11px]">{{ formatTokens(a.tokens) }}</td>
                </tr>
              </tbody>
            </table>
          </div>
        </section>

        <!-- Logs -->
        <section>
          <h2 class="text-[10px] font-semibold text-[#71717a] tracking-widest mb-3">RECENT EVENTS</h2>
          <div class="bg-[#18181b] border border-[#27272a] rounded-lg p-4 font-mono text-[11px] space-y-0.5 max-h-48 overflow-y-auto">
            <div v-for="(log, i) in logs" :key="i" class="text-[#52525b]">
              <span class="text-[#3f3f46]">[{{ log.time }}]</span>
              <span :class="logColor(log.type)">{{ log.message }}</span>
            </div>
          </div>
        </section>
      </div>

      <!-- Other views -->
      <div v-else class="p-6">
        <div class="bg-[#18181b] border border-[#27272a] rounded-lg p-12 text-center">
          <div class="text-4xl mb-3 opacity-30">⬡</div>
          <div class="text-sm font-medium text-[#a1a1aa]">{{ currentView.toUpperCase() }}</div>
          <div class="text-xs text-[#71717a] mt-1">Coming soon</div>
        </div>
      </div>
    </main>
  </div>
</template>