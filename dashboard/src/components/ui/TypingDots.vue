<script setup lang="ts">
/**
 * TypingDots — compact animated dots for inline "typing" indicators.
 * Used inside streaming message bubbles while content is actively arriving.
 */
const { size = 6 } = defineProps<{
  size?: number
}>()
</script>

<template>
  <span class="typing-dots" :style="{ '--dot-size': `${size}px` }">
    <span class="dot" />
    <span class="dot" />
    <span class="dot" />
  </span>
</template>

<style scoped>
.typing-dots {
  display: inline-flex;
  gap: 3px;
  align-items: center;
}

.dot {
  width: var(--dot-size);
  height: var(--dot-size);
  border-radius: 50%;
  background: currentColor;
  animation: typingDot 1.4s infinite ease-in-out both;
}

.dot:nth-child(1) { animation-delay: -0.32s; }
.dot:nth-child(2) { animation-delay: -0.16s; }
.dot:nth-child(3) { animation-delay: 0s; }

@keyframes typingDot {
  0%, 60%, 100% { transform: translateY(0); opacity: 0.4; }
  30% { transform: translateY(-4px); opacity: 1; }
}
</style>
