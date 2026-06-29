<script setup lang="ts">
defineProps<{
  variant?: 'cyan' | 'ghost' | 'danger'
  size?: 'sm' | 'md' | 'lg'
  disabled?: boolean
}>()
</script>

<template>
  <button
    class="btn inline-flex items-center justify-center gap-2"
    :class="[
      variant === 'cyan' ? 'btn-cyan' :
      variant === 'danger' ? 'btn-danger' :
      'btn-ghost',
      size === 'lg' ? 'px-6 py-3 text-sm' :
      size === 'sm' ? 'px-3 py-1.5 text-[11px]' :
      'px-4 py-2 text-xs',
      disabled ? 'opacity-40 cursor-not-allowed pointer-events-none' : '',
    ]"
    :disabled="disabled"
  >
    <slot />
  </button>
</template>
