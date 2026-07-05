/**
 * useApi — Backend API communication composable.
 *
 * Works in three modes:
 * 1. Tauri desktop: uses the API port emitted via `api:ready` event
 * 2. Browser dev (standalone Vite): uses `/api` proxy (localhost:8080)
 * 3. Remote mode: all calls go to a remote praxis server (host:port)
 *
 * The `setApiPort()` function is called by App.vue when the Tauri backend
 * emits the `api:ready` event with the port number.
 * The `setRemoteApi()`/`clearRemoteApi()` functions are called by
 * `useConnection()` when switching between local and remote mode.
 */

import { ref } from 'vue'

// In Tauri, this gets updated via `api:ready` event.
// In browser dev mode, Vite proxies `/api` to the API server.
export const apiPort = ref<number | null>(null)

/** Remote API override (set when connected to a remote server). */
const remoteApi = ref<{ host: string; port: number; token: string } | null>(null)
const isRemoteMode = ref(false)

/** Compute the API base URL dynamically. */
function apiBase(): string {
  if (remoteApi.value !== null) {
    return `http://${remoteApi.value.host}:${remoteApi.value.port}/api`
  }
  if (apiPort.value !== null) {
    return `http://127.0.0.1:${apiPort.value}/api`
  }
  return '/api'
}

/** Set the API port (called from Tauri event listener). */
export function setApiPort(port: number) {
  apiPort.value = port
}

/** Switch to remote API mode (called from useConnection). */
export function setRemoteApi(host: string, port: number, token: string) {
  remoteApi.value = { host, port, token }
  isRemoteMode.value = true
}

/** Switch back to local API mode. */
export function clearRemoteApi() {
  remoteApi.value = null
  isRemoteMode.value = false
}

/** Get the remote mode status for UI indicators. */
export function useRemoteStatus() {
  return { isRemoteMode, remoteHost: remoteApi.value?.host ?? null, remotePort: remoteApi.value?.port ?? null }
}

export interface HealthStatus {
  status: string; version: string; uptime_seconds: number
}

export interface Project {
  id: string
  name: string
  description: string
  created_at: string
  last_active: string
  forge_toml: string
  /** Path to the per-project directory (null for legacy projects). */
  path?: string | null
}

export interface ProjectConfig {
  raw: string
  roles: Record<string, RoleDetail>
  providers: Record<string, ProviderDetail>
  goals: GoalDetail[]
  limits: LimitsDetail
  project: { name: string; version: string }
}

export interface RoleDetail {
  model: string; temperature: number; max_tokens: number
  system_prompt: string; tools: string[]; description: string
}

export interface ProviderDetail {
  base_url: string; api_key_ref: string; default_model: string
}

export interface GoalDetail {
  name: string; agents: string[]; max_iterations: number; gates: string[]
}

export interface LimitsDetail {
  max_iterations_per_goal: number; max_iterations_per_phase: number
  session_ttl_seconds: number; phase_timeout_seconds: number
  max_tokens?: number | null
  max_cost_usd?: number | null
}

export interface ProviderKey { provider: string; key_masked: string; has_key: boolean }

function getToken(): string | null {
  // When in remote mode, use the remote connection's token
  if (remoteApi.value !== null) {
    return remoteApi.value.token
  }
  return localStorage.getItem('praxis-token')
}

async function apiFetch<T>(path: string, options?: RequestInit): Promise<T> {
  const token = getToken()
  const headers: Record<string, string> = { ...(options?.headers as Record<string, string> || {}) }
  if (token) headers['Authorization'] = `Bearer ${token}`
  const base = apiBase()
  const url = `${base}${path}`
  const res = await fetch(url, { ...options, headers })
  if (!res.ok) {
    const body = await res.text().catch(() => '')
    throw new Error(`API ${res.status}: ${body || res.statusText}`)
  }
  return res.json()
}

