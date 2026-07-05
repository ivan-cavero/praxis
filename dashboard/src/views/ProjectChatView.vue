<script setup lang="ts">
/**
 * ProjectChatView — Unified multi-agent chat.
 *
 * One goal → orchestrator dispatches to agents → unified message stream.
 * Options dialog (budget, skills, worktree) configurable before sending.
 */
import { ref, computed, inject, onMounted, watch, nextTick } from 'vue'
import { useRoute, useRouter } from 'vue-router'
import { useAppStore } from '../stores/app'
import { useApi, type Project, type SkillInfo } from '../composables/useApi'
import { useWebSocket, getEventPayload, type AgentOutputEvent } from '../composables/useWebSocket'
import { useToast } from '../composables/useToast'
import Icon from '../components/ui/Icon.vue'

const openSettings = inject<() => void>('openSettings')

const route = useRoute()
const router = useRouter()
const store = useAppStore()
const api = useApi()
const ws = useWebSocket()
const toast = useToast()

// ─── Project ───────────────────────────────────────────────────────

const project = ref<Project | null>(null)
const isLoading = ref(true)

const projectId = computed(() => route.params.id as string)

// ─── Tabs (removed — single unified view) ──────────────────────
// The chat is now a single view. No agent tabs. One goal → orchestrator → agents.

// ─── Chat messages (unified, no per-agent split) ────────────────

interface ChatMessage {
  id: string
  role: 'user' | 'assistant' | 'system'
  content: string
  timestamp: string
  agent?: string
}

const messages = ref<ChatMessage[]>([])

/** All messages, sorted by timestamp. */
const currentMessages = computed(() =>
  [...messages.value].sort((a, b) => a.timestamp.localeCompare(b.timestamp))
)

const inputText = ref('')
const isSending = ref(false)
const inputTextarea = ref<HTMLTextAreaElement | null>(null)
const showOptions = ref(false)

/** Auto-resize the textarea to fit content (up to a max height). */
function autoResize(): void {
  const el = inputTextarea.value
  if (!el) return
  el.style.height = 'auto'
  el.style.height = Math.min(el.scrollHeight, 200) + 'px'
}

/** Insert a newline at the cursor (for Shift+Enter). */
function handleNewline(): void {
  // Default behavior inserts a newline — just trigger auto-resize
  nextTick(() => autoResize())
}

/** Push a message to the unified message list. */
function pushMessage(_agent: string, msg: ChatMessage) {
  messages.value = [...messages.value, msg]
}

// ─── Git branch (removed — not needed in chat) ──────────────────

onMounted(async () => {
  isLoading.value = true
  try {
    if (projectId.value) {
      project.value = await api.getProject(projectId.value)
      store.selectProject(projectId.value)
    }
  } catch {
    toast.error('Failed to load project')
  }
  isLoading.value = false
})

// ─── Send a goal to the orchestrator ────────────────────────────────
//
// The orchestrator receives the goal and distributes work across agents
// through phases (plan → implement → review → test → consolidate).
// You never send to individual agents — one goal, full pipeline.

async function sendMessage() {
  if (!inputText.value.trim() || !projectId.value) return
  const text = inputText.value.trim()
  inputText.value = ''

  // Support multi-goal dispatch: split by newlines that start with "- " or "1. "
  // or just separate goals with "---" on its own line
  const goals = parseMultiGoals(text)

  for (const goal of goals) {
    // Push the user's goal to the "all" tab
    pushMessage('all', {
      id: crypto.randomUUID(),
      role: 'user',
      content: goal,
      timestamp: new Date().toISOString(),
      agent: 'all',
    })

    isSending.value = true
    try {
      // Start a real goal run via the API
      const result = await api.runGoal(projectId.value, {
        goal,
        until: untilCommand.value || undefined,
        max_tokens: maxTokens.value || undefined,
        max_cost_usd: maxCost.value || undefined,
        skills: enabledSkills.value.length > 0 ? enabledSkills.value : undefined,
        worktree: useWorktree.value || undefined,
      })

      activeSessionId.value = result.session_id

      pushMessage('all', {
        id: crypto.randomUUID(),
        role: 'system',
        content: `Goal dispatched — session ${result.session_id.slice(0, 8)}... orchestrator distributing across agents`,
        timestamp: new Date().toISOString(),
      })
      toast.success(`Goal dispatched to orchestrator`)

      // Start polling for live state (tokens, cost, phase)
      startStatePolling(result.session_id)
    } catch (error: unknown) {
      const message = error instanceof Error ? error.message : 'unknown error'
      pushMessage('all', {
        id: crypto.randomUUID(),
        role: 'system',
        content: `Failed to dispatch goal: ${message}`,
        timestamp: new Date().toISOString(),
      })
      toast.error(`Failed to dispatch goal: ${message}`)
    }
    isSending.value = false
  }
}

