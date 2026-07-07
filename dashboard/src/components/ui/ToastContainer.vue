<script setup lang="ts">
/**
 * ToastContainer — Renders global toast notifications.
 *
 * Mount once in App.vue. Reads from useToast() composable.
 */
import { useToast, type ToastItem } from '../../composables/useToast'
import Icon from './Icon.vue'

const toast = useToast()

function iconFor(kind: ToastItem['kind']): string {
  switch (kind) {
    case 'success': return 'check'
    case 'error': return 'alert-circle'
    case 'warning': return 'alert-triangle'
    case 'info': return 'info'
    default: return 'info'
  }
}
</script>

<template>
  <div class="toast-container" v-if="toast.toasts.value.length > 0">
    <TransitionGroup name="toast">
      <div
        v-for="item in toast.toasts.value"
        :key="item.id"
        class="toast-item"
        :class="item.kind"
        @click="item.dismissible && toast.dismiss(item.id)"
      >
        <Icon :name="iconFor(item.kind)" :size="16" class="toast-icon" />
        <span class="toast-message">{{ item.message }}</span>
        <button
          v-if="item.dismissible"
          class="toast-close"
          aria-label="Dismiss"
          @click.stop="toast.dismiss(item.id)"
        >
          <Icon name="x" :size="12" />
        </button>
      </div>
    </TransitionGroup>
  </div>
</template>

<style scoped>
.toast-container {
  position: fixed;
  bottom: 20px;
  right: 20px;
  z-index: 9999;
  display: flex;
  flex-direction: column;
  gap: 8px;
  max-width: 380px;
  pointer-events: none;
}

.toast-item {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 12px 16px;
  border-radius: 10px;
  font-size: 13px;
  font-weight: 500;
  cursor: pointer;
  pointer-events: auto;
  backdrop-filter: blur(12px);
  box-shadow: 0 4px 24px rgba(0, 0, 0, 0.3);
  border: 1px solid;
  transition: opacity 0.2s, transform 0.2s;
}

.toast-item:hover {
  opacity: 0.9;
}

.toast-success {
  background: rgba(34, 197, 94, 0.15);
  color: #4ade80;
  border-color: rgba(34, 197, 94, 0.3);
}

.toast-error {
  background: rgba(239, 68, 68, 0.15);
  color: #f87171;
  border-color: rgba(239, 68, 68, 0.3);
}

.toast-warning {
  background: rgba(251, 191, 36, 0.15);
  color: #fbbf24;
  border-color: rgba(251, 191, 36, 0.3);
}

.toast-info {
  background: rgba(59, 130, 246, 0.15);
  color: #60a5fa;
  border-color: rgba(59, 130, 246, 0.3);
}

.toast-icon {
  flex-shrink: 0;
}

.toast-message {
  flex: 1;
  line-height: 1.4;
}

.toast-close {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 20px;
  height: 20px;
  border: none;
  border-radius: 6px;
  background: transparent;
  color: inherit;
  cursor: pointer;
  opacity: 0.5;
  transition: opacity 0.15s;
  flex-shrink: 0;
}

.toast-close:hover {
  opacity: 1;
}

/* Transition animations */
.toast-enter-active {
  transition: all 0.3s ease-out;
}

.toast-leave-active {
  transition: all 0.2s ease-in;
}

.toast-enter-from {
  opacity: 0;
  transform: translateX(40px);
}

.toast-leave-to {
  opacity: 0;
  transform: translateX(40px);
}

.toast-move {
  transition: transform 0.2s ease;
}
</style>
