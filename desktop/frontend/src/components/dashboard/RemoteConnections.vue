<script setup lang="ts">
/**
 * RemoteConnections — Manage remote VPS/device connections via QR pairing.
 *
 * ## Future (Fase B)
 * When praxis.dev exists, this component will also show cloud-synced
 * connections and support OAuth login (Google/GitHub) instead of just
 * QR pairing. The QR will encode a URL pointing to app.praxis.dev/pair
 * instead of the direct server URL.
 */
import { ref, computed, onUnmounted } from 'vue'
import { useConnection } from '../../composables/useConnection'
import Icon from '../ui/Icon.vue'

interface QRCodeModule {
  create: (text: string, options?: { type?: string; errorCorrectionLevel?: string }) => string
}

const {
  connections,
  currentConnectionId,
  isRemoteMode,
  isPairing,
  pairingStatus,
  pairingCode,
  pairingQrUrl,
  pairingExpiresAt,
  pairingError,
  setCurrentConnection,
  switchToLocal,
  startPairing,
  cancelPairing,
  removeConnection,
  testConnection,
  formatLastSeen,
} = useConnection()

// ─── UI State ──────────────────────────────────────────────────

const showAddRemote = ref(false)
const remoteName = ref('')
const remoteHost = ref('')
const remotePort = ref(8080)
const testingConnection = ref(false)
const testResult = ref<'success' | 'fail' | null>(null)
const qrSvg = ref('')
const qrCountdown = ref(0)
let qrTimer: ReturnType<typeof setInterval> | null = null

// ─── Computed ──────────────────────────────────────────────────

const hasConnections = computed(() => connections.value.length > 0)

// ─── QR Generation ─────────────────────────────────────────────

async function generateQrSvg(text: string) {
  try {
    const QRCode = await import('qrcode') as unknown as QRCodeModule
    const svg = await QRCode.create(text, { type: 'svg', errorCorrectionLevel: 'M' })
    qrSvg.value = svg
  } catch {
    qrSvg.value = ''
  }
}

// ─── Methods ───────────────────────────────────────────────────

async function testRemote() {
  testingConnection.value = true
  testResult.value = null
  const ok = await testConnection(remoteHost.value, remotePort.value)
  testResult.value = ok ? 'success' : 'fail'
  testingConnection.value = false
}

async function handleStartPairing() {
  if (!remoteName.value || !remoteHost.value || !remotePort.value) return

  await startPairing(remoteName.value, remoteHost.value, remotePort.value)

  if (pairingQrUrl.value) {
    await generateQrSvg(pairingQrUrl.value)
    // Start countdown
    startCountdown()
  }
}

function handleCancel() {
  cancelPairing()
  qrSvg.value = ''
  stopCountdown()
  showAddRemote.value = false
  remoteName.value = ''
  remoteHost.value = ''
  remotePort.value = 8080
  testResult.value = null
}

function handleClosePairing() {
  cancelPairing()
  qrSvg.value = ''
  stopCountdown()
  showAddRemote.value = false
}

function selectConnection(id: string) {
  setCurrentConnection(id)
}

function handleRemove(id: string) {
  if (confirm('Remove this connection?')) {
    removeConnection(id)
  }
}

function startCountdown() {
  stopCountdown()
  qrTimer = setInterval(() => {
    const remaining = Math.max(0, Math.floor((pairingExpiresAt.value - Date.now()) / 1000))
    qrCountdown.value = remaining
    if (remaining <= 0) {
      stopCountdown()
    }
  }, 1000)
}

function stopCountdown() {
  if (qrTimer !== null) {
    clearInterval(qrTimer)
    qrTimer = null
  }
}

onUnmounted(() => {
  stopCountdown()
  cancelPairing()
})
</script>

