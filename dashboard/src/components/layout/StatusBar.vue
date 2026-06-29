<script setup lang="ts">
defineProps<{
  connected: boolean
  version?: string
  uptime?: string
  eventCount?: number
}>()
</script>

<template>
  <div class="px-4 py-3 border-t border-[var(--border-subtle)] space-y-2">
    <!-- Connection Status -->
    <div class="flex items-center gap-2">
      <span
        class="status-dot"
        :class="connected ? 'status-dot-online pulse-emerald' : 'status-dot-error'"
      />
      <span class="text-[10px] font-mono tracking-wider uppercase"
        :class="connected ? 'text-[var(--emerald)]' : 'text-[var(--crimson)]'"
      >
        {{ connected ? 'ONLINE' : 'OFFLINE' }}
      </span>
    </div>

    <!-- Meta -->
    <div class="space-y-1">
      <div class="flex items-center justify-between">
        <span class="text-[9px] font-mono text-[var(--text-ghost)] uppercase tracking-widest">Version</span>
        <span class="text-[10px] font-mono text-[var(--text-muted)]">{{ version || '—' }}</span>
      </div>
      <div class="flex items-center justify-between">
        <span class="text-[9px] font-mono text-[var(--text-ghost)] uppercase tracking-widest">Uptime</span>
        <span class="text-[10px] font-mono text-[var(--text-muted)]">{{ uptime || '—' }}</span>
      </div>
      <div class="flex items-center justify-between">
        <span class="text-[9px] font-mono text-[var(--text-ghost)] uppercase tracking-widest">Events</span>
        <span class="text-[10px] font-mono text-[var(--text-muted)]">{{ eventCount || 0 }}</span>
      </div>
    </div>
  </div>
</template>
