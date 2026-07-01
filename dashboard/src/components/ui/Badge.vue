<script setup lang="ts">
const props = withDefaults(defineProps<{
  variant?: 'cyan' | 'green' | 'amber' | 'red' | 'gray'
  size?: 'sm' | 'md'
  pulse?: boolean
}>(), {
  variant: 'gray',
  size: 'md',
})
</script>

<template>
  <span
    class="badge"
    :class="{
      'badge-cyan': variant === 'cyan',
      'badge-emerald': variant === 'green',
      'badge-amber': variant === 'amber',
      'badge-crimson': variant === 'red',
      'badge-gray': variant === 'gray',
    }"
  >
    <span
      v-if="pulse"
      class="pulse-dot"
      :class="{
        'pulse-cyan': variant === 'cyan',
        'pulse-emerald': variant === 'green',
        'pulse-amber': variant === 'amber',
      }"
    />
    <slot />
  </span>
</template>
