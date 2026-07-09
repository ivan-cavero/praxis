/**
 * useLiveMonitor — processes WebSocket events into a live session monitor state.
 *
 * Consumes the raw event stream from useWebSocket and derives:
 * - Current phase + iteration
 * - Active agents (running, done, queued)
 * - Per-agent tool calls
 * - Streaming output text
 * - Delegation tree (parent → child)
 * - Total tokens + cost
 */

import { computed, type Ref } from 'vue'
import { useWebSocket, type SystemEvent, type AgentStartedEvent, type AgentCompletedEvent, type AgentOutputEvent, type ToolCalledEvent, type PhaseChangedEvent, type TokenUsedEvent, type DelegationStartedEvent, type DelegationCompletedEvent } from './useWebSocket'

export interface AgentState {
  name: string
  role: string
  status: 'running' | 'completed' | 'failed' | 'queued'
  startedAt: number | null
  completedAt: number | null
  durationMs: number | null
  tokensUsed: number
  toolCalls: ToolCallInfo[]
  outputChunks: string[]
  delegatedTo: string[]
  delegatedFrom: string | null
}

export interface ToolCallInfo {
  tool: string
  durationMs: number
  success: boolean
  timestamp: number
}

export interface LiveMonitorState {
  phase: string
  iteration: number
  agents: Map<string, AgentState>
  totalTokens: number
  totalCost: number
  delegations: DelegationInfo[]
  streamingOutput: string
}

export interface DelegationInfo {
  parent: string
  child: string
  status: 'running' | 'completed' | 'failed'
  taskPreview: string
  resultPreview: string | null
  durationMs: number | null
  tokensUsed: number
}

/**
 * Build a live monitor state from the WebSocket event stream.
 * Returns a computed ref that updates as events arrive.
 */
export function useLiveMonitor(sessionId: Ref<string | null>) {
  const { events } = useWebSocket()

  const monitorState = computed<LiveMonitorState>(() => {
    const sid = sessionId.value
    const agents = new Map<string, AgentState>()
    let phase = ''
    let iteration = 0
    let totalTokens = 0
    let totalCost = 0
    const delegations: DelegationInfo[] = []
    const streamingOutput: string[] = []

    // Filter events for this session — events now carry session_id in metadata.
    // If the event has no session_id (legacy), include it (backward compat).
    // If the event has a session_id, only include it if it matches.
    const allEvents = sid
      ? events.value.filter(e => {
          const eventSid = e.metadata?.session_id
          return !eventSid || eventSid === sid
        })
      : events.value

    // Process events in order
    for (const event of allEvents) {
      processEvent(event, agents, {
        get phase() { return phase },
        set phase(v: string) { phase = v },
        get iteration() { return iteration },
        set iteration(v: number) { iteration = v },
        get totalTokens() { return totalTokens },
        set totalTokens(v: number) { totalTokens = v },
        get totalCost() { return totalCost },
        set totalCost(v: number) { totalCost = v },
        delegations,
        streamingOutput,
      })
    }

    return {
      phase,
      iteration,
      agents,
      totalTokens,
      totalCost,
      delegations,
      streamingOutput: streamingOutput.join(''),
    }
  })

  return {
    state: monitorState,
    agents: computed(() => Array.from(monitorState.value.agents.values())),
    activeAgents: computed(() =>
      Array.from(monitorState.value.agents.values()).filter(a => a.status === 'running')
    ),
    completedAgents: computed(() =>
      Array.from(monitorState.value.agents.values()).filter(a => a.status === 'completed' || a.status === 'failed')
    ),
  }
}

interface MutableState {
  phase: string
  iteration: number
  totalTokens: number
  totalCost: number
  delegations: DelegationInfo[]
  streamingOutput: string[]
}

