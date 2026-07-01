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
  <div class="input-group">
    <label v-if="label" class="data-label">{{ label }}</label>
    <input
      :type="type || 'text'"
      :value="modelValue"
      @input="$emit('update:modelValue', ($event.target as HTMLInputElement).value)"
      :placeholder="placeholder"
      class="input"
    />
  </div>
</template>
