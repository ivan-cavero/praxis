<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useAppStore } from './stores/app'
import { setApiPort } from './composables/useApi'
import { useTauriIpc, type DesktopSession, type RunGoalResult } from './composables/useTauriIpc'
import Icon from './components/ui/Icon.vue'
import LoginView from './views/LoginView.vue'
import SettingsView from './views/SettingsView.vue'

const store = useAppStore()
const ipc = useTauriIpc()

const isAuthenticated = ref(false)
const currentView = ref<'chat' | 'settings'>('chat')
const selectedProject = ref<string | null>(null)
const chatMessage = ref('')
const selectedModel = ref('GLM-5.2')
const isRunning = ref(false)
const sessions = ref<DesktopSession[]>([])
const messages = ref<{ role: 'user' | 'assistant'; content: string; timestamp: number }[]>([])
const runResult = ref<RunGoalResult | null>(null)

let refreshInterval: ReturnType<typeof setInterval> | null = null
let sessionsInterval: ReturnType<typeof setInterval> | null = null

// Listen for Tauri `api:ready` event (port number)
// Falls back gracefully in browser dev mode (Vite proxies /api)
async function listenTauriEvents() {
  try {
    const { listen } = await import('@tauri-apps/api/event')
    await listen<number>('api:ready', (event) => {
      setApiPort(event.payload)
    })
    await listen('core:ready', () => {
      // Core runtime ready — can start making API calls
    })
  } catch {
    // Not in Tauri — running in browser dev mode
  }
}

onMounted(async () => {
  await listenTauriEvents()

  const token = localStorage.getItem('praxis-token')
  if (token) {
    isAuthenticated.value = true
    startApp()
  }
})

onUnmounted(() => {
  if (refreshInterval) clearInterval(refreshInterval)
  if (sessionsInterval) clearInterval(sessionsInterval)
})

function handleLogin(token: string) {
  isAuthenticated.value = true
  localStorage.setItem('praxis-token', token)
  startApp()
}

function startApp() {
  store.refreshAll()
  refreshInterval = setInterval(() => store.refreshAll(), 10000)
  refreshSessions()
  sessionsInterval = setInterval(() => refreshSessions(), 5000)
}

async function refreshSessions() {
  try {
    sessions.value = await ipc.getSessions()
  } catch {
    // ignore polling errors
  }
}

async function stopSession(sessionId: string) {
  const stopped = await ipc.stopSession(sessionId)
  if (stopped) {
    messages.value = [...messages.value, {
      role: 'assistant',
      content: `Session ${sessionId.slice(0, 8)} stopped.`,
      timestamp: Date.now(),
    }]
    await refreshSessions()
  }
}

const greeting = computed(() => {
  const hour = new Date().getHours()
  if (hour < 12) return 'Morning'
  if (hour < 18) return 'Afternoon'
  return 'Evening'
})

function selectProject(id: string) {
  selectedProject.value = id
  store.selectProject(id)
}

async function handleSendMessage() {
  const message = chatMessage.value.trim()
  if (!message) return

  // Add user message to local history
  messages.value = [...messages.value, {
    role: 'user',
    content: message,
    timestamp: Date.now(),
  }]
  chatMessage.value = ''
  isRunning.value = true
  runResult.value = null

  try {
    const project = store.activeProject?.name || 'default'
    const result = await ipc.runGoal(project, message, selectedModel.value)
    runResult.value = result

    // Add assistant response
    messages.value = [...messages.value, {
      role: 'assistant',
      content: result.outcome === 'Achieved'
        ? `Goal completed in ${result.iterations} iterations.\n${result.message}`
        : `Goal ${result.outcome.toLowerCase()} after ${result.iterations} iterations.\n${result.message}`,
      timestamp: Date.now(),
    }]

    // Refresh sessions list
    await refreshSessions()
  } catch (caughtError: any) {
    messages.value = [...messages.value, {
      role: 'assistant',
      content: `Error: ${caughtError.message || 'Failed to run goal'}`,
      timestamp: Date.now(),
    }]
  } finally {
    isRunning.value = false
  }
}
</script>