export function useApi() {
  // Health & Metrics
  const getHealth = () => apiFetch<HealthStatus>('/health')
  const getMetricsSummary = () => apiFetch<{ version: string; uptime_seconds: number; active_sessions: number; total_tokens: number; avg_asi_score: number }>('/metrics/summary')

  // Projects
  const getProjects = () => apiFetch<Project[]>('/projects')
  const getProject = (id: string) => apiFetch<Project>(`/projects/${id}`)
  const createProject = (name: string, description = '', path = '') =>
    apiFetch<Project>('/projects', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ name, description, path: path || undefined }),
    })
  const updateProject = (id: string, data: { name?: string; description?: string; forge_toml?: string }) =>
    apiFetch<Project>(`/projects/${id}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(data),
    })
  const deleteProject = (id: string) => apiFetch(`/projects/${id}`, { method: 'DELETE' })

  // Project Config
  const getProjectConfig = (id: string) => apiFetch<ProjectConfig>(`/projects/${id}/config`)
  const updateProjectConfig = (id: string, config: string) =>
    apiFetch<ProjectConfig>(`/projects/${id}/config`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ config }),
    })

  // Vault
  const getVaultKeys = () => apiFetch<{ providers: ProviderKey[]; total: number }>('/vault/keys')
  const setVaultKey = (provider: string, api_key: string, base_url?: string) =>
    apiFetch('/vault/keys', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ provider, api_key, base_url }),
    })
  const deleteVaultKey = (provider: string) => apiFetch(`/vault/keys/${provider}`, { method: 'DELETE' })

  // Generic
  const get = <T,>(path: string, options?: RequestInit): Promise<T> => apiFetch<T>(path, options)
  const post = <T,>(path: string, body: unknown): Promise<T> =>
    apiFetch<T>(path, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) })
  const del = <T,>(path: string): Promise<T> => apiFetch<T>(path, { method: 'DELETE' })

  // Sessions
  const getSessions = () => apiFetch<SessionEntry[]>('/sessions')
  const getSession = (id: string) => apiFetch<SessionEntry>(`/sessions/${id}`)
  const stopSession = (id: string) => apiFetch<{ status: string; session_id: string }>(`/sessions/${id}/stop`, { method: 'POST' })
  const getSessionState = (id: string) => apiFetch<SessionStateResponse>(`/sessions/${id}/state`)

  // Goal execution — start a goal run in the background
  const runGoal = (projectId: string, req: RunGoalRequest) =>
    apiFetch<RunGoalResponse>(`/projects/${projectId}/run`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(req),
    })

  // Plan mode — generate a plan without executing
  const planGoal = (projectId: string, req: RunGoalRequest) =>
    apiFetch<PlanGoalResponse>(`/projects/${projectId}/plan`, {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(req),
    })

  // Skills — list available built-in skills
  const getSkills = () => apiFetch<SkillInfo[]>('/skills')

  // Agents
  const getAgents = () => apiFetch<AgentDefinition[]>('/agents')
  const getAgent = (name: string) => apiFetch<AgentDefinition>(`/agents/${name}`)
  const createAgent = (req: CreateAgentRequest) =>
    apiFetch<AgentDefinition>('/agents', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(req),
    })
  const updateAgent = (name: string, req: CreateAgentRequest) =>
    apiFetch<AgentDefinition>(`/agents/${name}`, {
      method: 'PUT',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify(req),
    })
  const deleteAgent = (name: string) =>
    apiFetch<{ deleted: string; scope: string }>(`/agents/${name}`, {
      method: 'DELETE',
    })

  // Inject
  const sendInject = (targetAgent: string, messageType: string, content: string) =>
    apiFetch<{ status: string; file: string; target_agent: string; message_type: string }>('/inject', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ target_agent: targetAgent, message_type: messageType, content }),
    })

  return {
    getHealth, getMetricsSummary,
    getProjects, getProject, createProject, updateProject, deleteProject,
    getProjectConfig, updateProjectConfig,
    getVaultKeys, setVaultKey, deleteVaultKey,
    getSessions, getSession, stopSession, getSessionState,
    runGoal, planGoal, getSkills,
    getAgents, getAgent, createAgent, updateAgent, deleteAgent,
    sendInject,
    get, post, del,
  }
}

export interface SessionEntry {
  id: string
  project: string
  goal: string
  phase: string
  iteration: number
  status: string
  started_at: string
  completed_at: string | null
  tokens_used?: number
  cost_usd?: number
}

export interface RunGoalRequest {
  goal: string
  completion?: string
  until?: string
  max_tokens?: number
  max_cost_usd?: number
  parallel_reviewers?: number
  /** Built-in skill IDs to enable (e.g., ["rust-best-practices", "security"]). */
  skills?: string[]
  /** Create a git worktree for this session (isolated working directory). */
  worktree?: boolean
}

export interface RunGoalResponse {
  session_id: string
  project_id: string
  goal: string
  status: string
}

export interface SessionStateResponse {
  session_id: string
  phase: string
  iteration: number
  tokens_used: number
  cost_usd: number
  status: string
  state_file?: string
}

export interface PlanGoalResponse {
  plan_id: string
  project_id: string
  goal: string
  plan: string
  plan_path: string
  status: string
}

export interface SkillInfo {
  id: string
  name: string
  description: string
}

export interface AgentSummary {
  name: string
  role: string
  model: string
  tools: string[]
  status: string
}

// ─── Agent CRUD ───────────────────────────────────────────────

export interface AgentDefinition {
  name: string
  description: string
  model: string
  temperature: number
  max_tokens: number
  tools: string[]
  max_turns: number
  max_depth: number
  can_spawn: string[]
  system_prompt: string
  scope: string
}

export interface CreateAgentRequest {
  name: string
  description?: string
  model?: string
  temperature?: number
  max_tokens?: number
  tools?: string[]
  max_turns?: number
  max_depth?: number
  can_spawn?: string[]
  system_prompt: string
  scope?: string
}
