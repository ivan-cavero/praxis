/**
 * useConnection — Remote connection manager.
 *
 * Stores saved remote connections in localStorage and provides pairing flow.
 * When `currentConnection` is set, the dashboard API calls redirect to the remote server.
 *
 * ## Future (Fase B)
 * When praxis.dev exists, connections will be synced to the cloud account.
 * This composable will then fetch the connection list from api.praxis.dev
 * and pair using the cloud relay instead of direct LAN pairing.
 */

import { ref, computed } from 'vue'
import { setRemoteApi, clearRemoteApi } from './useApi'

// ─── Types ─────────────────────────────────────────────────────

export interface RemoteConnection {
  id: string
  name: string
  host: string
  port: number
  token: string
  deviceId: string
  lastSeen: string | null
  createdAt: string
}

export interface PairResponse {
  code: string
  qr_url: string
  expires_in: number
}

export interface PairStatus {
  status: 'pending' | 'claimed' | 'expired'
  device_name: string | null
  device_id: string | null
  expires_in: number
}

export interface TokenResponse {
  jwt: string
  device_id: string
  device_name: string
}

// ─── State ─────────────────────────────────────────────────────

const STORAGE_KEY = 'praxis-connections'
const connections = ref<RemoteConnection[]>(loadConnections())
const currentConnectionId = ref<string | null>(null)
const isPairing = ref(false)
const pairingStatus = ref<'idle' | 'generating' | 'waiting' | 'claimed' | 'expired' | 'error'>('idle')
const pairingCode = ref('')
const pairingQrUrl = ref('')
const pairingExpiresAt = ref(0)
const pairingError = ref<string | null>(null)

// ─── Computed ──────────────────────────────────────────────────

const currentConnection = computed<RemoteConnection | null>(() => {
  const id = currentConnectionId.value
  if (!id) return null
  return connections.value.find(c => c.id === id) ?? null
})

const isRemoteMode = computed(() => currentConnection.value !== null)

// ─── Helpers ───────────────────────────────────────────────────

function loadConnections(): RemoteConnection[] {
  try {
    const raw = localStorage.getItem(STORAGE_KEY)
    return raw ? JSON.parse(raw) : []
  } catch {
    return []
  }
}

function saveConnections() {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(connections.value))
}

/** Generate a unique ID for a connection. */
function generateId(): string {
  return crypto.randomUUID ? crypto.randomUUID() : `${Date.now()}-${Math.random().toString(36).slice(2, 9)}`
}

/** Format "last seen" as relative time. */
function formatLastSeen(rfc3339: string | null): string {
  if (!rfc3339) return 'Never'
  const then = new Date(rfc3339).getTime()
  const now = Date.now()
  const diffMs = now - then
  const diffMin = Math.floor(diffMs / 60000)
  if (diffMin < 1) return 'Just now'
  if (diffMin < 60) return `${diffMin}m ago`
  const diffHr = Math.floor(diffMin / 60)
  if (diffHr < 24) return `${diffHr}h ago`
  const diffDays = Math.floor(diffHr / 24)
  return `${diffDays}d ago`
}

// ─── API helper for remote calls ───────────────────────────────

async function remoteFetch<T>(host: string, port: number, path: string, options?: RequestInit): Promise<T> {
  const url = `http://${host}:${port}${path}`
  const res = await fetch(url, {
    ...options,
    headers: {
      'Content-Type': 'application/json',
      ...(options?.headers as Record<string, string> || {}),
    },
  })
  if (!res.ok) {
    const body = await res.text().catch(() => '')
    throw new Error(`Remote ${res.status}: ${body || res.statusText}`)
  }
  return res.json()
}

async function healthCheck(host: string, port: number): Promise<boolean> {
  try {
    const res = await fetch(`http://${host}:${port}/api/health`, { signal: AbortSignal.timeout(5000) })
    return res.ok
  } catch {
    return false
  }
}

// ─── Polling controller ────────────────────────────────────────

let pollTimer: ReturnType<typeof setInterval> | null = null

