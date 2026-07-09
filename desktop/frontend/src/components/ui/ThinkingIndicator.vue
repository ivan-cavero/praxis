<script setup lang="ts">
/**
 * ThinkingIndicator — shows an agent is "thinking" before its first output.
 *
 * Displays animated dots + the agent name + elapsed time.
 * Used in the chat stream between AgentStarted and the first AgentOutput delta.
 */
import { ref, onMounted, onUnmounted } from 'vue'

const { agent = 'Agent' } = defineProps<{
  agent?: string
}>()

const elapsed = ref(0)
let timer: ReturnType<typeof setInterval> | null = null

onMounted(() => {
  const start = Date.now()
  timer = setInterval(() => {
    elapsed.value = Math.floor((Date.now() - start) / 1000)
  }, 1000)
})

onUnmounted(() => {
  if (timer) clearInterval(timer)
})
</script>

<template>
  <div class="thinking-indicator animate-slide-up">
    <div class="thinking-avatar">{{ agent.charAt(0).toUpperCase() }}</div>
    <div class="thinking-body">
      <div class="thinking-header">
        <span class="thinking-name">{{ agent }}</span>
        <span class="thinking-status">thinking</span>
        <span v-if="elapsed > 0" class="thinking-time">{{ elapsed }}s</span>
      </div>
      <div class="thinking-dots">
        <span class="dot" />
        <span class="dot" />
        <span class="dot" />
      </div>
    </div>
  </div>
</template>

<style scoped>
.thinking-indicator {
  display: flex;
  gap: var(--space-3);
  padding: var(--space-2) 0;
}

.thinking-avatar {
  width: 28px;
  height: 28px;
  border-radius: var(--radius-full);
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 11px;
  font-weight: 700;
  flex-shrink: 0;
  margin-top: 2px;
  background: var(--bg-elevated);
  color: var(--text-muted);
  border: 1px solid var(--border-subtle);
}

.thinking-body {
  flex: 1;
  min-width: 0;
}

.thinking-header {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  margin-bottom: var(--space-1);
}

.thinking-name {
  font-size: 13px;
  font-weight: 600;
  color: var(--text-secondary);
}

.thinking-status {
  font-size: 11px;
  font-weight: 500;
  color: var(--text-muted);
  font-style: italic;
  text-transform: lowercase;
}

.thinking-time {
  font-size: 11px;
  font-family: var(--font-mono);
  color: var(--text-disabled);
}

.thinking-dots {
  display: flex;
  gap: 4px;
  padding: var(--space-1) 0;
}

.dot {
  width: 6px;
  height: 6px;
  border-radius: 50%;
  background: var(--text-muted);
  animation: thinkingBounce 1.4s infinite ease-in-out both;
}

.dot:nth-child(1) { animation-delay: -0.32s; }
.dot:nth-child(2) { animation-delay: -0.16s; }
.dot:nth-child(3) { animation-delay: 0s; }

@keyframes thinkingBounce {
  0%, 80%, 100% { transform: scale(0.6); opacity: 0.4; }
  40% { transform: scale(1); opacity: 1; }
}
</style>