<template>
  <LoginView v-if="!isAuthenticated" @login="handleLogin" />

  <div v-else class="layout">
    <!-- ═══ SIDEBAR ═══ -->
    <aside class="sidebar">
      <!-- Logo -->
      <div class="sidebar-header">
        <div class="logo-mark">P</div>
        <div class="logo-text">
          <div class="brand">praxis</div>
        </div>
      </div>

      <!-- Quick actions -->
      <div class="sidebar-nav">
        <button class="nav-item" @click="currentView = 'chat'">
          <Icon name="plus" :size="18" class="nav-icon" />
          <span class="nav-label">New task</span>
          <span class="nav-shortcut">Ctrl+N</span>
        </button>
        <button class="nav-item">
          <Icon name="search" :size="18" class="nav-icon" />
          <span class="nav-label">Search</span>
          <span class="nav-shortcut">Ctrl+K</span>
        </button>
        <button class="nav-item">
          <Icon name="code" :size="18" class="nav-icon" />
          <span class="nav-label">Skills</span>
        </button>
      </div>

      <!-- Tabs: Group / Project -->
      <div class="sidebar-tabs">
        <button class="sidebar-tab active">
          <Icon name="list" :size="14" />
          Group
        </button>
        <button class="sidebar-tab">
          <Icon name="folder" :size="14" />
          Project
        </button>
      </div>

      <!-- Project list -->
      <div class="sidebar-projects">
        <div v-if="store.projects.length === 0" class="project-hint">
          No projects yet
        </div>
        <template v-for="project in store.projects" :key="project.id">
          <div
            class="project-item"
            :class="{ active: selectedProject === project.id }"
            @click="selectProject(project.id)"
          >
            <Icon name="folder" :size="16" class="project-icon" />
            <span class="project-name">{{ project.name }}</span>
          </div>
          <div class="project-hint" v-if="selectedProject === project.id">
            No tasks yet
          </div>
        </template>
      </div>

      <!-- Settings nav -->
      <div class="sidebar-nav" style="border-top: 1px solid var(--border-subtle); padding-top: var(--space-2);">
        <button
          class="nav-item"
          :class="{ active: currentView === 'settings' }"
          @click="currentView = 'settings'"
        >
          <Icon name="settings" :size="18" class="nav-icon" />
          <span class="nav-label">Settings</span>
        </button>
      </div>

      <!-- Footer: User -->
      <div class="sidebar-footer">
        <div class="sidebar-user">
          <div class="user-avatar">I</div>
          <span class="user-name">Ivan</span>
          <div class="sidebar-footer-actions">
            <button class="sidebar-footer-btn" title="Remote control">
              <Icon name="phone" :size="16" />
            </button>
            <button class="sidebar-footer-btn" @click="currentView = 'settings'" title="Settings">
              <Icon name="settings" :size="16" />
            </button>
          </div>
        </div>
      </div>
    </aside>

    <!-- ═══ MAIN CONTENT ═══ -->
    <div class="main-content">
      <!-- ═══ CHAT VIEW ═══ -->
      <template v-if="currentView === 'chat'">

        <!-- Sessions indicator -->
        <div v-if="sessions.length > 0" class="sessions-bar">
          <Icon name="activity" :size="14" />
          <span>{{ sessions.length }} session{{ sessions.length !== 1 ? 's' : '' }}</span>
          <div class="session-badges">
            <span
              v-for="s in sessions"
              :key="s.session_id"
              class="session-badge"
              :class="s.status"
              @click="stopSession(s.session_id)"
              :title="'Click to stop: ' + s.session_id"
            >
              {{ s.project }}:{{ s.phase }}
              <Icon v-if="s.status === 'running'" name="x" :size="10" class="session-stop-icon" />
            </span>
          </div>
        </div>

        <!-- Message history -->
        <div v-if="messages.length > 0" class="messages-area">
          <div
            v-for="(msg, index) in messages"
            :key="index"
            class="message-bubble"
            :class="msg.role"
          >
            <div class="message-avatar">{{ msg.role === 'user' ? 'U' : 'P' }}</div>
            <div class="message-content">{{ msg.content }}</div>
          </div>
        </div>

        <!-- Greeting area (shown when no messages yet) -->
        <div v-else class="main-greeting">
          <!-- Logo placeholder -->
          <svg class="greeting-logo" viewBox="0 0 120 120" fill="none" xmlns="http://www.w3.org/2000/svg">
            <rect x="20" y="40" width="80" height="50" rx="8" stroke="currentColor" stroke-width="2" fill="none"/>
            <path d="M40 40 L60 20 L80 40" stroke="currentColor" stroke-width="2" fill="none"/>
            <circle cx="60" cy="65" r="8" stroke="currentColor" stroke-width="2" fill="none"/>
          </svg>

          <!-- Greeting text -->
          <h1 class="greeting-text">Good {{ greeting }}, how can I help?</h1>
        </div>

        <!-- Chat input -->
        <div class="chat-input-container">
          <div class="chat-input-wrapper">
            <!-- Header: Project & Branch -->
            <div class="chat-input-header">
              <div class="chat-input-header-item">
                <Icon name="folder" :size="14" />
                <span>{{ store.activeProject?.name || 'No project' }}</span>
                <Icon name="chevron-right" :size="12" />
              </div>
              <div class="chat-input-header-item">
                <Icon name="code" :size="14" />
                <span>main</span>
                <Icon name="chevron-right" :size="12" />
              </div>
            </div>

            <!-- Textarea -->
            <textarea
              v-model="chatMessage"
              class="chat-input-textarea"
              placeholder="Ask praxis anything, @ for files, folders, or whiteboards, / for commands or agents, $ for skills, # for related conversations"
              @keydown.enter.exact="handleSendMessage"
              rows="2"
            />

            <!-- Footer: Actions & Model -->
            <div class="chat-input-footer">
              <div class="chat-input-actions">
                <button class="chat-input-action">
                  <Icon name="plus" :size="16" />
                </button>
                <button class="chat-input-action primary">
                  <Icon name="shield" :size="14" />
                  <span>Full access</span>
                  <Icon name="chevron-right" :size="12" />
                </button>
              </div>

              <div class="chat-input-model">
                <button class="chat-input-action">
                  <Icon name="circle" :size="14" />
                </button>
                <button class="chat-input-action">
                  <span>{{ selectedModel }}</span>
                  <Icon name="chevron-right" :size="12" />
                </button>
                <button class="chat-input-action">
                  <Icon name="settings" :size="14" />
                  <span>Max</span>
                  <Icon name="chevron-right" :size="12" />
                </button>
                <button
                  class="btn-icon-send"
                  :disabled="!chatMessage.trim() || isRunning"
                  @click="handleSendMessage"
                >
                  <span v-if="isRunning" class="loading-spinner" />
                  <Icon v-else name="send" :size="16" />
                </button>
              </div>
            </div>
          </div>
        </div>
      </template>

      <!-- ═══ SETTINGS VIEW ═══ -->
      <template v-else-if="currentView === 'settings'">
        <SettingsView @back="currentView = 'chat'" />
      </template>
    </div>
  </div>
