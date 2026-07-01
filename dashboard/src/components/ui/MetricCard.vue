<script setup lang="ts">
defineProps<{
  label: string
  value: string | number
  icon?: string
  accent?: 'cyan' | 'amber' | 'crimson' | 'emerald'
  trend?: 'up' | 'down' | 'stable'
  trendValue?: string
}>()

const accentColor = (a?: string) => {
  switch (a) {
    case 'amber': return 'var(--clr-amber)'
    case 'crimson': return 'var(--clr-crimson)'
    case 'emerald': return 'var(--clr-emerald)'
    default: return 'var(--clr-primary)'
  }
}
</script>

<template>
  <div class="metric-card component">
    <div
      class="metric-card-glow"
      :style="{ background: accentColor(accent) }"
    />

    <div class="metric-card-content">
      <div class="metric-card-header">
        <span class="data-label">{{ label }}</span>
        <span v-if="icon" class="metric-icon">{{ icon }}</span>
      </div>

      <div class="metric-card-value-row">
        <span
          class="metric-value"
          :style="{ color: accentColor(accent) }"
        >{{ value }}</span>

        <span
          v-if="trend"
          class="metric-trend"
          :style="{ color: trend === 'up' ? 'var(--clr-emerald)' : trend === 'down' ? 'var(--clr-crimson)' : 'var(--clr-text-muted)' }"
        >
          {{ trend === 'up' ? '↑' : trend === 'down' ? '↓' : '→' }}
          {{ trendValue }}
        </span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.component {
  position: relative;
  overflow: hidden;
}

.metric-card {
  background: var(--clr-surface);
  border: 1px solid var(--clr-border);
  border-radius: var(--radius-lg);
}

.metric-card-glow {
  position: absolute;
  top: -48px;
  right: -48px;
  width: 96px;
  height: 96px;
  border-radius: 50%;
  opacity: 0.04;
  transition: opacity 0.5s;
  pointer-events: none;
}

.component:hover .metric-card-glow {
  opacity: 0.08;
}

.metric-card-content {
  position: relative;
  z-index: 10;
  padding: var(--space-md);
}

.metric-card-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--space-md);
}

.metric-icon {
  font-size: 16px;
  opacity: 0.6;
}

.metric-card-value-row {
  display: flex;
  align-items: flex-end;
  gap: var(--space-sm);
}

.metric-value {
  font-size: 20px;
  font-weight: 700;
  font-family: var(--font-mono);
  letter-spacing: -0.02em;
}

.metric-trend {
  font-size: 10px;
  font-family: var(--font-mono);
  padding: 4px 0;
}
</style>
