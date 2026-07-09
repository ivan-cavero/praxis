<script setup lang="ts">
/**
 * EmptyState — contextual empty-state illustration with optional CTA.
 */
import Icon from './Icon.vue'

const {
  icon = 'inbox',
  title = 'Nothing here yet',
  description = '',
  actionLabel = '',
  onAction,
} = defineProps<{
  icon?: string
  title?: string
  description?: string
  actionLabel?: string
  onAction?: () => void
}>()
</script>

<template>
  <div class="empty-state">
    <!-- SVG Illustration -->
    <div class="empty-illustration">
      <svg viewBox="0 0 200 140" fill="none" xmlns="http://www.w3.org/2000/svg" class="empty-svg">
        <!-- Background circle -->
        <circle cx="100" cy="70" r="55" fill="var(--bg-elevated)" opacity="0.5" />
        <!-- Inner dashed circle -->
        <circle cx="100" cy="70" r="45" stroke="var(--border-subtle)" stroke-width="1.5" stroke-dasharray="4 4" />
        <!-- Center icon area -->
        <g class="empty-icon-group">
          <Icon :name="icon" :size="32" class="empty-icon-svg" />
        </g>
        <!-- Floating dots decoration -->
        <circle cx="40" cy="30" r="3" fill="var(--text-disabled)" opacity="0.3" />
        <circle cx="160" cy="25" r="2" fill="var(--text-disabled)" opacity="0.2" />
        <circle cx="170" cy="110" r="2.5" fill="var(--text-disabled)" opacity="0.25" />
        <circle cx="35" cy="115" r="2" fill="var(--text-disabled)" opacity="0.2" />
      </svg>
    </div>

    <!-- Text content -->
    <div class="empty-text">
      <h3 class="empty-title">{{ title }}</h3>
      <p v-if="description" class="empty-description">{{ description }}</p>
    </div>

    <!-- CTA button -->
    <div v-if="actionLabel" class="empty-actions">
      <button class="btn btn-primary" @click="onAction?.()">
        <Icon :name="icon === 'inbox' ? 'plus' : 'arrow-right'" :size="14" />
        {{ actionLabel }}
      </button>
    </div>
  </div>
</template>

<style scoped>
.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  padding: var(--space-12) var(--space-8);
  gap: var(--space-4);
  text-align: center;
}

.empty-illustration {
  width: 160px;
  height: 112px;
  margin-bottom: var(--space-2);
}

.empty-svg {
  width: 100%;
  height: 100%;
}

.empty-icon-group {
  transform: translateY(2px);
}

.empty-icon-svg {
  color: var(--text-disabled);
  opacity: 0.5;
}

.empty-text {
  display: flex;
  flex-direction: column;
  align-items: center;
  gap: var(--space-2);
}

.empty-title {
  font-size: 15px;
  font-weight: 600;
  color: var(--text-secondary);
  letter-spacing: -0.01em;
}

.empty-description {
  font-size: 13px;
  color: var(--text-muted);
  max-width: 280px;
  line-height: 1.5;
}

.empty-actions {
  margin-top: var(--space-2);
}

.btn {
  all: unset;
  display: inline-flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-4);
  border-radius: var(--radius-md);
  font-size: 13px;
  font-weight: 500;
  font-family: inherit;
  cursor: pointer;
  border: 1px solid transparent;
  transition: all var(--transition-fast);
  white-space: nowrap;
  line-height: 1.4;
}

.btn-primary {
  background: var(--primary);
  color: var(--bg-base);
  border: none;
}

.btn-primary:hover {
  background: var(--primary-hover);
  box-shadow: 0 4px 12px var(--primary-glow);
}
</style>
