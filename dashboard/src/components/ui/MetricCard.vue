<script setup lang="ts">
defineProps<{
  label: string
  value: string | number
  icon?: string
  accent?: 'cyan' | 'amber' | 'crimson' | 'emerald'
  trend?: 'up' | 'down' | 'stable'
  trendValue?: string
}>()
</script>

<template>
  <div class="card card-glow p-4 relative overflow-hidden group">
    <!-- Background gradient accent -->
    <div
      class="absolute -top-12 -right-12 w-24 h-24 rounded-full opacity-[0.04] group-hover:opacity-[0.08] transition-opacity duration-500"
      :class="{
        'bg-[var(--cyan)]': accent === 'cyan' || !accent,
        'bg-[var(--amber)]': accent === 'amber',
        'bg-[var(--crimson)]': accent === 'crimson',
        'bg-[var(--emerald)]': accent === 'emerald',
      }"
    />

    <div class="relative z-10">
      <div class="flex items-center justify-between mb-3">
        <span class="data-label">{{ label }}</span>
        <span v-if="icon" class="text-base opacity-60">{{ icon }}</span>
      </div>

      <div class="flex items-end gap-2">
        <span
          class="text-2xl font-bold font-mono tracking-tight"
          :class="{
            'text-[var(--cyan)]': accent === 'cyan' || !accent,
            'text-[var(--amber)]': accent === 'amber',
            'text-[var(--crimson)]': accent === 'crimson',
            'text-[var(--emerald)]': accent === 'emerald',
          }"
        >{{ value }}</span>

        <span
          v-if="trend"
          class="text-[10px] font-mono mb-1"
          :class="{
            'text-[var(--emerald)]': trend === 'up',
            'text-[var(--crimson)]': trend === 'down',
            'text-[var(--text-muted)]': trend === 'stable',
          }"
        >
          {{ trend === 'up' ? '↑' : trend === 'down' ? '↓' : '→' }}
          {{ trendValue }}
        </span>
      </div>
    </div>
  </div>
</template>
