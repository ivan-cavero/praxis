<script setup lang="ts">
defineProps<{
  modelValue?: string
  placeholder?: string
  type?: string
  label?: string
}>()

defineEmits(['update:modelValue'])
</script>

<template>
  <div class="space-y-1.5">
    <label v-if="label" class="data-label">{{ label }}</label>
    <input
      :type="type || 'text'"
      :value="modelValue"
      @input="$emit('update:modelValue', ($event.target as HTMLInputElement).value)"
      :placeholder="placeholder"
      class="w-full bg-[var(--surface)] border border-[var(--border-subtle)] rounded-[var(--radius-md)] px-4 py-2.5 text-xs font-mono text-[var(--text-primary)] placeholder-[var(--text-muted)]
             focus:outline-none transition-all duration-200"
    />
  </div>
</template>
