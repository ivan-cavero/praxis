/**
 * useTauriIpc — invoke Tauri IPC commands from the desktop backend.
 *
 * In browser dev mode, these calls silently fall back to HTTP API.
 * In Tauri, they invoke the Rust commands directly.
 */

import { invoke } from '@tauri-apps/api/core'
import { useApi } from './useApi'

// ─── Types ──────────────────────────────────────────────────────────

export interface DesktopStatus {
  healthy: boolean
  uptime_seconds: number
  active_sessions: number
}

export interface DesktopSession {
  session_id: string
  project: string
  status: string
  phase: string
  iteration: number
  started_at: string
}

export interface DesktopMetrics {
  sessions_total: number
  tokens_used: number
  agents_completed: number
  pathologies_detected: number
}

export interface RunGoalResult {
  session_id: string
  outcome: string
  iterations: number
  message: string
}

// ─── IPC Composable ─────────────────────────────────────────────────

export function useTauriIpc() {
  const api = useApi()

  /**
   * Health / status — tries IPC first, falls back to HTTP API.
   */
  async function getStatus(): Promise<DesktopStatus> {
    try {
      const result = await invoke<DesktopStatus>('get_status')
      return result
    } catch {
      // Browser dev mode fallback
      const health = await api.getHealth()
      return {
        healthy: health.status === 'ok',
        uptime_seconds: health.uptime_seconds,
        active_sessions: 0,
      }
    }
  }

  /**
   * Version string from backend.
   */
  async function getVersion(): Promise<string> {
    try {
      return await invoke<string>('get_version')
    } catch {
      const health = await api.getHealth()
      return health.version
    }
  }

  /**
   * List active/completed sessions.
   */
  async function getSessions(): Promise<DesktopSession[]> {
    try {
      return await invoke<DesktopSession[]>('get_sessions')
    } catch {
      // Fallback: try HTTP API sessions endpoint
      try {
        const result = await api.get<{ sessions: DesktopSession[] }>('/sessions')
        return result.sessions
      } catch {
        return []
      }
    }
  }

  /**
   * Run a goal in a new session.
   */
  async function runGoal(
    project: string,
    goal: string,
    model?: string,
  ): Promise<RunGoalResult> {
    try {
      return await invoke<RunGoalResult>('run_goal', {
        project,
        goal,
        model: model ?? null,
      })
    } catch {
      // Fallback via HTTP
      return api.post<RunGoalResult>('/sessions', {
        project,
        goal,
        model: model ?? undefined,
      })
    }
  }

  /**
   * Stop a running session.
   */
  async function stopSession(sessionId: string): Promise<boolean> {
    try {
      await invoke('stop_session', { sessionId })
      return true
    } catch {
      try {
        await api.del(`/sessions/${sessionId}`)
        return true
      } catch {
        return false
      }
    }
  }

  /**
   * Get metrics summary.
   */
  async function getMetrics(): Promise<DesktopMetrics> {
    try {
      return await invoke<DesktopMetrics>('get_metrics')
    } catch {
      try {
        return api.get<DesktopMetrics>('/metrics/summary')
      } catch {
        return { sessions_total: 0, tokens_used: 0, agents_completed: 0, pathologies_detected: 0 }
      }
    }
  }

  return {
    getStatus,
    getVersion,
    getSessions,
    runGoal,
    stopSession,
    getMetrics,
  }
}