</template>

<style scoped>
/* Settings view transitions */
.fade-enter-active,
.fade-leave-active {
  transition: opacity 0.2s ease;
}

.fade-enter-from,
.fade-leave-to {
  opacity: 0;
}

/* Sessions bar */
.sessions-bar {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.5rem 1rem;
  font-size: 0.75rem;
  color: var(--text-secondary);
  border-bottom: 1px solid var(--border-subtle);
}

.session-badge {
  display: inline-block;
  padding: 0.125rem 0.5rem;
  border-radius: 9999px;
  font-size: 0.675rem;
  background: var(--surface-raised);
}

.session-badge.running { color: var(--accent); cursor: pointer; }
.session-badge.running:hover { background: var(--surface-raised); }
.session-badge.completed { color: var(--text-success, #4ade80); }
.session-badge.failed { color: var(--text-danger, #f87171); }

.session-badges {
  display: flex;
  align-items: center;
  gap: 0.25rem;
  flex-wrap: wrap;
}

.session-stop-icon {
  margin-left: 0.25rem;
  opacity: 0.6;
}

.session-badge:hover .session-stop-icon {
  opacity: 1;
}

/* Messages area */
.messages-area {
  flex: 1;
  overflow-y: auto;
  padding: 1rem;
  display: flex;
  flex-direction: column;
  gap: 1rem;
}

.message-bubble {
  display: flex;
  gap: 0.75rem;
  max-width: 80%;
}

.message-bubble.user {
  align-self: flex-end;
  flex-direction: row-reverse;
}

.message-bubble.assistant {
  align-self: flex-start;
}

.message-avatar {
  width: 2rem;
  height: 2rem;
  border-radius: 50%;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 0.75rem;
  font-weight: 600;
  flex-shrink: 0;
}

.message-bubble.user .message-avatar {
  background: var(--accent);
  color: white;
}

.message-bubble.assistant .message-avatar {
  background: var(--surface-raised);
  color: var(--text-primary);
}

.message-content {
  padding: 0.5rem 0.75rem;
  border-radius: 0.5rem;
  font-size: 0.875rem;
  line-height: 1.5;
  white-space: pre-wrap;
}

.message-bubble.user .message-content {
  background: var(--accent);
  color: white;
}

.message-bubble.assistant .message-content {
  background: var(--surface-raised);
  color: var(--text-primary);
}
</style>
