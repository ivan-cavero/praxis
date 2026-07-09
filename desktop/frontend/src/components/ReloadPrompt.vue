<script setup lang="ts">
/**
 * ReloadPrompt — PWA update and offline-ready notification toast.
 *
 * Shows a toast when:
 * - The app is ready to work offline
 * - New content is available and a reload is needed
 *
 * Uses the virtual:pwa-register/vue module from vite-plugin-pwa.
 */
import { useRegisterSW } from 'virtual:pwa-register/vue'
import Icon from './ui/Icon.vue'

const {
  offlineReady,
  needRefresh,
  updateServiceWorker,
} = useRegisterSW()

function close() {
  offlineReady.value = false
  needRefresh.value = false
}
</script>

<template>
  <div
    v-if="offlineReady || needRefresh"
    class="pwa-toast"
    role="alert"
  >
    <div class="pwa-message">
      <Icon
        v-if="offlineReady"
        name="check"
        :size="16"
      />
      <Icon
        v-else
        name="refresh"
        :size="16"
      />
      <span v-if="offlineReady">App ready to work offline</span>
      <span v-else>New content available — reload to update.</span>
    </div>
    <div class="pwa-actions">
      <button
        v-if="needRefresh"
        class="pwa-btn pwa-btn-primary"
        @click="updateServiceWorker()"
      >
        Reload
      </button>
      <button
        class="pwa-btn"
        @click="close"
      >
        Close
      </button>
    </div>
  </div>
</template>

<style scoped>
.pwa-toast {
  position: fixed;
  right: var(--space-4);
  bottom: var(--space-4);
  z-index: 9000;
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
  padding: var(--space-3) var(--space-4);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-lg);
  background: var(--bg-surface);
  box-shadow: var(--shadow-lg);
  animation: pwaSlideIn 0.2s ease-out;
}

@keyframes pwaSlideIn {
  from { opacity: 0; transform: translateY(8px); }
  to { opacity: 1; transform: translateY(0); }
}

.pwa-message {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  font-size: 13px;
  color: var(--text-primary);
}

.pwa-actions {
  display: flex;
  gap: var(--space-2);
  justify-content: flex-end;
}

.pwa-btn {
  padding: 4px 12px;
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  background: var(--bg-elevated);
  color: var(--text-secondary);
  font-size: 12px;
  cursor: pointer;
}
.pwa-btn:hover { color: var(--text-primary); }

.pwa-btn-primary {
  border-color: var(--primary);
  background: var(--primary-muted);
  color: var(--primary);
}
.pwa-btn-primary:hover { background: var(--primary-glow); }
</style>