/** Parse multi-goal input. Supports:
 * - Single goal (one line or paragraph)
 * - Multiple goals separated by "---" on its own line
 * - Numbered list (1. goal1 \n 2. goal2)
 * - Bullet list (- goal1 \n - goal2)
 */
function parseMultiGoals(text: string): string[] {
  const trimmed = text.trim()

  // Check for separator
  if (trimmed.includes('\n---\n')) {
    return trimmed.split('\n---\n').map(g => g.trim()).filter(g => g.length > 0)
  }

  // Check for numbered list (1. ... 2. ...)
  const numberedMatch = trimmed.match(/^(\d+\.\s+.+(\n|$))+/)
  if (numberedMatch) {
    return trimmed
      .split(/\n(?=\d+\.\s)/)
      .map(g => g.replace(/^\d+\.\s+/, '').trim())
      .filter(g => g.length > 0)
  }

  // Check for bullet list (- ... - ...)
  if (trimmed.match(/^(-\s+.+(\n|$))+/)) {
    return trimmed
      .split(/\n(?=-\s)/)
      .map(g => g.replace(/^-\s+/, '').trim())
      .filter(g => g.length > 0)
  }

  // Single goal
  return [trimmed]
}

// ─── Plan mode ────────────────────────────────────────────────────

const showPlanModal = ref(false)
const planContent = ref('')
const planPath = ref('')
const isPlanning = ref(false)

async function planGoal() {
  if (!inputText.value.trim() || !projectId.value) return
  const text = inputText.value.trim()

  isPlanning.value = true
  try {
    const result = await api.planGoal(projectId.value, { goal: text })
    planContent.value = result.plan
    planPath.value = result.plan_path
    showPlanModal.value = true
    toast.success('Plan generated — review and execute when ready')
  } catch (error: unknown) {
    const message = error instanceof Error ? error.message : 'unknown error'
    pushMessage('all', {
      id: crypto.randomUUID(),
      role: 'system',
      content: `Failed to plan: ${message}`,
      timestamp: new Date().toISOString(),
    })
    toast.error(`Failed to plan: ${message}`)
  }
  isPlanning.value = false
}

async function executePlan() {
  if (!projectId.value || !planContent.value) return
  showPlanModal.value = false

  // Extract the goal from the plan content (first "# Plan: ..." line)
  const goalLine = planContent.value.split('\n').find(l => l.startsWith('# Plan:'))
  const goal = goalLine ? goalLine.replace('# Plan:', '').trim() : inputText.value

  inputText.value = goal
  await sendMessage()
}

// ─── Live session state polling ────────────────────────────────────

const activeSessionId = ref<string | null>(null)
const liveTokens = ref(0)
const liveCost = ref(0)
const livePhase = ref('')
const liveIteration = ref(0)
const liveStatus = ref('')
const untilCommand = ref('')
const maxTokens = ref<number | null>(null)
const maxCost = ref<number | null>(null)
const useWorktree = ref(false)
const availableSkills = ref<SkillInfo[]>([])
const enabledSkills = ref<string[]>([])

// Load available skills on mount
onMounted(async () => {
  try {
    availableSkills.value = await api.getSkills()
  } catch {
    // skills endpoint might not be available
  }
})

function toggleSkill(skillId: string) {
  enabledSkills.value = enabledSkills.value.includes(skillId)
    ? enabledSkills.value.filter(id => id !== skillId)
    : [...enabledSkills.value, skillId]
}

let statePollInterval: ReturnType<typeof setInterval> | null = null

function startStatePolling(sessionId: string) {
  if (statePollInterval) clearInterval(statePollInterval)
  statePollInterval = setInterval(async () => {
    try {
      const state = await api.getSessionState(sessionId)
      liveTokens.value = state.tokens_used
      liveCost.value = state.cost_usd
      livePhase.value = state.phase
      liveIteration.value = state.iteration
      liveStatus.value = state.status

      // Stop polling when the session is no longer running
      if (state.status !== 'running') {
        if (statePollInterval) {
          clearInterval(statePollInterval)
          statePollInterval = null
        }
        pushMessage('all', {
          id: crypto.randomUUID(),
          role: 'system',
          content: `Session ${state.status} — ${state.tokens_used} tokens, $${state.cost_usd.toFixed(4)}`,
          timestamp: new Date().toISOString(),
        })
      }
    } catch {
      // Session might not be found yet — keep polling
    }
  }, 2000)
}

