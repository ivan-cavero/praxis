<script setup lang="ts">
import { ref } from 'vue'
import Icon from '../ui/Icon.vue'

const pressure = ref(0)
const budgetTotal = ref(100000)
const totalCompressions = ref(0)
const avgContextSize = ref(4096)

const budgetPercent = ref(0)

function getPressureColor(p: number): string {
  if (p < 0.3) return 'var(--primary)'
  if (p < 0.7) return 'var(--warning)'
  return 'var(--error)'
}
</script>

<template>
  <div class="context-panel">
    <div class="panel-header">
      <h3 class="panel-title">Context Pressure</h3>
    </div>

    <div class="pressure-gauge">
      <svg viewBox="0 0 120 60" class="gauge-svg">
        <path
          d="M 10 55 A 50 50 0 0 1 110 55"
          fill="none"
          stroke="var(--border-subtle)"
          stroke-width="8"
          stroke-linecap="round"
        />
        <path
          d="M 10 55 A 50 50 0 0 1 110 55"
          fill="none"
          :stroke="getPressureColor(pressure)"
          stroke-width="8"
          stroke-linecap="round"
          :stroke-dasharray="`${pressure * 157} 157`"
          class="gauge-fill"
        />
      </svg>
      <div class="gauge-center">
        <span class="gauge-value">{{ Math.round(pressure * 100) }}%</span>
        <span class="gauge-label">Pressure</span>
      </div>
    </div>

    <div class="panel-stats">
      <div class="stat-row">
        <span class="stat-label">Budget Used</span>
        <span class="stat-value">0 / {{ (budgetTotal / 1000).toFixed(0) }}K</span>
      </div>
      <div class="stat-bar">
        <div class="stat-bar-fill" :style="{ width: `${budgetPercent}%` }" />
      </div>

      <div class="stat-row">
        <span class="stat-label">Compressions</span>
        <span class="stat-value">{{ totalCompressions }}</span>
      </div>

      <div class="stat-row">
        <span class="stat-label">Avg Context</span>
        <span class="stat-value">{{ (avgContextSize / 1024).toFixed(1) }}K tokens</span>
      </div>
    </div>

    <div class="panel-empty" v-if="pressure === 0">
      <Icon name="database" :size="20" class="empty-icon" />
      <span>No active session to monitor</span>
    </div>
  </div>
</template>

<style scoped>
.context-panel {
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  overflow: hidden;
}

.panel-header {
  padding: var(--space-4);
  border-bottom: 1px solid var(--border-subtle);
}

.panel-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
}

.pressure-gauge {
  position: relative;
  padding: var(--space-4);
  display: flex;
  justify-content: center;
}

.gauge-svg {
  width: 160px;
  height: 80px;
}

.gauge-fill {
  transition: stroke-dasharray 0.5s ease;
}

.gauge-center {
  position: absolute;
  bottom: var(--space-6);
  display: flex;
  flex-direction: column;
  align-items: center;
}

.gauge-value {
  font-size: 24px;
  font-weight: 700;
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
}

.gauge-label {
  font-size: 10px;
  text-transform: uppercase;
  letter-spacing: 0.08em;
  color: var(--text-muted);
}

.panel-stats {
  padding: 0 var(--space-4) var(--space-4);
  display: flex;
  flex-direction: column;
  gap: var(--space-3);
}

.stat-row {
  display: flex;
  justify-content: space-between;
  font-size: 12px;
}

.stat-label {
  color: var(--text-muted);
}

.stat-value {
  color: var(--text-primary);
  font-family: var(--font-mono);
  font-size: 11px;
  font-variant-numeric: tabular-nums;
}

.stat-bar {
  height: 4px;
  background: var(--border-subtle);
  border-radius: 2px;
  overflow: hidden;
  margin-top: -2px;
}

.stat-bar-fill {
  height: 100%;
  background: var(--primary);
  border-radius: 2px;
  transition: width 0.3s ease;
}

.panel-empty {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  padding: var(--space-8);
  color: var(--text-muted);
  font-size: 12px;
}

.empty-icon {
  opacity: 0.4;
}
</style>
