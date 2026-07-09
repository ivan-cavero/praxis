<script setup lang="ts">
/**
 * GoalLibraryView — Reusable goal templates.
 *
 * Save frequently-used goals as templates with a name, description,
 * the goal text, and optional completion criterion. Templates are
 * stored in localStorage (client-side only, no backend needed).
 *
 * From a template, users can quickly start a new goal run or copy
 * the goal text to clipboard.
 */
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import { useApi, type Project } from '../composables/useApi'
import { useToast } from '../composables/useToast'
import Icon from '../components/ui/Icon.vue'
import EmptyState from '../components/ui/EmptyState.vue'

const router = useRouter()
const api = useApi()
const toast = useToast()

interface GoalTemplate {
  id: string
  name: string
  description: string
  goal: string
  completion: string
  created_at: string
}

const STORAGE_KEY = 'praxis:goal-templates'

const templates = ref<GoalTemplate[]>([])
const projects = ref<Project[]>([])
const showForm = ref(false)
const editingId = ref<string | null>(null)

// Form state
const formName = ref('')
const formDescription = ref('')
const formGoal = ref('')
const formCompletion = ref('coding')

function loadTemplates() {
  const stored = localStorage.getItem(STORAGE_KEY)
  if (stored) {
    try {
      templates.value = JSON.parse(stored) as GoalTemplate[]
    } catch {
      templates.value = []
    }
  }
}

function saveTemplates() {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(templates.value))
}

function loadProjects() {
  api.getProjects()
    .then(data => { projects.value = data })
    .catch(() => { /* Error loading projects */ })
}

function resetForm() {
  formName.value = ''
  formDescription.value = ''
  formGoal.value = ''
  formCompletion.value = 'coding'
  editingId.value = null
}

function openCreateForm() {
  resetForm()
  showForm.value = true
}

function openEditForm(template: GoalTemplate) {
  editingId.value = template.id
  formName.value = template.name
  formDescription.value = template.description
  formGoal.value = template.goal
  formCompletion.value = template.completion || 'coding'
  showForm.value = true
}

function saveTemplate() {
  if (!formName.value.trim() || !formGoal.value.trim()) {
    toast.error('Name and goal are required')
    return
  }

  if (editingId.value) {
    // Update existing
    templates.value = templates.value.map(t =>
      t.id === editingId.value
        ? {
            ...t,
            name: formName.value,
            description: formDescription.value,
            goal: formGoal.value,
            completion: formCompletion.value,
          }
        : t
    )
    toast.success('Template updated')
  } else {
    // Create new
    const newTemplate: GoalTemplate = {
      id: crypto.randomUUID(),
      name: formName.value,
      description: formDescription.value,
      goal: formGoal.value,
      completion: formCompletion.value,
      created_at: new Date().toISOString(),
    }
    templates.value = [...templates.value, newTemplate]
    toast.success('Template saved')
  }

  saveTemplates()
  showForm.value = false
  resetForm()
}

function deleteTemplate(id: string) {
  templates.value = templates.value.filter(t => t.id !== id)
  saveTemplates()
  toast.success('Template deleted')
}

function copyGoal(goal: string) {
  navigator.clipboard.writeText(goal)
    .then(() => toast.success('Goal copied to clipboard'))
    .catch(() => toast.error('Failed to copy'))
}

function runTemplate(template: GoalTemplate) {
  if (projects.value.length === 0) {
    toast.error('Create a project first')
    return
  }
  // Navigate to the dashboard with the goal pre-filled via query params
  router.push({ path: '/', query: { goal: template.goal, completion: template.completion } })
}

onMounted(() => {
  loadTemplates()
  loadProjects()
})
</script>

<template>
  <div class="goal-library-view">
    <div class="gl-header">
      <h1 class="gl-title">Goal Library</h1>
      <button class="btn-primary" @click="openCreateForm">
        <Icon name="plus" :size="14" />
        New Template
      </button>
    </div>

    <!-- Template form modal -->
    <div v-if="showForm" class="form-overlay" @click.self="showForm = false">
      <div class="form-modal">
        <div class="form-header">
          <h2 class="form-title">{{ editingId ? 'Edit Template' : 'New Template' }}</h2>
          <button class="form-close" @click="showForm = false" aria-label="Close form">
            <Icon name="x" :size="16" />
          </button>
        </div>
        <div class="form-body">
          <div class="form-field">
            <label class="form-label" for="tpl-name">Name</label>
            <input
              id="tpl-name"
              v-model="formName"
              class="form-input"
              placeholder="e.g., Fix clippy warnings"
            />
          </div>
          <div class="form-field">
            <label class="form-label" for="tpl-desc">Description</label>
            <input
              id="tpl-desc"
              v-model="formDescription"
              class="form-input"
              placeholder="Optional description"
            />
          </div>
          <div class="form-field">
            <label class="form-label" for="tpl-goal">Goal</label>
            <textarea
              id="tpl-goal"
              v-model="formGoal"
              class="form-textarea"
              placeholder="The goal text to send to the agent pipeline"
              rows="4"
            />
          </div>
          <div class="form-field">
            <label class="form-label" for="tpl-completion">Completion Criterion</label>
            <select id="tpl-completion" v-model="formCompletion" class="form-select">
              <option value="coding">Coding (default)</option>
              <option value="testing">Testing</option>
              <option value="review">Review</option>
              <option value="security">Security</option>
            </select>
          </div>
        </div>
        <div class="form-footer">
          <button class="btn-ghost" @click="showForm = false">Cancel</button>
          <button class="btn-primary" @click="saveTemplate">
            <Icon name="check" :size="14" />
            {{ editingId ? 'Update' : 'Save' }}
          </button>
        </div>
      </div>
    </div>

    <!-- Template grid -->
    <EmptyState
      v-if="templates.length === 0"
      icon="folder"
      title="No templates yet"
      description="Save frequently-used goals as templates for quick reuse."
      action-label="Create Template"
      :on-action="openCreateForm"
    />
    <div v-else class="template-grid">
      <div v-for="tpl in templates" :key="tpl.id" class="template-card">
        <div class="template-header">
          <h3 class="template-name">{{ tpl.name }}</h3>
          <span class="template-badge">{{ tpl.completion }}</span>
        </div>
        <p v-if="tpl.description" class="template-desc">{{ tpl.description }}</p>
        <div class="template-goal-preview">{{ tpl.goal.slice(0, 120) }}{{ tpl.goal.length > 120 ? '...' : '' }}</div>
        <div class="template-actions">
          <button class="action-btn action-run" @click="runTemplate(tpl)" aria-label="Run goal">
            <Icon name="play" :size="14" />
            Run
          </button>
          <button class="action-btn" @click="copyGoal(tpl.goal)" aria-label="Copy goal">
            <Icon name="copy" :size="14" />
          </button>
          <button class="action-btn" @click="openEditForm(tpl)" aria-label="Edit template">
            <Icon name="edit" :size="14" />
          </button>
          <button class="action-btn action-delete" @click="deleteTemplate(tpl.id)" aria-label="Delete template">
            <Icon name="trash" :size="14" />
          </button>
        </div>
      </div>
    </div>
  </div>
