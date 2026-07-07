<script setup lang="ts">
/**
 * MetricCard — compact stat card with optional icon and accent color.
 */
import Icon from './Icon.vue'
const { label, value, sub, color = 'green', icon } = defineProps<{
  label: string
  value: string | number
  sub?: string
  color?: 'green' | 'emerald' | 'amber' | 'crimson' | 'blue' | 'purple'
  icon?: string
}>()
</script>

<template>
  <div class="metric-card animate-slide-up">
    <div class="metric-top">
      <div class="metric-label">{{ label }}</div>
      <div v-if="icon" class="metric-icon" :class="color">
        <Icon :name="icon" :size="16" />
      </div>
    </div>
    <div class="metric-value" :class="color">{{ value }}</div>
    <div v-if="sub" class="metric-sub">{{ sub }}</div>
  </div>
</template>

<style scoped>
.metric-card {
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  padding: var(--space-4) var(--space-5);
  transition: all var(--transition-normal);
  display: flex;
  flex-direction: column;
  gap: var(--space-1);
}

.metric-card:hover {
  border-color: var(--border-default);
  transform: translateY(-2px);
  box-shadow: var(--shadow-md);
}

.metric-top {
  display: flex;
  align-items: center;
  justify-content: space-between;
}

.metric-label {
  font-size: 12px;
  font-weight: 500;
  color: var(--text-muted);
}

.metric-icon {
  width: 28px;
  height: 28px;
  border-radius: var(--radius-md);
  display: flex;
  align-items: center;
  justify-content: center;
  opacity: 0.6;
}

.metric-icon.green { color: #22c55e; background: rgba(34, 197, 94, 0.1); }
.metric-icon.emerald { color: #10b981; background: rgba(16, 185, 129, 0.1); }
.metric-icon.amber { color: #f59e0b; background: rgba(245, 158, 11, 0.1); }
.metric-icon.crimson { color: #ef4444; background: rgba(239, 68, 68, 0.1); }
.metric-icon.blue { color: #3b82f6; background: rgba(59, 130, 246, 0.1); }
.metric-icon.purple { color: #a855f7; background: rgba(168, 85, 247, 0.1); }

.metric-value {
  font-size: 28px;
  font-weight: 700;
  letter-spacing: -0.02em;
  line-height: 1.1;
  margin-top: var(--space-1);
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
