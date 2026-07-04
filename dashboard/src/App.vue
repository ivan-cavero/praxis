<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useRouter, useRoute } from 'vue-router'
import { storeToRefs } from 'pinia'
import { useAppStore } from './stores/app'
import { setApiPort } from './composables/useApi'
import { useWebSocket } from './composables/useWebSocket'
import { useUpdater } from './composables/useUpdater'
import TitleBar from './components/layout/TitleBar.vue'
import Icon from './components/ui/Icon.vue'
import LoginView from './views/LoginView.vue'
import SettingsDialog from './views/SettingsDialog.vue'

const router = useRouter()
const route = useRoute()
const store = useAppStore()
const ws = useWebSocket()
const updater = useUpdater()

// ─── Store refs ───────────────────────────────────────────────────
const { projects } = storeToRefs(store)

// ─── Auth ─────────────────────────────────────────────────────────
const isAuthenticated = ref(false)

// ─── Settings Dialog ─────────────────────────────────────────────
const showSettings = ref(false)

function openSettings() {
  showSettings.value = true
}

function closeSettings() {
  showSettings.value = false
}

// ─── New Project ─────────────────────────────────────────────────
const showNewProject = ref(false)
const newProjectName = ref('')
const newProjectDescription = ref('')
const isCreatingProject = ref(false)
const createError = ref<string | null>(null)

async function handleCreateProject() {
  if (!newProjectName.value.trim()) return
  isCreatingProject.value = true
  createError.value = null
  try {
    await store.createProject(newProjectName.value.trim(), newProjectDescription.value.trim())
    newProjectName.value = ''
    newProjectDescription.value = ''
    showNewProject.value = false
  } catch (caughtError: unknown) {
    createError.value = caughtError instanceof Error ? caughtError.message : 'Failed to create project'
  } finally {
    isCreatingProject.value = false
  }
}

// ─── Navigation ───────────────────────────────────────────────────
const navItems = [
  { name: 'dashboard', label: 'Dashboard', icon: 'dashboard', route: '/' },
  { name: 'pipeline', label: 'Pipeline', icon: 'chart-line', route: '/pipeline' },
  { name: 'sessions', label: 'Sessions', icon: 'server', route: '/sessions' },
]

const currentRouteName = computed(() => route.name as string || 'dashboard')

function navigateTo(item: typeof navItems[number]) {
  router.push(item.route)
}

// ─── Tauri Events ─────────────────────────────────────────────────
async function listenTauriEvents() {
  try {
    const { listen } = await import('@tauri-apps/api/event')
    await listen<number>('api:ready', (event) => {
      setApiPort(event.payload)
    })
    await listen('core:ready', () => {
      // Core runtime ready
    })
    // System tray events
    await listen('tray:new_session', () => {
      router.push('/sessions')
    })
    await listen('tray:settings', () => {
      openSettings()
    })
  } catch {
    // Not in Tauri — running in browser dev mode
  }
}

// ─── Restart ──────────────────────────────────────────────────────
async function restartApp() {
  try {
    const { relaunch } = await import('@tauri-apps/plugin-process')
    await relaunch()
  } catch {
    window.location.reload()
  }
}

// ─── Project selection ────────────────────────────────────────────
function handleProjectClick(projectId: string) {
  store.selectProject(projectId)
  // Could navigate to project detail or just select
}

// ─── Lifecycle ────────────────────────────────────────────────────
let refreshInterval: ReturnType<typeof setInterval> | null = null

onMounted(async () => {
  await listenTauriEvents()
  updater.checkForUpdates()

  const token = localStorage.getItem('praxis-token')
  if (token) {
    isAuthenticated.value = true
    store.refreshAll()
    refreshInterval = setInterval(() => store.refreshAll(), 10000)
  }
})

onUnmounted(() => {
  if (refreshInterval) clearInterval(refreshInterval)
})

function handleLogin(token: string) {
  isAuthenticated.value = true
  localStorage.setItem('praxis-token', token)
  store.refreshAll()
  refreshInterval = setInterval(() => store.refreshAll(), 10000)
}
</script>

