<script setup lang="ts">
import { ref, computed, onMounted, onUnmounted } from 'vue'
import { useApi } from '../../composables/useApi'

const api = useApi()

interface TokenDataPoint {
  time: string
  input: number
  output: number
}

const history = ref<TokenDataPoint[]>([])
const totalInput = ref(0)
const totalOutput = ref(0)
const maxTokens = ref(1)

let pollingInterval: ReturnType<typeof setInterval> | null = null

const canvasData = computed(() => {
  const points = history.value
  if (points.length < 2) return null

  const max = Math.max(maxTokens.value, 1)
  return points.map(p => ({
    inputRatio: p.input / max,
    outputRatio: p.output / max,
    label: p.time.slice(11, 16),
  }))
})

async function pollTokens() {
  try {
    const metrics = await api.getMetricsSummary()
    totalInput.value = metrics.total_tokens
    totalOutput.value = metrics.total_tokens

    const now = new Date().toISOString()
    const last = history.value[history.value.length - 1]
    if (!last || last.time.slice(0, 5) !== now.slice(14, 19)) {
      history.value = [
        ...history.value.slice(-59),
        { time: now, input: totalInput.value, output: totalOutput.value },
      ]
      if (history.value.length > 1) {
        const latest = history.value[history.value.length - 1]
        const prev = history.value[history.value.length - 2]
        maxTokens.value = Math.max(
          maxTokens.value,
          Math.abs(latest.input - prev.input),
          Math.abs(latest.output - prev.output),
          1
        )
      }
    }
  } catch {
    // silent
  }
}

onMounted(() => {
  pollTokens()
  pollingInterval = setInterval(pollTokens, 5000)
})

onUnmounted(() => {
  if (pollingInterval) clearInterval(pollingInterval)
})
</script>

<template>
  <div class="token-chart">
    <div class="chart-header">
      <h3 class="chart-title">Token Usage</h3>
      <div class="chart-legend">
        <span class="legend-item">
          <span class="legend-dot input" />
          Input
        </span>
        <span class="legend-item">
          <span class="legend-dot output" />
          Output
        </span>
      </div>
    </div>

    <div class="chart-values">
      <div class="chart-value">
        <span class="chart-value-number">{{ (totalInput / 1000).toFixed(0) }}K</span>
        <span class="chart-value-label">Total input</span>
      </div>
      <div class="chart-value">
        <span class="chart-value-number">{{ (totalOutput / 1000).toFixed(0) }}K</span>
        <span class="chart-value-label">Total output</span>
      </div>
    </div>

    <!-- Simple bar visualization -->
    <div class="chart-bars" v-if="canvasData && canvasData.length > 1">
      <div
        v-for="(point, i) in canvasData.slice(-30)"
        :key="i"
        class="chart-bar-group"
      >
        <div
          class="chart-bar chart-bar-input"
          :style="{ height: `${point.inputRatio * 100}%` }"
        />
        <div
          class="chart-bar chart-bar-output"
          :style="{ height: `${point.outputRatio * 100}%` }"
        />
      </div>
    </div>

    <div v-else class="chart-empty">
      Collecting data...
    </div>
  </div>
</template>

<style scoped>
.token-chart {
  background: var(--bg-surface);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-lg);
  padding: var(--space-4);
}

.chart-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  margin-bottom: var(--space-4);
}

.chart-title {
  font-size: 14px;
  font-weight: 600;
  color: var(--text-primary);
}

.chart-legend {
  display: flex;
  gap: var(--space-3);
}

.legend-item {
  display: flex;
  align-items: center;
  gap: var(--space-1);
  font-size: 11px;
  color: var(--text-muted);
}

.legend-dot {
  width: 8px;
  height: 8px;
  border-radius: 2px;
}

.legend-dot.input {
  background: var(--primary);
}

.legend-dot.output {
  background: var(--info);
}

.chart-values {
  display: flex;
  gap: var(--space-6);
  margin-bottom: var(--space-4);
}

.chart-value {
  display: flex;
  flex-direction: column;
}

.chart-value-number {
  font-size: 20px;
  font-weight: 600;
  color: var(--text-primary);
  font-variant-numeric: tabular-nums;
}

.chart-value-label {
  font-size: 11px;
  color: var(--text-muted);
}

.chart-bars {
  display: flex;
  align-items: flex-end;
  gap: 2px;
  height: 80px;
}

.chart-bar-group {
  flex: 1;
  display: flex;
  gap: 1px;
  align-items: flex-end;
  height: 100%;
}

.chart-bar {
  flex: 1;
  min-height: 2px;
  border-radius: 2px 2px 0 0;
  transition: height 0.5s ease;
}

.chart-bar-input {
  background: var(--primary);
  opacity: 0.8;
}

.chart-bar-output {
  background: var(--info);
  opacity: 0.6;
}

.chart-empty {
  height: 80px;
  display: flex;
  align-items: center;
  justify-content: center;
  font-size: 12px;
  color: var(--text-muted);
}
</style>