<template>
  <div class="remote-connections">
    <!-- Header -->
    <div class="flex items-center justify-between mb-6">
      <div>
        <h2 class="text-lg font-semibold text-[var(--text-primary)]">Remote Connections</h2>
        <p class="text-sm text-[var(--text-secondary)] mt-1">
          Connect to remote praxis servers via QR pairing
        </p>
      </div>
      <button
        class="btn btn-primary"
        @click="showAddRemote = true"
      >
        <Icon name="plus" :size="16" />
        <span>Add Remote</span>
      </button>
    </div>

    <!-- Mode indicator -->
    <div class="mode-indicator" :class="{ remote: isRemoteMode }">
      <span class="mode-dot" :class="{ connected: isRemoteMode }" />
      <span class="mode-label">
        {{ isRemoteMode ? `Remote: ${currentConnectionId}` : 'Local mode' }}
      </span>
      <button
        v-if="isRemoteMode"
        class="btn btn-ghost btn-xs"
        @click="switchToLocal()"
      >
        Switch to local
      </button>
    </div>

    <!-- Connection list -->
    <div v-if="hasConnections" class="connections-list">
      <div
        v-for="conn in connections"
        :key="conn.id"
        class="connection-card"
        :class="{ active: currentConnectionId === conn.id }"
        @click="selectConnection(conn.id)"
      >
        <div class="connection-info">
          <div class="connection-name">
            <Icon name="server" :size="16" />
            <span>{{ conn.name }}</span>
          </div>
          <div class="connection-details">
            <span class="connection-host">{{ conn.host }}:{{ conn.port }}</span>
            <span class="connection-separator">·</span>
            <span class="connection-last-seen">{{ formatLastSeen(conn.lastSeen) }}</span>
          </div>
        </div>
        <div class="connection-actions">
          <button
            class="btn btn-ghost btn-xs"
            :class="{ active: currentConnectionId === conn.id }"
            @click.stop="selectConnection(conn.id)"
          >
            {{ currentConnectionId === conn.id ? 'Connected' : 'Connect' }}
          </button>
          <button
            class="btn btn-ghost btn-xs text-red-400"
            @click.stop="handleRemove(conn.id)"
            aria-label="Remove connection"
          >
            <Icon name="trash-2" :size="14" />
          </button>
        </div>
      </div>
    </div>

    <!-- Empty state -->
    <div v-else class="empty-state">
      <Icon name="wifi-off" :size="48" class="opacity-30" />
      <p class="text-[var(--text-secondary)]">No remote connections yet</p>
      <p class="text-sm text-[var(--text-secondary)] opacity-60">
        Click "Add Remote" to connect to a praxis server
      </p>
    </div>

    <!-- Add Remote Modal -->
    <div v-if="showAddRemote" class="modal-overlay" @click.self="handleCancel">
      <div class="modal-card">
        <!-- Step 1: Connection form -->
        <template v-if="!isPairing">
          <div class="modal-header">
            <h3 class="text-lg font-semibold">Add Remote Connection</h3>
            <button class="btn btn-ghost btn-xs" @click="handleCancel">
              <Icon name="x" :size="16" />
            </button>
          </div>

          <div class="modal-body">
            <div class="form-group">
              <label class="form-label">Connection Name</label>
              <input
                v-model="remoteName"
                type="text"
                class="form-input"
                placeholder="e.g. VPS-Prod, Home Server"
              />
            </div>

            <div class="form-group">
              <label class="form-label">Host</label>
              <input
                v-model="remoteHost"
                type="text"
                class="form-input"
                placeholder="e.g. 192.168.1.100 or myserver.local"
              />
            </div>

            <div class="form-group">
              <label class="form-label">Port</label>
              <input
                v-model="remotePort"
                type="number"
                class="form-input"
                placeholder="8080"
              />
            </div>

            <div class="form-group">
              <button
                class="btn btn-secondary"
                :disabled="!remoteHost || testingConnection"
                @click="testRemote"
              >
                {{ testingConnection ? 'Testing...' : 'Test Connection' }}
              </button>
              <span
                v-if="testResult === 'success'"
                class="ml-2 text-green-400 text-sm"
              >
                ✓ Reachable
              </span>
              <span
                v-else-if="testResult === 'fail'"
                class="ml-2 text-red-400 text-sm"
              >
                ✗ Unreachable
              </span>
            </div>
          </div>

          <div class="modal-footer">
            <button class="btn btn-ghost" @click="handleCancel">Cancel</button>
            <button
              class="btn btn-primary"
              :disabled="!remoteName || !remoteHost"
              @click="handleStartPairing"
            >
              Generate QR
            </button>
          </div>
        </template>

        <!-- Step 2: QR pairing -->
        <template v-else>
          <div class="modal-header">
            <h3 class="text-lg font-semibold">Pair with {{ remoteName }}</h3>
          </div>

          <div class="modal-body text-center">
            <!-- QR Code -->
            <div v-if="pairingStatus === 'waiting' || pairingStatus === 'claimed'" class="qr-section">
              <div
                v-if="qrSvg"
                class="qr-code"
                v-html="qrSvg"
              />
              <div v-else class="qr-placeholder">
                <Icon name="loader" :size="32" class="animate-spin" />
              </div>

              <p class="qr-instructions mt-4 text-sm text-[var(--text-secondary)]">
                Scan this QR code with your phone or
                <a :href="pairingQrUrl" target="_blank" class="text-green-400 underline">
                  open the link
                </a>
              </p>

              <div v-if="qrCountdown > 0" class="qr-timer mt-3">
                <span class="text-xs text-[var(--text-secondary)]">
                  Code expires in {{ qrCountdown }}s
                </span>
                <div class="timer-bar mt-1">
                  <div
                    class="timer-fill"
                    :style="{ width: `${(qrCountdown / 300) * 100}%` }"
                  />
                </div>
              </div>

              <div class="pairing-code mt-3">
                <span class="text-xs text-[var(--text-secondary)]">Or enter code:</span>
                <span class="code-value">{{ pairingCode }}</span>
              </div>

              <div v-if="pairingStatus === 'claimed'" class="mt-4 text-green-400">
                ✓ Paired successfully!
              </div>
            </div>

            <!-- Generating -->
            <div v-else-if="pairingStatus === 'generating'" class="py-8">
              <Icon name="loader" :size="32" class="animate-spin" />
              <p class="mt-3 text-sm text-[var(--text-secondary)]">Generating pairing code...</p>
            </div>

            <!-- Error -->
            <div v-else-if="pairingStatus === 'error'" class="py-8">
              <p class="text-red-400">✗ {{ pairingError || 'Pairing failed' }}</p>
            </div>

            <!-- Expired -->
            <div v-else-if="pairingStatus === 'expired'" class="py-8">
              <p class="text-yellow-400">⚠ Code expired</p>
              <button
                class="btn btn-primary mt-4"
                @click="handleStartPairing"
              >
                Generate new code
              </button>
            </div>
          </div>

          <div class="modal-footer">
            <button
              v-if="pairingStatus !== 'claimed'"
              class="btn btn-ghost"
              @click="handleCancel"
            >
              Cancel
            </button>
            <button
              v-if="pairingStatus === 'claimed'"
              class="btn btn-primary"
              @click="handleClosePairing"
            >
              Done
            </button>
          </div>
        </template>
      </div>
    </div>
  </div>
