<script setup lang="ts">
const props = withDefaults(defineProps<{
  variant?: 'cyan' | 'ghost' | 'danger'
  size?: 'sm' | 'md' | 'lg'
  disabled?: boolean
  block?: boolean
}>(), {
  variant: 'ghost',
  size: 'md',
  block: false,
})
</script>

<template>
  <button
    class="btn"
    :class="{
      'btn-primary': variant === 'cyan',
      'btn-danger': variant === 'danger',
      'btn-secondary': variant === 'ghost',
      'btn-sm': size === 'sm',
      'btn-block': block,
    }"
    :disabled="disabled"
  >
    <slot />
  </button>
</template>