// ─── Listen for agent output via WebSocket ─────────────────────────

watch(() => ws.events.value, (allEvents) => {
  for (const event of allEvents) {
    const agentOut = getEventPayload<AgentOutputEvent>(event, 'AgentOutput')
    if (agentOut && agentOut.delta && agentOut.agent) {
      messages.value = [...messages.value, {
        id: crypto.randomUUID(),
        role: 'assistant',
        content: agentOut.delta,
        timestamp: event.timestamp,
        agent: agentOut.agent,
      }]
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

      <!-- Actions -->
      <div class="chat-header-actions">
        <button
          class="chat-options-btn"
          @click="showOptions = true"
          title="Goal options (budget, skills, worktree)"
        >
          <Icon name="settings" :size="15" />
          <span class="chat-options-label">Options</span>
        </button>
        <button
          v-if="openSettings"
          class="chat-settings-btn"
          @click="openSettings()"
          title="Project settings (agents, providers, limits)"
        >
          <Icon name="robot" :size="15" />
        </button>
      </div>
    </div>

    <!-- Orchestrator hint -->
    <div class="chat-orch-hint">
      <Icon name="brain" :size="12" />
      One goal → orchestrator → agents → done. Use Shift+Enter for multi-line, or --- for multi-goal.
    </div>

    <!-- Messages -->
    <div class="chat-messages">
      <div v-if="currentMessages.length === 0" class="chat-empty">
        <Icon name="message" :size="48" class="empty-icon" />
        <p>No messages yet</p>
        <p class="empty-hint">Send a goal and the orchestrator will distribute it across agents</p>
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

    <!-- Live state bar (shown when a session is active) -->
    <div v-if="activeSessionId" class="live-state-bar">
      <div class="live-state-item">
        <span class="live-state-label">Phase</span>
        <span class="live-state-value">{{ livePhase }}</span>
      </div>
      <div class="live-state-item">
        <span class="live-state-label">Iter</span>
        <span class="live-state-value">{{ liveIteration }}</span>
      </div>
      <div class="live-state-item">
        <span class="live-state-label">Tokens</span>
        <span class="live-state-value">{{ liveTokens.toLocaleString() }}</span>
      </div>
      <div class="live-state-item">
        <span class="live-state-label">Cost</span>
        <span class="live-state-value">${{ liveCost.toFixed(4) }}</span>
      </div>
      <div class="live-state-item">
        <span class="live-state-label">Status</span>
        <span class="live-state-value" :class="{ 'status-running': liveStatus === 'running' }">{{ liveStatus }}</span>
      </div>
    </div>

    <!-- Input — multi-line textarea for multi-goal support -->
    <div class="chat-input">
      <textarea
        ref="inputTextarea"
        v-model="inputText"
        class="chat-input-field"
        :placeholder="'Describe what you want to build... (Enter to send, Shift+Enter for new line. Use --- for multi-goal)'"
        :disabled="isSending"
        rows="1"
        @keydown.enter.exact.prevent="sendMessage"
        @keydown.shift.enter="handleNewline"
        @input="autoResize"
      ></textarea>
      <button
        class="chat-send-btn"
        :disabled="!inputText.trim() || isSending"
        @click="sendMessage"
        title="Send goal (Enter)"
      >
        <Icon v-if="isSending" name="refresh" :size="16" class="animate-spin" />
        <Icon v-else name="send" :size="16" />
      </button>
      <button
        class="chat-plan-btn"
        :disabled="!inputText.trim() || isPlanning"
        @click="planGoal"
        title="Plan first (Planning + Designing only)"
      >
        <Icon v-if="isPlanning" name="refresh" :size="16" class="animate-spin" />
        <Icon v-else name="code" :size="16" />
      </button>
    </div>

    <!-- Options dialog (budget, skills, worktree) -->
    <div v-if="showOptions" class="options-overlay" @click.self="showOptions = false">
      <div class="options-dialog">
        <div class="options-header">
          <h2>Goal Options</h2>
          <button class="options-close" @click="showOptions = false">
            <Icon name="x" :size="18" />
          </button>
        </div>
        <div class="options-body">
          <!-- Budget -->
          <div class="options-section">
            <h3 class="options-section-title">Budget Limits</h3>
            <div class="options-row">
              <label class="options-field">
                <span class="options-label">Until command</span>
                <input v-model="untilCommand" placeholder="cargo test" class="options-input" />
              </label>
            </div>
            <div class="options-row">
              <label class="options-field">
                <span class="options-label">Max tokens</span>
                <input v-model.number="maxTokens" type="number" placeholder="1000000" class="options-input" />
              </label>
              <label class="options-field">
                <span class="options-label">Max cost ($)</span>
                <input v-model.number="maxCost" type="number" step="0.01" placeholder="5.00" class="options-input" />
              </label>
            </div>
          </div>

          <!-- Worktree -->
          <div class="options-section">
            <h3 class="options-section-title">Git Worktree</h3>
            <label class="options-checkbox-row">
              <input type="checkbox" v-model="useWorktree" class="options-checkbox" />
              <div class="options-checkbox-text">
                <span class="options-checkbox-label">Isolate in worktree</span>
                <span class="options-checkbox-desc">Creates a separate git worktree for this session</span>
              </div>
            </label>
          </div>

          <!-- Skills -->
          <div v-if="availableSkills.length > 0" class="options-section">
            <h3 class="options-section-title">Skills</h3>
            <div class="skills-grid">
              <button
                v-for="skill in availableSkills"
                :key="skill.id"
                class="skill-chip"
                :class="{ 'skill-enabled': enabledSkills.includes(skill.id) }"
                :title="skill.description"
                @click="toggleSkill(skill.id)"
              >
                {{ skill.name }}
              </button>
            </div>
          </div>
        </div>
        <div class="options-footer">
          <button class="btn btn-primary" @click="showOptions = false">Done</button>
        </div>
      </div>
    </div>

    <!-- Plan modal -->
    <div v-if="showPlanModal" class="plan-modal-overlay" @click.self="showPlanModal = false">
      <div class="plan-modal">
        <div class="plan-modal-header">
          <h2>Plan Review</h2>
          <button class="plan-close" @click="showPlanModal = false">×</button>
        </div>
        <div class="plan-modal-body">
          <pre class="plan-content">{{ planContent }}</pre>
        </div>
        <div class="plan-modal-footer">
          <span class="plan-path">{{ planPath }}</span>
          <div class="plan-actions">
            <button class="btn btn-ghost" @click="showPlanModal = false">Cancel</button>
            <button class="btn btn-primary" @click="executePlan">Execute Plan</button>
          </div>
        </div>
      </div>
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
  align-items: flex-end;
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
  resize: none;
  min-height: 38px;
  max-height: 200px;
  overflow-y: auto;
  line-height: 1.5;
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

.chat-plan-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 36px;
  height: 36px;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-full);
  background: var(--bg-elevated);
  color: var(--text-secondary);
  cursor: pointer;
  transition: all var(--transition-fast);
  flex-shrink: 0;
}
.chat-plan-btn:hover:not(:disabled) {
  background: var(--primary);
  color: var(--bg-base);
  border-color: var(--primary);
}
.chat-plan-btn:disabled { opacity: 0.4; cursor: not-allowed; }

/* ─── Plan modal ────────────────────────────────────────────────── */

.plan-modal-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.6);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 1000;
}