</template>

<style scoped>
.remote-connections {
  padding: 1rem 0;
}

.mode-indicator {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  padding: 0.5rem 0.75rem;
  background: var(--bg-surface, #27272a);
  border-radius: 0.5rem;
  margin-bottom: 1.5rem;
}

.mode-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: #22c55e;
}

.mode-dot.connected {
  background: #22c55e;
  box-shadow: 0 0 8px rgba(34, 197, 94, 0.4);
}

.mode-label {
  font-size: 0.875rem;
  color: var(--text-secondary, #a1a1aa);
}

.connections-list {
  display: flex;
  flex-direction: column;
  gap: 0.5rem;
}

.connection-card {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 0.75rem 1rem;
  background: var(--bg-surface, #27272a);
  border: 1px solid transparent;
  border-radius: 0.5rem;
  cursor: pointer;
  transition: border-color 0.15s, background 0.15s;
}

.connection-card:hover {
  border-color: var(--border-hover, #52525b);
}

.connection-card.active {
  border-color: #22c55e;
  background: rgba(34, 197, 94, 0.05);
}

.connection-info {
  display: flex;
  flex-direction: column;
  gap: 0.25rem;
}

.connection-name {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-size: 0.9375rem;
  font-weight: 500;
  color: var(--text-primary, #f4f4f5);
}

.connection-details {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  font-size: 0.8125rem;
  color: var(--text-secondary, #a1a1aa);
}

.connection-actions {
  display: flex;
  align-items: center;
  gap: 0.25rem;
}

.empty-state {
  display: flex;
  flex-direction: column;
  align-items: center;
  justify-content: center;
  gap: 0.5rem;
  padding: 3rem 1rem;
  text-align: center;
}

/* Modal */
.modal-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.6);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 100;
  padding: 1rem;
}

.modal-card {
  background: var(--bg-elevated, #18181b);
  border: 1px solid var(--border-default, #3f3f46);
  border-radius: 0.75rem;
  max-width: 480px;
  width: 100%;
  max-height: 90vh;
  overflow-y: auto;
}

.modal-header {
  display: flex;
  align-items: center;
  justify-content: space-between;
  padding: 1rem 1.25rem;
  border-bottom: 1px solid var(--border-default, #3f3f46);
}

.modal-body {
  padding: 1.25rem;
}

.modal-footer {
  display: flex;
  align-items: center;
  justify-content: flex-end;
  gap: 0.5rem;
  padding: 1rem 1.25rem;
  border-top: 1px solid var(--border-default, #3f3f46);
}

.form-group {
  margin-bottom: 1rem;
}

.form-label {
  display: block;
  font-size: 0.8125rem;
  font-weight: 500;
  color: var(--text-secondary, #a1a1aa);
  margin-bottom: 0.375rem;
}

.form-input {
  width: 100%;
  padding: 0.5rem 0.75rem;
  background: var(--bg-input, #3f3f46);
  border: 1px solid var(--border-default, #52525b);
  border-radius: 0.375rem;
  color: var(--text-primary, #f4f4f5);
  font-size: 0.875rem;
  outline: none;
  transition: border-color 0.15s;
}

.form-input:focus {
  border-color: #22c55e;
}

/* QR Section */
.qr-section {
  display: flex;
  flex-direction: column;
  align-items: center;
  padding: 1rem 0;
}

.qr-code {
  background: white;
  padding: 0.75rem;
  border-radius: 0.5rem;
  width: 220px;
  height: 220px;
}

.qr-code :deep(svg) {
  width: 100%;
  height: 100%;
}

.qr-placeholder {
  width: 220px;
  height: 220px;
  display: flex;
  align-items: center;
  justify-content: center;
  background: var(--bg-surface, #27272a);
  border-radius: 0.5rem;
}

.pairing-code {
  display: flex;
  align-items: center;
  gap: 0.5rem;
  background: var(--bg-surface, #27272a);
  padding: 0.5rem 0.75rem;
  border-radius: 0.375rem;
}

.code-value {
  font-family: monospace;
  font-size: 1.125rem;
  font-weight: 700;
  letter-spacing: 0.15em;
  color: var(--text-primary, #f4f4f5);
}

.timer-bar {
  width: 200px;
  height: 4px;
  background: var(--bg-input, #3f3f46);
  border-radius: 2px;
  overflow: hidden;
}

.timer-fill {
  height: 100%;
  background: #22c55e;
  border-radius: 2px;
  transition: width 1s linear;
}
</style>
