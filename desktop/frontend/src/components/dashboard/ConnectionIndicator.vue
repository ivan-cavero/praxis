<script setup lang="ts">
/**
 * ConnectionIndicator — Shows current connection mode (local/remote) in the header.
 */
import { ref, onMounted, onUnmounted } from 'vue'
import { useRemoteStatus } from '../../composables/useApi'
import { useConnection } from '../../composables/useConnection'
import Icon from '../ui/Icon.vue'

const { connections, currentConnection, setCurrentConnection } = useConnection()
const { isRemoteMode, remoteHost, remotePort } = useRemoteStatus()
const showDropdown = ref(false)

function toggleDropdown() {
  showDropdown.value = !showDropdown.value
}

function handleSelect(id: string | null) {
  setCurrentConnection(id)
  showDropdown.value = false
}

function handleClickOutside(event: MouseEvent) {
  const target = event.target as HTMLElement
  if (!target.closest('.connection-indicator')) {
    showDropdown.value = false
  }
}

function handleKeydown(event: KeyboardEvent) {
  if (event.key === 'Escape') {
    showDropdown.value = false
  }
}

onMounted(() => {
  document.addEventListener('click', handleClickOutside)
  document.addEventListener('keydown', handleKeydown)
})

onUnmounted(() => {
  document.removeEventListener('click', handleClickOutside)
  document.removeEventListener('keydown', handleKeydown)
})
</script>

<template>
  <div class="connection-indicator">
    <button
      class="indicator-btn"
      :class="{ remote: isRemoteMode }"
      @click="toggleDropdown"
      :title="isRemoteMode ? `Remote: ${remoteHost}:${remotePort}` : 'Local mode'"
    >
      <span class="status-dot" :class="{ connected: isRemoteMode }" />
      <span class="status-label">
        {{ isRemoteMode ? `${remoteHost}:${remotePort}` : 'Local' }}
      </span>
      <Icon name="chevron-down" :size="12" class="chevron" />
    </button>

    <div v-if="showDropdown" class="dropdown">
      <button
        class="dropdown-item"
        :class="{ active: !isRemoteMode }"
        @click="handleSelect(null)"
      >
        <span class="status-dot" />
        <span>Local</span>
      </button>

      <div v-if="connections.length > 0" class="dropdown-divider" />

      <button
        v-for="conn in connections"
        :key="conn.id"
        class="dropdown-item"
        :class="{ active: currentConnection?.id === conn.id }"
        @click="handleSelect(conn.id)"
      >
        <span class="status-dot connected" />
        <span class="conn-name">{{ conn.name }}</span>
        <span class="conn-host">{{ conn.host }}:{{ conn.port }}</span>
      </button>

      <div v-if="connections.length === 0" class="dropdown-empty">
        No remote connections
      </div>
    </div>
  </div>
</template>

<style scoped>
.connection-indicator {
  position: relative;
}

.indicator-btn {
  display: flex;
  align-items: center;
  gap: 0.375rem;
  padding: 0.375rem 0.625rem;
  background: var(--bg-surface, #27272a);
  border: 1px solid var(--border-default, #3f3f46);
  border-radius: 0.375rem;
  color: var(--text-secondary, #a1a1aa);
  font-size: 0.8125rem;
  cursor: pointer;
  transition: border-color 0.15s;
}

.indicator-btn:hover {
  border-color: var(--border-hover, #52525b);
}

.indicator-btn.remote {
  border-color: rgba(34, 197, 94, 0.3);
  color: #22c55e;
}

.status-dot {
  width: 7px;
  height: 7px;
  border-radius: 50%;
  background: #52525b;
  flex-shrink: 0;
}

.status-dot.connected {
  background: #22c55e;
  box-shadow: 0 0 6px rgba(34, 197, 94, 0.4);
}

.status-label {
  white-space: nowrap;
  max-width: 120px;
  overflow: hidden;
  text-overflow: ellipsis;
}

.chevron {
  opacity: 0.5;
  transition: transform 0.15s;
}

/* Dropdown */
.dropdown {
  position: absolute;
  top: calc(100% + 4px);
  right: 0;
  min-width: 220px;
  background: var(--bg-elevated, #18181b);
  border: 1px solid var(--border-default, #3f3f46);
  border-radius: 0.5rem;
  box-shadow: 0 8px 24px rgba(0, 0, 0, 0.4);
  z-index: 50;
  overflow: hidden;
}

.dropdown-item {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  width: 100%;
  padding: 0.625rem 0.75rem;
  background: none;
  border: none;
  color: var(--text-primary, #f4f4f5);
  font-size: 0.8125rem;
  cursor: pointer;
  text-align: left;
  transition: background 0.1s;
}

.dropdown-item:hover {
  background: var(--bg-surface, #27272a);
}

.dropdown-item.active {
  background: rgba(34, 197, 94, 0.08);
  color: #22c55e;
}

.conn-name {
  font-weight: 500;
}

.conn-host {
  margin-left: auto;
  color: var(--text-secondary, #a1a1aa);
  font-size: 0.75rem;
}

.dropdown-divider {
  height: 1px;
  background: var(--border-default, #3f3f46);
  margin: 0.25rem 0;
}

.dropdown-empty {
  padding: 0.75rem;
  text-align: center;
  color: var(--text-secondary, #a1a1aa);
  font-size: 0.75rem;
}
</style>