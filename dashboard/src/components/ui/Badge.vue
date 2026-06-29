<script setup lang="ts">
defineProps<{
  variant?: 'cyan' | 'green' | 'amber' | 'red' | 'gray'
  size?: 'sm' | 'md'
  pulse?: boolean
}>()
</script>

<template>
  <span
    class="inline-flex items-center gap-1.5 font-mono font-medium rounded-full"
    :class="[
      size === 'sm' ? 'text-[9px] px-2 py-0.5 tracking-wider' : 'text-[10px] px-2.5 py-1 tracking-wide',
      variant === 'cyan' ? 'bg-[var(--cyan-ghost)] text-[var(--cyan)] border border-[rgba(0,229,255,0.12)]' :
      variant === 'green' ? 'bg-[var(--emerald-glow)] text-[var(--emerald)] border border-[rgba(16,185,129,0.12)]' :
      variant === 'amber' ? 'bg-[var(--amber-glow)] text-[var(--amber)] border border-[rgba(245,158,11,0.12)]' :
      variant === 'red' ? 'bg-[var(--crimson-glow)] text-[var(--crimson)] border border-[rgba(239,68,68,0.12)]' :
      'bg-white/[0.03] text-[var(--text-secondary)] border border-[var(--border-subtle)]',
    ]"
  >
    <span
      v-if="pulse"
      class="w-1.5 h-1.5 rounded-full"
      :class="{
        'bg-[var(--cyan)] pulse-cyan': variant === 'cyan',
        'bg-[var(--emerald)] pulse-emerald': variant === 'green',
        'bg-[var(--amber)] pulse-amber': variant === 'amber',
        'bg-[var(--crimson)] pulse-crimson': variant === 'red',
      }"
    />
    <slot />
  </span>
</template>
