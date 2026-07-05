<script setup lang="ts">
/**
 * NotFoundView — 404 page for invalid routes.
 */
import { useRouter } from 'vue-router'
import Icon from '../components/ui/Icon.vue'

const router = useRouter()
</script>

<template>
  <div class="not-found">
    <div class="not-found-content">
      <div class="not-found-code">404</div>
      <h1 class="not-found-title">Page not found</h1>
      <p class="not-found-desc">
        The page you're looking for doesn't exist or has been moved.
      </p>
      <div class="not-found-actions">
        <button class="btn btn-primary" @click="router.push('/')">
          <Icon name="dashboard" :size="16" />
          Go to Dashboard
        </button>
        <button class="btn btn-ghost" @click="router.back()">
          <Icon name="arrow-left" :size="16" />
          Go Back
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.not-found {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100%;
  width: 100%;
  padding: var(--space-8);
}

.not-found-content {
  text-align: center;
  max-width: 400px;
}

.not-found-code {
  font-size: 72px;
  font-weight: 800;
  color: var(--text-disabled);
  letter-spacing: -0.04em;
  line-height: 1;
}

.not-found-title {
  font-size: 22px;
  font-weight: 600;
  color: var(--text-primary);
  margin-top: var(--space-3);
}

.not-found-desc {
  font-size: 14px;
  color: var(--text-muted);
  margin-top: var(--space-2);
  line-height: 1.6;
}

.not-found-actions {
  display: flex;
  gap: var(--space-3);
  justify-content: center;
  margin-top: var(--space-6);
}

.btn {
  display: inline-flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-4);
  border-radius: var(--radius-md);
  font-size: 13px;
  font-weight: 500;
  font-family: inherit;
  cursor: pointer;
  border: 1px solid transparent;
  transition: all 0.15s;
}

.btn-primary {
  background: var(--primary);
  color: var(--bg-base);
}

.btn-primary:hover {
  background: var(--primary-hover);
}

.btn-ghost {
  background: transparent;
  color: var(--text-secondary);
  border-color: var(--border-subtle);
}

.btn-ghost:hover {
  background: var(--bg-hover);
  color: var(--text-primary);
}
</style>