<template>
  <!-- Login screen (unauthenticated) -->
  <LoginView v-if="!isAuthenticated" @login="handleLogin" />

  <!-- Main app (authenticated) -->
  <template v-else>
    <div class="layout-wrapper">
      <!-- Update Banner -->
      <div
        v-if="updater.updateAvailable.value && !updater.dismissed.value"
        class="update-banner"
      >
        <div class="update-banner-content">
          <Icon name="download" :size="16" class="update-icon" />
          <span v-if="updater.installDone.value">
            Update {{ updater.updateVersion.value }} installed — restart to apply
          </span>
          <span v-else-if="updater.installing.value">
            Installing update...
          </span>
          <span v-else-if="updater.downloading.value">
            Downloading update {{ updater.updateVersion.value }} —
            {{ updater.progressPercent() }}%
          </span>
          <span v-else>
            Update {{ updater.updateVersion.value }} available
            <span v-if="updater.updateBody.value" class="update-body">
              — {{ updater.updateBody.value.slice(0, 80) }}...
            </span>
          </span>
        </div>
        <div class="update-banner-actions">
          <button
            v-if="!updater.downloading.value && !updater.installDone.value"
            class="update-btn"
            @click="updater.installUpdate()"
          >
            Install
          </button>
          <button
            v-if="updater.installDone.value"
            class="update-btn"
            @click="restartApp()"
          >
            Restart
          </button>
          <button class="update-dismiss" @click="updater.dismissUpdate()" title="Dismiss">
            &times;
          </button>
        </div>
      </div>

      <!-- Title Bar (custom window chrome) -->
      <TitleBar />

      <!-- Main Layout -->
      <div class="layout">
        <!-- Sidebar -->
        <aside class="sidebar">
          <!-- Navigation -->
          <div class="sidebar-nav">
            <button
              v-for="item in navItems"
              :key="item.name"
              class="nav-item"
              :class="{ active: currentRouteName === item.name }"
              @click="navigateTo(item)"
            >
              <Icon :name="item.icon" :size="18" class="nav-icon" />
              <span class="nav-label">{{ item.label }}</span>
            </button>
          </div>

          <!-- Projects Section -->
          <div class="sidebar-section">
            <div class="sidebar-section-header">
              <span class="sidebar-section-title">Projects</span>
              <button
                class="sidebar-section-action"
                @click="showNewProject = true"
                title="New Project"
              >
                <Icon name="plus" :size="14" />
              </button>
            </div>
            <div v-if="projects.length === 0" class="sidebar-hint">
              No projects yet
            </div>
            <div v-else class="sidebar-projects">
              <button
                v-for="project in projects"
                :key="project.id"
                class="project-item"
                :class="{ active: store.activeProject?.id === project.id }"
                @click="handleProjectClick(project.id)"
              >
                <Icon name="folder" :size="14" class="project-icon" />
                <span class="project-name">{{ project.name }}</span>
              </button>
            </div>
          </div>

          <!-- Spacer -->
          <div class="sidebar-spacer" />

          <!-- Settings & Status -->
          <div class="sidebar-status">
            <button
              class="nav-item settings-btn"
              @click="openSettings()"
            >
              <Icon name="settings" :size="18" class="nav-icon" />
              <span class="nav-label">Settings</span>
            </button>
          </div>

          <div class="sidebar-footer">
            <div class="status-row" :class="{ online: ws.connected.value }">
              <span class="status-dot" />
              <span class="status-label">{{ ws.connected.value ? 'Connected' : 'Offline' }}</span>
            </div>
            <div class="sidebar-user">
              <div class="user-avatar">I</div>
              <span class="user-name">Ivan</span>
            </div>
          </div>
        </aside>

        <!-- Main Content Area -->
        <div class="main-content">
          <router-view />
        </div>
      </div>
    </div>

    <!-- Settings Dialog (overlay) -->
    <SettingsDialog
      v-if="showSettings"
      @close="closeSettings()"
    />

    <!-- New Project Modal -->
    <div v-if="showNewProject" class="modal-overlay" @click.self="showNewProject = false" @keydown.esc="showNewProject = false">
      <div class="modal-card modal-card-sm">
        <div class="modal-header">
          <h3 class="modal-title">New Project</h3>
          <button class="modal-close" @click="showNewProject = false">
            <Icon name="x" :size="18" />
          </button>
        </div>
        <div class="modal-body">
          <div class="input-group">
            <label class="input-label">Project Name</label>
            <input
              v-model="newProjectName"
              class="input"
              placeholder="e.g. my-awesome-app"
              @keydown.enter="handleCreateProject"
              autofocus
            />
          </div>
          <div class="input-group">
            <label class="input-label">Description (optional)</label>
            <input
              v-model="newProjectDescription"
              class="input"
              placeholder="What is this project about?"
            />
          </div>
          <div v-if="createError" class="form-error">
            <Icon name="alert-circle" :size="14" />
            <span>{{ createError }}</span>
          </div>
        </div>
        <div class="modal-footer">
          <button class="btn btn-ghost" @click="showNewProject = false">Cancel</button>
          <button
            class="btn btn-primary"
            :disabled="!newProjectName.trim() || isCreatingProject"
            @click="handleCreateProject()"
          >
            <Icon v-if="isCreatingProject" name="refresh" :size="14" class="animate-spin" />
            <Icon v-else name="plus" :size="14" />
            {{ isCreatingProject ? 'Creating...' : 'Create Project' }}
          </button>
        </div>
      </div>
    </div>
  </template>
