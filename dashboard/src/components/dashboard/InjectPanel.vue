<script setup lang="ts">
import { ref } from 'vue'
import { useApi } from '../../composables/useApi'
import Icon from '../ui/Icon.vue'

const api = useApi()

const targetAgent = ref('all')
const messageType = ref('instruction')
const content = ref('')
const isSending = ref(false)
const sendResult = ref<string | null>(null)
const showSuccess = ref(false)

const agentOptions = ['all', 'architect', 'coder', 'reviewer', 'security', 'tester', 'researcher']
const typeOptions = ['instruction', 'hint', 'context', 'correction', 'stop']

async function handleSend() {
  if (!content.value.trim()) return

  isSending.value = true
  sendResult.value = null
  showSuccess.value = false

  try {
    const result = await api.sendInject(targetAgent.value, messageType.value, content.value)
    sendResult.value = `Injected: ${result.status}`
    showSuccess.value = true
    content.value = ''
  } catch (caughtError: any) {
    sendResult.value = `Error: ${caughtError.message}`
  } finally {
    isSending.value = false
    setTimeout(() => { showSuccess.value = false }, 3000)
  }
}
</script>

<template>
  <div class="inject-panel">
    <div class="panel-header">
      <h3 class="panel-title">Mid-Loop Injection</h3>
      <p class="panel-subtitle">Send instructions directly to agents mid-execution</p>
    </div>

    <div class="inject-form">
      <div class="form-row">
        <div class="form-group">
          <label class="form-label">Target Agent</label>
          <select v-model="targetAgent" class="form-select">
            <option v-for="agent in agentOptions" :key="agent" :value="agent">
              {{ agent }}
            </option>
          </select>
        </div>

        <div class="form-group">
          <label class="form-label">Message Type</label>
          <select v-model="messageType" class="form-select">
            <option v-for="type in typeOptions" :key="type" :value="type">
              {{ type }}
            </option>
          </select>
        </div>
      </div>

      <div class="form-group">
        <label class="form-label">Message</label>
        <textarea
          v-model="content"
          class="form-textarea"
          placeholder="Type your instruction to the agent..."
          rows="3"
        />
      </div>

      <div class="form-actions">
        <button
          class="btn-inject"
          :disabled="!content.trim() || isSending"
          @click="handleSend"
        >
          <Icon v-if="isSending" name="refresh" :size="14" class="animate-spin" />
          <Icon v-else name="send" :size="14" />
          {{ isSending ? 'Sending...' : 'Inject' }}
        </button>

        <span v-if="showSuccess" class="inject-success">
          <Icon name="check" :size="14" />
          {{ sendResult }}
        </span>
        <span v-else-if="sendResult && !showSuccess" class="inject-error">
          <Icon name="alert" :size="14" />
          {{ sendResult }}
        </span>
      </div>
    </div>

    <div class="panel-info">
      <Icon name="info" :size="14" />
      <span>Injections are picked up at the next agent execution boundary. Use "stop" type to halt a session.</span>
    </div>
  </div>
</template>

<style scoped>
.inject-panel {
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  overflow: hidden;
}

.panel-header {
  padding: var(--space-4);
  border-bottom: 1px solid var(--border-subtle);
}

.panel-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
}

.panel-subtitle {
  font-size: 12px;
  color: var(--text-muted);
  margin-top: var(--space-1);
}

.inject-form {
  padding: var(--space-4);
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.form-row {
  display: grid;
  grid-template-columns: 1fr 1fr;
  gap: var(--space-3);
}

.form-group {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.form-label {
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.05em;
  text-transform: uppercase;
  color: var(--text-muted);
}

.form-select {
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-md);
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  color: var(--text-primary);
  font-size: 13px;
  font-family: inherit;
  cursor: pointer;
  transition: all var(--transition-fast);
}

.form-select:focus {
  outline: none;
  border-color: var(--primary);
}

.form-textarea {
  padding: var(--space-3);
  border-radius: var(--radius-md);
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  color: var(--text-primary);
  font-size: 13px;
  font-family: var(--font-sans);
  resize: vertical;
  min-height: 60px;
  transition: all var(--transition-fast);
  line-height: 1.5;
}

.form-textarea:focus {
  outline: none;
  border-color: var(--primary);
}

.form-textarea::placeholder {
  color: var(--text-muted);
}

.form-actions {
  display: flex;
  align-items: center;
  gap: var(--space-3);
}

.btn-inject {
  all: unset;
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-4);
  border-radius: var(--radius-md);
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  background: var(--primary);
  color: var(--bg-base);
  transition: all var(--transition-fast);
  font-family: inherit;
}

.btn-inject:hover {
  background: var(--primary-hover);
}

.btn-inject:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.inject-success {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  font-size: 12px;
  color: var(--primary);
}

.inject-error {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  font-size: 12px;
  color: var(--error);
}

.panel-info {
  display: flex;
  align-items: flex-start;
  gap: var(--space-2);
  padding: var(--space-3) var(--space-4);
  border-top: 1px solid var(--border-subtle);
  font-size: 11px;
  color: var(--text-muted);
  line-height: 1.5;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.animate-spin {
  animation: spin 0.8s linear infinite;
}
</style>
