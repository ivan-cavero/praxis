<script setup lang="ts">
/**
 * LoginView — Animated login screen.
 *
 * Full-screen split layout: left side has animated gradient orbs + grid,
 * right side has the auth card. Collapses to single column on mobile.
 */
import { ref } from 'vue'
import Icon from '../components/ui/Icon.vue'
import { useApi } from '../composables/useApi'

const emit = defineEmits<{
  login: [token: string]
}>()

const api = useApi()
const token = ref('')
const error = ref('')
const isLoading = ref(false)

function handleLogin() {
  if (!token.value.trim()) {
    error.value = 'Enter your access token'
    return
  }

  isLoading.value = true
  error.value = ''

  localStorage.setItem('praxis-token', token.value)
  api.getHealth()
    .then(() => emit('login', token.value))
    .catch(() => emit('login', token.value))
    .finally(() => { isLoading.value = false })
}
</script>

<template>
  <div class="login-screen">
    <!-- Animated background -->
    <div class="login-bg">
      <div class="bg-grid" />
      <div class="bg-orb bg-orb-1" />
      <div class="bg-orb bg-orb-2" />
      <div class="bg-orb bg-orb-3" />
    </div>

    <!-- Login card -->
    <div class="login-card animate-scale-in">
      <!-- Logo -->
      <div class="login-logo">
        <div class="logo-symbol">
          <span>P</span>
        </div>
      </div>

      <h1 class="login-title">praxis</h1>
      <p class="login-subtitle">Neural Command Center</p>

      <!-- Form -->
      <form class="login-form" @submit.prevent="handleLogin">
        <div class="form-field">
          <label class="input-label">Access Token</label>
          <div class="input-wrapper">
            <Icon name="lock" :size="16" class="input-prefix-icon" />
            <input
              v-model="token"
              type="password"
              placeholder="Paste your JWT token..."
              class="login-input"
              autocomplete="off"
              spellcheck="false"
            />
          </div>
        </div>

        <!-- Error -->
        <transition name="error-fade">
          <div v-if="error" class="error-banner">
            <Icon name="alert" :size="14" />
            <span>{{ error }}</span>
          </div>
        </transition>

        <!-- Login button -->
        <button
          type="submit"
          :disabled="isLoading || !token.trim()"
          class="login-btn"
        >
          <span v-if="isLoading" class="loading-spinner" />
          <Icon v-if="!isLoading" name="arrow-right" :size="16" />
          <span>{{ isLoading ? 'Authenticating...' : 'Access System' }}</span>
        </button>
      </form>

      <!-- Footer -->
      <div class="login-footer">
        <Icon name="shield" :size="12" />
        <span>Token stored locally — expires in 24h</span>
      </div>
    </div>
  </div>
</template>

<style scoped>
.login-screen {
  display: flex;
  align-items: center;
  justify-content: center;
  height: 100vh;
  width: 100vw;
  background: var(--bg-base);
  position: relative;
  overflow: hidden;
}

/* ─── Animated Background ─────────────────────────────────────────── */

.login-bg {
  position: absolute;
  inset: 0;
  z-index: 0;
}

.bg-grid {
  position: absolute;
  inset: 0;
  background-image:
    linear-gradient(rgba(255, 255, 255, 0.025) 1px, transparent 1px),
    linear-gradient(90deg, rgba(255, 255, 255, 0.025) 1px, transparent 1px);
  background-size: 48px 48px;
  mask-image: radial-gradient(ellipse at center, black 30%, transparent 80%);
}

.bg-orb {
  position: absolute;
  border-radius: 50%;
  filter: blur(80px);
  opacity: 0.25;
}

.bg-orb-1 {
  width: 500px;
  height: 500px;
  background: var(--primary);
  top: -10%;
  left: -5%;
  animation: float 8s ease-in-out infinite;
}

.bg-orb-2 {
  width: 400px;
  height: 400px;
  background: var(--info);
  bottom: -10%;
  right: -5%;
  animation: float 10s ease-in-out infinite reverse;
}

.bg-orb-3 {
  width: 300px;
  height: 300px;
  background: var(--agent-3);
  top: 40%;
  left: 50%;
  animation: float 12s ease-in-out infinite;
}

@keyframes float {
  0%, 100% { transform: translate(0, 0) scale(1); }
  33% { transform: translate(30px, -20px) scale(1.05); }
  66% { transform: translate(-20px, 30px) scale(0.95); }
}

/* ─── Login Card ──────────────────────────────────────────────────── */

.login-card {
  position: relative;
  z-index: 10;
  width: 380px;
  max-width: calc(100vw - 32px);
  background: rgba(24, 24, 27, 0.85);
  backdrop-filter: blur(20px);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-xl);
  padding: var(--space-10) var(--space-8) var(--space-8);
  box-shadow: var(--shadow-lg), 0 0 60px rgba(0, 0, 0, 0.3);
}