</template>

<style scoped>
/* ─── Sidebar Status ──────────────────────────────────────────── */

.sidebar-section {
  padding: var(--space-2) var(--space-3) 0;
  display: flex;
  flex-direction: column;
}

.sidebar-section-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: var(--space-1) var(--space-1) var(--space-1) var(--space-3);
}

.sidebar-section-title {
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: var(--text-muted);
}

.sidebar-section-action {
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
  transition: all 0.15s;
}

.sidebar-section-action:hover {
  color: var(--text-primary);
  background: var(--bg-hover);
}

.sidebar-hint {
  font-size: 12px;
  color: var(--text-disabled);
  padding: var(--space-2) var(--space-3);
}

.sidebar-projects {
  display: flex;
  flex-direction: column;
  gap: 1px;
  max-height: 240px;
  overflow-y: auto;
  padding: var(--space-1) 0;
}

.project-item {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-1) var(--space-3);
  border-radius: var(--radius-md);
  border: none;
  background: transparent;
  color: var(--text-secondary);
  font-size: 13px;
  cursor: pointer;
  transition: all var(--transition-fast);
  text-align: left;
  width: 100%;
  font-family: inherit;
}

.project-item:hover {
  color: var(--text-primary);
  background: var(--bg-hover);
}

.project-item.active {
  color: var(--text-primary);
  background: var(--bg-elevated);
}

.project-icon {
  flex-shrink: 0;
  opacity: 0.6;
}

.project-name {
  flex: 1;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.sidebar-spacer {
  flex: 1;
}

.sidebar-status {
  padding: var(--space-1) var(--space-3) 0;
}

.settings-btn {
  border: 1px solid transparent;
}

.settings-btn:hover {
  border-color: var(--border-subtle);
}

.status-row {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  font-size: 12px;
  color: var(--text-muted);
}

.status-row.online {
  color: var(--primary);
}

.status-dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--text-disabled);
  flex-shrink: 0;
}

.status-row.online .status-dot {
  background: var(--primary);
  box-shadow: 0 0 6px var(--primary-glow);
}

/* ─── Update Banner ──────────────────────────────────────────── */

.update-banner {
  display: flex;
  align-items: center;
  justify-content: space-between;
  gap: var(--space-3);
  padding: var(--space-2) var(--space-4);
  background: var(--primary);
  color: var(--bg-base);
  font-size: 13px;
  flex-shrink: 0;
}

.update-banner-content {
  display: flex;
  align-items: center;
  gap: var(--space-2);
}

.update-icon {
  flex-shrink: 0;
  opacity: 0.8;
}

.update-body {
  opacity: 0.7;
}

.update-banner-actions {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  flex-shrink: 0;
}

.update-btn {
  padding: var(--space-1) var(--space-3);
  border: 1px solid var(--bg-base);
  border-radius: var(--radius-sm);
  background: transparent;
  color: var(--bg-base);
  font-size: 12px;
  font-weight: 600;
  cursor: pointer;
  transition: background 0.15s, color 0.15s;
  font-family: inherit;
}

.update-btn:hover {
  background: var(--bg-base);
  color: var(--primary);
}

.update-dismiss {
  padding: 0 var(--space-1);
  border: none;
  background: transparent;
  color: var(--bg-base);
  font-size: 18px;
  cursor: pointer;
  opacity: 0.6;
  line-height: 1;
  font-family: inherit;
}

.update-dismiss:hover {
  opacity: 1;
}

/* ─── New Project Modal ────────────────────────────────────────── */

.form-error {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-md);
  background: rgba(239, 68, 68, 0.1);
  color: var(--error);
  font-size: 13px;
}

/* Override modal-card for small size */
.modal-card-sm {
  width: 380px;
}
</style>