.plan-modal {
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  width: 90%;
  max-width: 700px;
  max-height: 80vh;
  display: flex;
  flex-direction: column;
}

.plan-modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-3) var(--space-4);
  border-bottom: 1px solid var(--border-subtle);
}

.plan-modal-header h2 {
  font-size: 16px;
  font-weight: 600;
  color: var(--text-primary);
}

.plan-close {
  background: none;
  border: none;
  color: var(--text-muted);
  font-size: 20px;
  cursor: pointer;
  padding: 0 var(--space-1);
}

.plan-modal-body {
  flex: 1;
  overflow-y: auto;
  padding: var(--space-4);
}

.plan-content {
  font-family: var(--font-mono, monospace);
  font-size: 13px;
  line-height: 1.6;
  color: var(--text-secondary);
  white-space: pre-wrap;
  word-break: break-word;
  margin: 0;
}

.plan-modal-footer {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-3) var(--space-4);
  border-top: 1px solid var(--border-subtle);
  gap: var(--space-3);
}

.plan-path {
  font-size: 11px;
  color: var(--text-disabled);
  font-family: var(--font-mono, monospace);
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
  flex: 1;
}

.plan-actions {
  display: flex;
  gap: var(--space-2);
}

.btn {
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-md);
  font-size: 13px;
  font-weight: 500;
  border: none;
  cursor: pointer;
  transition: all var(--transition-fast);
}

