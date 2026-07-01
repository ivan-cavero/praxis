/**
 * useApi — Backend API communication composable.
 */

const API_BASE = '/api'

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
}

export interface ProviderKey { provider: string; key_masked: string; has_key: boolean }

function getToken(): string | null {
  return localStorage.getItem('praxis-token')
}

async function apiFetch<T>(path: string, options?: RequestInit): Promise<T> {
  const token = getToken()
  const headers: Record<string, string> = { ...(options?.headers as Record<string, string> || {}) }
  if (token) headers['Authorization'] = `Bearer ${token}`
  const res = await fetch(`${API_BASE}${path}`, { ...options, headers })
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
  const createProject = (name: string, description = '') =>
    apiFetch<Project>('/projects', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body: JSON.stringify({ name, description }),
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
  const get = apiFetch
  const post = (path: string, body: unknown) =>
    apiFetch(path, { method: 'POST', headers: { 'Content-Type': 'application/json' }, body: JSON.stringify(body) })
  const del = (path: string) => apiFetch(path, { method: 'DELETE' })

  return {
    getHealth, getMetricsSummary,
    getProjects, getProject, createProject, updateProject, deleteProject,
    getProjectConfig, updateProjectConfig,
    getVaultKeys, setVaultKey, deleteVaultKey,
    get, post, del,
  }
}
