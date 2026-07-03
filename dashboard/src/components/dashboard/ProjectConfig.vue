<script setup lang="ts">
import { ref, watch, onMounted } from 'vue'
import { useApi, type Project } from '../../composables/useApi'
import Icon from '../ui/Icon.vue'

const api = useApi()

const projects = ref<Project[]>([])
const selectedProjectId = ref<string | null>(null)
const configText = ref('')
const originalConfig = ref('')
const isSaving = ref(false)
const saveMessage = ref<string | null>(null)

async function loadProjects() {
  try {
    projects.value = await api.getProjects()
  } catch {
    // silent
  }
}

async function loadConfig() {
  if (!selectedProjectId.value) return
  try {
    const config = await api.getProjectConfig(selectedProjectId.value)
    configText.value = config.raw
    originalConfig.value = config.raw
  } catch {
    configText.value = '# Failed to load config'
  }
}

async function saveConfig() {
  if (!selectedProjectId.value) return
  isSaving.value = true
  saveMessage.value = null
  try {
    await api.updateProjectConfig(selectedProjectId.value, configText.value)
    originalConfig.value = configText.value
    saveMessage.value = 'Saved successfully'
  } catch (caughtError: any) {
    saveMessage.value = `Error: ${caughtError.message}`
  } finally {
    isSaving.value = false
    setTimeout(() => { saveMessage.value = null }, 3000)
  }
}

const hasChanges = ref(false)
watch(configText, (val) => {
  hasChanges.value = val !== originalConfig.value
})

onMounted(() => {
  loadProjects()
})
</script>

<template>
  <div class="project-config">
    <div class="config-header">
      <h3 class="config-title">Project Configuration</h3>
    </div>

    <div class="config-selector">
      <select v-model="selectedProjectId" class="project-select" @change="loadConfig">
        <option value="" disabled>Select a project...</option>
        <option v-for="p in projects" :key="p.id" :value="p.id">
          {{ p.name }}
        </option>
      </select>
    </div>

    <div v-if="selectedProjectId" class="config-editor">
      <textarea
        v-model="configText"
        class="config-textarea"
        spellcheck="false"
        :class="{ modified: hasChanges }"
      />

      <div class="config-actions">
        <button
          class="btn-save"
          :disabled="!hasChanges || isSaving"
          @click="saveConfig"
        >
          <Icon v-if="isSaving" name="refresh" :size="14" class="animate-spin" />
          <Icon v-else name="check" :size="14" />
          {{ isSaving ? 'Saving...' : 'Save' }}
        </button>

        <span v-if="saveMessage" class="save-message" :class="{ error: saveMessage.startsWith('Error') }">
          {{ saveMessage }}
        </span>
      </div>
    </div>

    <div v-else class="config-empty">
      <Icon name="settings" :size="24" class="empty-icon" />
      <p>Select a project to edit its forge.toml configuration.</p>
    </div>
  </div>
</template>

<style scoped>
.project-config {
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  overflow: hidden;
}

.config-header {
  padding: var(--space-4);
  border-bottom: 1px solid var(--border-subtle);
}

.config-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
}

.config-selector {
  padding: var(--space-4);
  border-bottom: 1px solid var(--border-subtle);
}

.project-select {
  width: 100%;
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-md);
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  color: var(--text-primary);
  font-size: 13px;
  font-family: inherit;
  cursor: pointer;
}

.project-select:focus {
  outline: none;
  border-color: var(--primary);
}

.config-editor {
  display: flex;
  flex-direction: column;
}

.config-textarea {
  width: 100%;
  min-height: 300px;
  padding: var(--space-4);
  background: var(--bg-base);
  border: none;
  border-bottom: 1px solid var(--border-subtle);
  color: var(--text-primary);
  font-family: var(--font-mono);
  font-size: 12px;
  line-height: 1.6;
  resize: vertical;
  tab-size: 2;
}

.config-textarea:focus {
  outline: none;
}

.config-textarea.modified {
  border-left: 3px solid var(--warning);
}

.config-actions {
  display: flex;
  align-items: center;
  gap: var(--space-3);
  padding: var(--space-3) var(--space-4);
}

.btn-save {
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

.btn-save:hover {
  background: var(--primary-hover);
}

.btn-save:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

.save-message {
  font-size: 12px;
  color: var(--primary);
}

.save-message.error {
  color: var(--error);
}

.config-empty {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: var(--space-12);
  color: var(--text-muted);
  gap: var(--space-3);
}

.empty-icon {
  opacity: 0.3;
}

@keyframes spin {
  to { transform: rotate(360deg); }
}

.animate-spin {
  animation: spin 0.8s linear infinite;
}
</style>
