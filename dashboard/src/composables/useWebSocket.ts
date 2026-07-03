/**
 * useWebSocket — Real-time event streaming from backend EventBus.
 *
 * Singleton pattern: the FIRST call creates the connection; subsequent
 * calls return the same shared refs. Always use within setup().
 *
 * The `kind` field is a structured JSON object mirroring the Rust
 * `MessageKind` enum — e.g.:
 *   { "TokenUsed": { "provider": "openai", "model": "gpt-5", "input": 100, "output": 50 } }
 *   { "PhaseChanged": { "from": "Planning", "to": "Implementing", ... } }
 *   { "AgentCompleted": { "agent": "coder", "status": "success", ... } }
 */

import { ref, onUnmounted } from 'vue'

export interface SystemEvent {
  id: string
  timestamp: string
  /** Structured JSON mirroring the Rust MessageKind enum */
  kind: Record<string, unknown>
  source: string
  metadata: Record<string, unknown>
}

// ─── Event kind type aliases for TypeScript convenience ─────────

export interface TokenUsedEvent {
  provider: string
  model: string
  input: number
  output: number
}

export interface PhaseChangedEvent {
  from: string
  to: string
  condition?: string
  timestamp?: string
}

export interface AgentCompletedEvent {
  agent: string
  role: string
  status: string
  duration_ms: number
  output_preview: string
}

export interface AgentStartedEvent {
  agent: string
  role: string
  phase: string
}

export interface ContextPressureEvent {
  pressure: number
  agent_id: string
  action: string
}

export interface ContextCompressionEvent {
  before_tokens: number
  after_tokens: number
  ratio: number
  technique: string
}

/**
 * Extract typed payload from a SystemEvent by kind variant name.
 * Returns `undefined` if the event doesn't match.
 */
export function getEventPayload<T>(event: SystemEvent, kindName: string): T | undefined {
  const payload = event.kind[kindName]
  if (payload && typeof payload === 'object') {
    return payload as T
  }
  return undefined
}

// ─── Module-level singleton state ───────────────────────────────
const isConnected = ref(false)
const events = ref<SystemEvent[]>([])
const maxEvents = 500

let ws: WebSocket | null = null
let reconnectTimeout: ReturnType<typeof setTimeout> | null = null
let reconnectDelay = 1000
let referenceCount = 0

function connect() {
  if (ws && (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING)) {
    return
  }

  const protocol = window.location.protocol === 'https:' ? 'wss:' : 'ws:'
  const host = window.location.host
  const url = `${protocol}//${host}/ws/global`

  ws = new WebSocket(url)

  ws.onopen = () => {
    isConnected.value = true
    reconnectDelay = 1000
  }

  ws.onmessage = (message) => {
    try {
      const data = JSON.parse(message.data) as SystemEvent
      events.value = [...events.value, data]
      if (events.value.length > maxEvents) {
        events.value = events.value.slice(-maxEvents)
      }
    } catch {
      // skip malformed messages
    }
  }

  ws.onclose = () => {
    isConnected.value = false
    reconnectTimeout = setTimeout(() => {
      reconnectDelay = Math.min(reconnectDelay * 2, 30000)
      connect()
    }, reconnectDelay)
  }

  ws.onerror = () => {
    // onclose will fire after onerror, triggering reconnect
  }
}

function disconnect() {
  if (reconnectTimeout) {
    clearTimeout(reconnectTimeout)
    reconnectTimeout = null
  }
  if (ws) {
    ws.close()
    ws = null
  }
  isConnected.value = false
}

function clearEvents() {
  events.value = []
}

// ─── Public API ─────────────────────────────────────────────────

export function useWebSocket() {
  // First call connects
  if (referenceCount === 0) {
    connect()
  }
  referenceCount++

  onUnmounted(() => {
    referenceCount--
    // Don't disconnect on unmount — keep alive for the whole SPA session
  })

  return {
    connected: isConnected,
    events,
    disconnect,
    clearEvents,
  }
}