function processEvent(event: SystemEvent, agents: Map<string, AgentState>, state: MutableState) {
  // PhaseChanged
  const phaseEvent = getPayload<PhaseChangedEvent>(event, 'PhaseChanged')
  if (phaseEvent) {
    state.phase = phaseEvent.to
    state.iteration++
    return
  }

  // AgentStarted
  const started = getPayload<AgentStartedEvent>(event, 'AgentStarted')
  if (started) {
    const existing = agents.get(started.agent)
    if (existing) {
      existing.status = 'running'
      existing.startedAt = Date.now()
    } else {
      agents.set(started.agent, {
        name: started.agent,
        role: started.role,
        status: 'running',
        startedAt: Date.now(),
        completedAt: null,
        durationMs: null,
        tokensUsed: 0,
        toolCalls: [],
        outputChunks: [],
        delegatedTo: [],
        delegatedFrom: null,
      })
    }
    return
  }

  // AgentOutput (streaming)
  const output = getPayload<AgentOutputEvent>(event, 'AgentOutput')
  if (output) {
    const agent = agents.get(output.agent)
    if (agent) {
      agent.outputChunks.push(output.delta)
    }
    state.streamingOutput.push(output.delta)
    return
  }

  // AgentCompleted
  const completed = getPayload<AgentCompletedEvent>(event, 'AgentCompleted')
  if (completed) {
    const agent = agents.get(completed.agent)
    if (agent) {
      agent.status = completed.status === 'Completed' ? 'completed' : 'failed'
      agent.completedAt = Date.now()
      agent.durationMs = completed.duration_ms
    }
    return
  }

  // ToolCalled
  const toolCall = getPayload<ToolCalledEvent>(event, 'ToolCalled')
  if (toolCall) {
    const agent = agents.get(toolCall.agent)
    if (agent) {
      agent.toolCalls.push({
        tool: toolCall.tool,
        durationMs: toolCall.duration_ms,
        success: toolCall.success,
        timestamp: Date.now(),
      })
    }
    return
  }

  // TokenUsed
  const tokenUsed = getPayload<TokenUsedEvent>(event, 'TokenUsed')
  if (tokenUsed) {
    state.totalTokens += tokenUsed.input + tokenUsed.output
    // Rough cost estimate — would use the pricing table in production
    state.totalCost += estimateCost(tokenUsed.model, tokenUsed.input, tokenUsed.output)
    return
  }

  // DelegationStarted
  const delegationStarted = getPayload<DelegationStartedEvent>(event, 'DelegationStarted')
  if (delegationStarted) {
    const parent = agents.get(delegationStarted.parent)
    if (parent) {
      parent.delegatedTo.push(delegationStarted.child)
    }
    const child = agents.get(delegationStarted.child)
    if (child) {
      child.delegatedFrom = delegationStarted.parent
    } else {
      agents.set(delegationStarted.child, {
        name: delegationStarted.child,
        role: delegationStarted.child,
        status: 'running',
        startedAt: Date.now(),
        completedAt: null,
        durationMs: null,
        tokensUsed: 0,
        toolCalls: [],
        outputChunks: [],
        delegatedTo: [],
        delegatedFrom: delegationStarted.parent,
      })
    }
    state.delegations.push({
      parent: delegationStarted.parent,
      child: delegationStarted.child,
      status: 'running',
      taskPreview: delegationStarted.task_preview,
      resultPreview: null,
      durationMs: null,
      tokensUsed: 0,
    })
    return
  }

  // DelegationCompleted
  const delegationCompleted = getPayload<DelegationCompletedEvent>(event, 'DelegationCompleted')
  if (delegationCompleted) {
    const child = agents.get(delegationCompleted.child)
    if (child) {
      child.status = delegationCompleted.status === 'Completed' ? 'completed' : 'failed'
      child.completedAt = Date.now()
      child.durationMs = delegationCompleted.duration_ms
      child.tokensUsed = delegationCompleted.tokens_used
    }
    // Update the delegation entry
    const delegation = state.delegations.find(
      d => d.parent === delegationCompleted.parent && d.child === delegationCompleted.child && d.status === 'running'
    )
    if (delegation) {
      delegation.status = delegationCompleted.status === 'Completed' ? 'completed' : 'failed'
      delegation.resultPreview = delegationCompleted.result_preview
      delegation.durationMs = delegationCompleted.duration_ms
      delegation.tokensUsed = delegationCompleted.tokens_used
    }
    return
  }
}

function getPayload<T>(event: SystemEvent, kindName: string): T | undefined {
  const payload = event.kind[kindName]
  if (payload && typeof payload === 'object') {
    return payload as T
  }
  return undefined
}

// Rough cost estimate per model (simplified from the Rust pricing table)
const PRICING: Record<string, { input: number; output: number }> = {
  'gpt-5': { input: 5.0 / 1_000_000, output: 15.0 / 1_000_000 },
  'gpt-5-mini': { input: 0.15 / 1_000_000, output: 0.6 / 1_000_000 },
  'gpt-5-nano': { input: 0.1 / 1_000_000, output: 0.3 / 1_000_000 },
  'claude-sonnet-4-20250514': { input: 3.0 / 1_000_000, output: 15.0 / 1_000_000 },
  'claude-opus-4-20250514': { input: 15.0 / 1_000_000, output: 75.0 / 1_000_000 },
}

function estimateCost(model: string, inputTokens: number, outputTokens: number): number {
  const pricing = PRICING[model] ?? PRICING['gpt-5']
  return (inputTokens * pricing.input) + (outputTokens * pricing.output)
}