function startPolling(host: string, port: number, code: string, onClaimed: (jwt: string, deviceId: string) => void) {
  stopPolling()
  pollTimer = setInterval(async () => {
    try {
      const status = await remoteFetch<PairStatus>(host, port, `/api/pair/${code}/status`)
      if (status.status === 'claimed') {
        pairingStatus.value = 'claimed'
        // Retrieve the JWT
        const tokenData = await remoteFetch<TokenResponse>(host, port, `/api/pair/${code}/token`, { method: 'POST' })
        onClaimed(tokenData.jwt, tokenData.device_id)
        stopPolling()
      } else if (status.status === 'expired') {
        pairingStatus.value = 'expired'
        pairingError.value = 'Pairing code expired. Please try again.'
        stopPolling()
      }
    } catch (cause: unknown) {
      // Network errors during polling are expected (phone may not be reachable)
      // Only stop if the error persists — but we keep polling
    }
  }, 2000)
}

function stopPolling() {
  if (pollTimer !== null) {
    clearInterval(pollTimer)
    pollTimer = null
  }
}

// ─── Public API ────────────────────────────────────────────────

export function useConnection() {
  /** Get all saved connections. */
  function getConnections(): RemoteConnection[] {
    return connections.value
  }

  /** Get a connection by ID. */
  function getConnection(id: string): RemoteConnection | undefined {
    return connections.value.find(c => c.id === id)
  }

  /** Set the active remote connection (null = local mode). */
  function setCurrentConnection(id: string | null) {
    currentConnectionId.value = id
    if (id === null) {
      clearRemoteApi()
    } else {
      const conn = connections.value.find(c => c.id === id)
      if (conn) {
        setRemoteApi(conn.host, conn.port, conn.token)
      }
    }
  }

  /** Switch to local mode. */
  function switchToLocal() {
    currentConnectionId.value = null
    clearRemoteApi()
  }

  /** Start the pairing flow with a remote server. */
  async function startPairing(name: string, host: string, port: number): Promise<void> {
    isPairing.value = true
    pairingStatus.value = 'generating'
    pairingError.value = null

    try {
      const pairData = await remoteFetch<PairResponse>(host, port, '/api/pair', { method: 'POST' })
      pairingCode.value = pairData.code
      pairingQrUrl.value = pairData.qr_url
      pairingExpiresAt.value = Date.now() + pairData.expires_in * 1000
      pairingStatus.value = 'waiting'

      // Start polling for status changes
      startPolling(host, port, pairData.code, (jwt: string, deviceId: string) => {
        // Save the connection
        const connection: RemoteConnection = {
          id: generateId(),
          name,
          host,
          port,
          token: jwt,
          deviceId,
          lastSeen: new Date().toISOString(),
          createdAt: new Date().toISOString(),
        }
        connections.value = [...connections.value, connection]
        saveConnections()
        currentConnectionId.value = connection.id
        isPairing.value = false
      })
    } catch (cause: unknown) {
      pairingStatus.value = 'error'
      pairingError.value = cause instanceof Error ? cause.message : 'Connection failed'
      isPairing.value = false
    }
  }

  /** Cancel the pairing flow. */
  function cancelPairing() {
    stopPolling()
    isPairing.value = false
    pairingStatus.value = 'idle'
    pairingCode.value = ''
    pairingQrUrl.value = ''
    pairingExpiresAt.value = 0
    pairingError.value = null
  }

  /** Remove a saved connection. */
  function removeConnection(id: string) {
    connections.value = connections.value.filter(c => c.id !== id)
    saveConnections()
    if (currentConnectionId.value === id) {
      currentConnectionId.value = null
    }
  }

  /** Update lastSeen for a connection. */
  function touchConnection(id: string) {
    const conn = connections.value.find(c => c.id === id)
    if (conn) {
      const updated = [...connections.value.filter(c => c.id !== id), { ...conn, lastSeen: new Date().toISOString() }]
      connections.value = updated
      saveConnections()
    }
  }

  /** Test if a remote server is reachable. */
  async function testConnection(host: string, port: number): Promise<boolean> {
    return healthCheck(host, port)
  }

  return {
    // State
    connections,
    currentConnection,
    currentConnectionId,
    isRemoteMode,
    isPairing,
    pairingStatus,
    pairingCode,
    pairingQrUrl,
    pairingExpiresAt,
    pairingError,

    // Methods
    getConnections,
    getConnection,
    setCurrentConnection,
    switchToLocal,
    startPairing,
    cancelPairing,
    removeConnection,
    touchConnection,
    testConnection,
    formatLastSeen,
  }
}