.btn-ghost {
  background: transparent;
  color: var(--text-muted);
  border: 1px solid var(--border-subtle);
}
.btn-ghost:hover { background: var(--bg-hover); }

.btn-primary {
  background: var(--primary);
  color: var(--bg-base);
}
.btn-primary:hover { background: var(--primary-hover); }

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

/* ─── Live state bar ────────────────────────────────────────────── */

.live-state-bar {
  display: flex;
  gap: var(--space-4);
  padding: var(--space-2) var(--space-4);
  background: var(--bg-elevated);
  border-bottom: 1px solid var(--border-subtle);
  flex-shrink: 0;
  font-size: 12px;
}

.live-state-item {
  display: flex;
  flex-direction: column;
  gap: 1px;
}

.live-state-label {
  font-size: 10px;
  color: var(--text-disabled);
  text-transform: uppercase;
  letter-spacing: 0.05em;
}

.live-state-value {
  font-family: var(--font-mono, monospace);
  color: var(--text-secondary);
  font-weight: 500;
}

.status-running {
  color: var(--success, #22c55e);
}

/* ─── Options dialog ───────────────────────────────────────────── */

.options-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.6);
  backdrop-filter: blur(4px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 200;
}

.options-dialog {
  width: 500px;
  max-width: 90vw;
  max-height: 80vh;
  background: var(--bg-surface);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-xl);
  display: flex;
  flex-direction: column;
  overflow: hidden;
  box-shadow: 0 8px 40px rgba(0, 0, 0, 0.4);
}

.options-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-4) var(--space-5);
  border-bottom: 1px solid var(--border-subtle);
}

.options-header h2 {
  font-size: 16px;
  font-weight: 600;
  color: var(--text-primary);
}

.options-close {
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

.options-close:hover {
  color: var(--text-primary);
  background: var(--bg-hover);
}

.options-body {
  flex: 1;
  overflow-y: auto;
  padding: var(--space-5);
  display: flex;
  flex-direction: column;
  gap: var(--space-5);
}

.options-section {
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.options-section-title {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-primary);
  text-transform: uppercase;
  letter-spacing: 0.04em;
}

.options-row {
  display: flex;
  gap: var(--space-3);
}

.options-field {
  flex: 1;
  display: flex;
  flex-direction: column;
  gap: 4px;
}

.options-label {
  font-size: 11px;
  font-weight: 500;
  text-transform: uppercase;
  letter-spacing: 0.04em;
  color: var(--text-muted);
}

.options-input {
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-md);
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  color: var(--text-primary);
  font-size: 13px;
  font-family: var(--font-mono, monospace);
  outline: none;
  transition: border-color var(--transition-fast);
}

.options-input:focus {
  border-color: var(--primary);
}

.options-checkbox-row {
  display: flex;
  align-items: flex-start;
  gap: var(--space-3);
  cursor: pointer;
}

.options-checkbox {
  width: 18px;
  height: 18px;
  margin-top: 2px;
  cursor: pointer;
  accent-color: var(--primary);
}

.options-checkbox-text {
  display: flex;
  flex-direction: column;
  gap: 2px;
}

.options-checkbox-label {
  font-size: 13px;
  font-weight: 500;
  color: var(--text-primary);
}

.options-checkbox-desc {
  font-size: 12px;
  color: var(--text-muted);
}

.options-footer {
  display: flex;
  justify-content: flex-end;
  padding: var(--space-3) var(--space-5);
  border-top: 1px solid var(--border-subtle);
}

/* ─── Skills ──────────────────────────────────────────────────── */

.skills-grid {
  display: flex;
  flex-wrap: wrap;
  gap: var(--space-2);
}

.skill-chip {
  padding: 6px 12px;
  border-radius: var(--radius-full);
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  color: var(--text-muted);
  font-size: 12px;
  cursor: pointer;
  transition: all var(--transition-fast);
  font-family: inherit;
}

.skill-chip:hover {
  border-color: var(--primary);
  color: var(--text-secondary);
}

.skill-chip.skill-enabled {
  background: var(--primary);
  color: var(--bg-base);
  border-color: var(--primary);
}

/* ─── Options button in header ──────────────────────────────── */

.chat-options-btn {
  display: flex;
  align-items: center;
  gap: 6px;
  padding: 6px 12px;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: var(--bg-elevated);
  color: var(--text-secondary);
  font-size: 12px;
  cursor: pointer;
  transition: all var(--transition-fast);
}

.chat-options-btn:hover {
  color: var(--text-primary);
  background: var(--bg-hover);
  border-color: var(--border-default);
}

.chat-options-label {
  font-weight: 500;
}
</style>
