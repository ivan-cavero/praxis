<script setup lang="ts">
defineProps<{
  title?: string
  subtitle?: string
  padding?: boolean
  glow?: 'cyan' | 'amber' | 'crimson' | null
  hover?: boolean
}>()
</script>

<template>
  <div
    class="card relative overflow-hidden"
    :class="[
      padding !== false ? 'p-5' : '',
      glow ? `glow-${glow}` : '',
      hover ? 'card-glow' : '',
    ]"
  >
    <!-- Accent line at top -->
    <div
      v-if="glow"
      class="absolute top-0 left-0 h-[1px] w-16"
      :class="{
        'bg-gradient-to-r from-[var(--cyan)] to-transparent': glow === 'cyan',
        'bg-gradient-to-r from-[var(--amber)] to-transparent': glow === 'amber',
        'bg-gradient-to-r from-[var(--crimson)] to-transparent': glow === 'crimson',
      }"
    />

    <div v-if="title" class="flex items-baseline gap-3 mb-4">
      <span class="data-label">{{ title }}</span>
      <span v-if="subtitle" class="text-[10px] text-[var(--text-ghost)] font-mono">{{ subtitle }}</span>
    </div>
    <slot />
  </div>
</template>