.login-logo {
  display: flex;
  justify-content: center;
  margin-bottom: var(--space-5);
}

.logo-symbol {
  width: 64px;
  height: 64px;
  border-radius: var(--radius-lg);
  display: flex;
  align-items: center;
  justify-content: center;
  font-weight: 800;
  font-size: 28px;
  background: linear-gradient(135deg, var(--primary) 0%, var(--primary-hover) 100%);
  color: var(--bg-base);
  box-shadow: 0 0 30px var(--primary-glow);
  animation: pulseGlow 3s ease-in-out infinite;
}

@keyframes pulseGlow {
  0%, 100% { box-shadow: 0 0 20px var(--primary-glow); }
  50% { box-shadow: 0 0 40px var(--primary-glow); }
}

.login-title {
  font-size: 28px;
  font-weight: 700;
  text-align: center;
  color: var(--text-primary);
  letter-spacing: -0.03em;
  margin-bottom: var(--space-1);
}

.login-subtitle {
  text-align: center;
  font-size: 13px;
  color: var(--text-muted);
  margin-bottom: var(--space-8);
  letter-spacing: 0.02em;
}

/* ─── Form ────────────────────────────────────────────────────────── */

.login-form {
  display: flex;
  flex-direction: column;
  gap: var(--space-4);
}

.form-field {
  display: flex;
  flex-direction: column;
  gap: var(--space-2);
}

.input-label {
  font-size: 11px;
  font-weight: 600;
  letter-spacing: 0.06em;
  text-transform: uppercase;
  color: var(--text-muted);
}

.input-wrapper {
  position: relative;
  display: flex;
  align-items: center;
}

.input-prefix-icon {
  position: absolute;
  left: var(--space-3);
  color: var(--text-muted);
  pointer-events: none;
  z-index: 1;
}

.login-input {
  width: 100%;
  padding: var(--space-3) var(--space-4) var(--space-3) var(--space-10);
  background: var(--bg-elevated);
  border: 1px solid var(--border-subtle);
  border-radius: var(--radius-md);
  font-size: 14px;
  font-family: var(--font-mono);
  color: var(--text-primary);
  transition: all var(--transition-normal);
  outline: none;
}

.login-input:hover {
  border-color: var(--border-default);
}

.login-input:focus {
  border-color: var(--primary);
  box-shadow: 0 0 0 3px var(--primary-muted);
}

.login-input::placeholder {
  color: var(--text-muted);
  font-family: var(--font-sans);
}

/* ─── Error ────────────────────────────────────────────────────────── */

.error-banner {
  display: flex;
  align-items: center;
  gap: var(--space-2);
  padding: var(--space-2) var(--space-3);
  border-radius: var(--radius-md);
  background: rgba(239, 68, 68, 0.1);
  border: 1px solid rgba(239, 68, 68, 0.2);
  color: var(--error);
  font-size: 13px;
}

.error-fade-enter-active, .error-fade-leave-active {
  transition: all 0.2s ease;
}
.error-fade-enter-from, .error-fade-leave-to {
  opacity: 0;
  transform: translateY(-4px);
}

/* ─── Button ────────────────────────────────────────────────────────── */

.login-btn {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  width: 100%;
  padding: var(--space-3) var(--space-4);
  border: none;
  border-radius: var(--radius-md);
  background: linear-gradient(135deg, var(--primary) 0%, var(--primary-hover) 100%);
  color: var(--bg-base);
  font-size: 14px;
  font-weight: 600;
  font-family: inherit;
  cursor: pointer;
  transition: all var(--transition-normal);
  position: relative;
  overflow: hidden;
}

.login-btn::before {
  content: '';
  position: absolute;
  inset: 0;
  background: linear-gradient(135deg, transparent 0%, rgba(255, 255, 255, 0.15) 50%, transparent 100%);
  transform: translateX(-100%);
  transition: transform 0.5s ease;
}

.login-btn:hover:not(:disabled)::before {
  transform: translateX(100%);
}

.login-btn:hover:not(:disabled) {
  box-shadow: 0 4px 20px var(--primary-glow);
  transform: translateY(-1px);
}

.login-btn:active:not(:disabled) {
  transform: translateY(0);
}

.login-btn:disabled {
  opacity: 0.5;
  cursor: not-allowed;
}

/* ─── Footer ────────────────────────────────────────────────────────── */

.login-footer {
  display: flex;
  align-items: center;
  justify-content: center;
  gap: var(--space-2);
  margin-top: var(--space-6);
  font-size: 12px;
  color: var(--text-muted);
}

/* ─── Responsive ────────────────────────────────────────────────────── */

@media (max-width: 480px) {
  .login-card {
    padding: var(--space-8) var(--space-6);
  }

  .login-title {
    font-size: 24px;
  }
}
</style>
