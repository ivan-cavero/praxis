<script setup lang="ts">
defineProps<{
  label: string
  value: string | number
  sub?: string
  color?: 'green' | 'emerald' | 'amber' | 'crimson' | 'blue' | 'purple'
}>()
</script>

<template>
  <div class="metric-card">
    <div class="metric-label">{{ label }}</div>
    <div class="metric-value" :class="color || 'green'">{{ value }}</div>
    <div v-if="sub" class="metric-sub">{{ sub }}</div>
  </div>
</template>

<style scoped>
.metric-card {
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  padding: var(--space-4);
  transition: border-color var(--transition-fast);
}
.metric-card:hover {
  border-color: var(--border-default);
}
.metric-label {
  font-size: 12px;
  font-weight: 500;
  color: var(--text-muted);
  margin-bottom: var(--space-2);
}
.metric-value {
  font-size: 28px;
  font-weight: 700;
  letter-spacing: -0.02em;
  line-height: 1.1;
  margin-bottom: var(--space-1);
}
.metric-value.green { color: #22c55e; }
.metric-value.emerald { color: #10b981; }
.metric-value.amber { color: #f59e0b; }
.metric-value.crimson { color: #ef4444; }
.metric-value.blue { color: #3b82f6; }
.metric-value.purple { color: #a855f7; }
.metric-sub {
  font-size: 12px;
  color: var(--text-muted);
}
</style>