</template>

<style scoped>
.goal-library-view {
  padding: var(--space-4);
  height: 100%;
  overflow-y: auto;
  background: var(--bg-base);
}

.gl-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--space-4);
}

.gl-title {
  font-size: 20px;
  font-weight: 600;
  color: var(--text-primary);
}

.btn-primary {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 6px 12px;
  border: 1px solid var(--primary);
  border-radius: var(--radius-md);
  background: var(--primary-muted);
  color: var(--primary);
  font-size: 13px;
  cursor: pointer;
}
.btn-primary:hover { background: var(--primary-glow); }

.btn-ghost {
  padding: 6px 12px;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: transparent;
  color: var(--text-secondary);
  font-size: 13px;
  cursor: pointer;
}
.btn-ghost:hover { border-color: var(--text-muted); }

/* Form modal */
.form-overlay {
  position: fixed;
  top: 0;
  left: 0;
  right: 0;
  bottom: 0;
  background: rgba(0, 0, 0, 0.6);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
}

.form-modal {
  width: 500px;
  max-width: 90vw;
  max-height: 85vh;
  overflow-y: auto;
  background: var(--bg-surface);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-xl);
  display: flex;
  flex-direction: column;
}

.form-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-3) var(--space-4);
  border-bottom: 1px solid var(--border-subtle);
}

.form-title {
  font-size: 16px;
  font-weight: 600;
  color: var(--text-primary);
}

.form-close {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 24px;
  height: 24px;
  border: none;
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--text-muted);
  cursor: pointer;
}
.form-close:hover { color: var(--text-primary); background: var(--bg-elevated); }

.form-body {
  padding: var(--space-4);
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.form-field {
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.form-label {
  font-size: 12px;
  color: var(--text-muted);
}

.form-input, .form-textarea, .form-select {
  padding: 6px 10px;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: var(--bg-base);
  color: var(--text-primary);
  font-size: 13px;
  font-family: inherit;
}
.form-textarea {
  font-family: var(--font-mono, monospace);
  resize: vertical;
}
.form-input:focus-visible, .form-textarea:focus-visible, .form-select:focus-visible {
  outline: 2px solid var(--primary);
  outline-offset: 1px;
}

.form-footer {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-2);
  padding: var(--space-3) var(--space-4);
  border-top: 1px solid var(--border-subtle);
}

/* Template grid */
.template-grid {
  display: grid;
  grid-template-columns: repeat(auto-fill, minmax(300px, 1fr));
  gap: var(--space-3);
}

.template-card {
  padding: var(--space-3);
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.template-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-2);
}

.template-name {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
}

.template-badge {
  font-size: 10px;
  text-transform: uppercase;
  color: var(--text-muted);
  padding: 2px 6px;
  border-radius: var(--radius-sm);
  background: var(--bg-elevated);
  flex-shrink: 0;
}

.template-desc {
  font-size: 12px;
  color: var(--text-muted);
}

.template-goal-preview {
  font-size: 12px;
  color: var(--text-secondary);
  font-family: var(--font-mono, monospace);
  padding: var(--space-2);
  background: var(--bg-elevated);
  border-radius: var(--radius-md);
  line-height: 1.4;
}

.template-actions {
  display: flex;
  gap: var(--space-1);
  margin-top: auto;
}

.action-btn {
  display: flex;
  align-items: center;
  gap: 4px;
  padding: 4px 8px;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: var(--bg-elevated);
  color: var(--text-muted);
  font-size: 12px;
  cursor: pointer;
}
.action-btn:hover { color: var(--text-primary); border-color: var(--text-muted); }

.action-run {
  color: var(--primary);
  border-color: var(--primary);
}
.action-run:hover { background: var(--primary-muted); }

.action-delete:hover { color: var(--error); border-color: var(--error); }
</style>
