<script setup lang="ts">
import { ref } from 'vue'
import Icon from '../components/ui/Icon.vue'

const emit = defineEmits<{
  login: [token: string]
}>()

const token = ref('')
const error = ref('')
const isLoading = ref(false)

async function handleLogin() {
  if (!token.value.trim()) {
    error.value = 'Enter your access token'
    return
  }

  isLoading.value = true
  error.value = ''

  try {
    // Validate token with backend
    const res = await fetch('/api/health', {
      headers: { 'Authorization': `Bearer ${token.value}` }
    })

    if (res.ok) {
      localStorage.setItem('project-x-token', token.value)
      emit('login', token.value)
    } else {
      error.value = 'Invalid token or server unavailable'
    }
  } catch (e) {
    // If server is reachable but auth fails, still allow for local dev
    localStorage.setItem('project-x-token', token.value)
    emit('login', token.value)
  } finally {
    isLoading.value = false
  }
}
</script>

<template>
  <div class="h-screen flex items-center justify-center relative overflow-hidden"
    style="background: var(--void)">

    <!-- Background Grid -->
    <div class="absolute inset-0 grid-bg opacity-40" />

    <!-- Background Glow -->
    <div class="absolute top-1/2 left-1/2 -translate-x-1/2 -translate-y-1/2 w-[600px] h-[600px] rounded-full"
      style="background: radial-gradient(circle, rgba(0,229,255,0.06) 0%, transparent 70%)" />

    <!-- Login Card -->
    <div class="relative z-10 w-full max-w-sm mx-4 anim-slide-up">
      <!-- Logo -->
      <div class="text-center mb-8">
        <div class="w-14 h-14 rounded-2xl mx-auto mb-4 flex items-center justify-center text-xl font-black"
          style="background: linear-gradient(135deg, var(--cyan-dim), var(--cyan)); color: var(--void);">
          X
        </div>
        <h1 class="text-lg font-bold tracking-wide text-[var(--text-primary)]">PROJECT-X</h1>
        <p class="text-[10px] font-mono text-[var(--text-ghost)] tracking-[0.3em] uppercase mt-1">Neural Command Center</p>
      </div>

      <!-- Form -->
      <div class="card p-6 glow-cyan">
        <div class="space-y-4">
          <div>
            <label class="data-label block mb-1.5">Access Token</label>
            <div class="relative">
              <input
                v-model="token"
                type="password"
                placeholder="Paste your JWT token..."
                class="w-full bg-[var(--surface-overlay)] border border-[var(--border)] rounded-[var(--radius-md)] px-4 py-3 text-xs font-mono text-[var(--text-primary)] placeholder-[var(--text-muted)]
                       focus:outline-none transition-all duration-200 pr-10"
                @keydown.enter="handleLogin"
              />
              <Icon name="send" :size="14" class="absolute right-3 top-1/2 -translate-y-1/2 text-[var(--text-ghost)]" />
            </div>
          </div>

          <!-- Error -->
          <div v-if="error" class="flex items-center gap-2 text-[10px] font-mono text-[var(--crimson)] bg-[var(--crimson-glow)] px-3 py-2 rounded-[var(--radius-sm)]">
            <Icon name="alert" :size="12" />
            {{ error }}
          </div>

          <!-- Login Button -->
          <button
            @click="handleLogin"
            :disabled="isLoading || !token.trim()"
            class="btn btn-cyan w-full py-3 text-xs font-semibold tracking-wide flex items-center justify-center gap-2"
          >
            <Icon v-if="!isLoading" name="login" :size="14" />
            <span v-if="isLoading" class="w-3 h-3 border-2 border-[var(--void)] border-t-transparent rounded-full animate-spin" />
            <span>{{ isLoading ? 'Authenticating...' : 'Access System' }}</span>
          </button>
        </div>

        <!-- Footer -->
        <div class="mt-4 pt-4 border-t border-[var(--border-subtle)] text-center">
          <p class="text-[9px] font-mono text-[var(--text-ghost)] tracking-wider">
            Token is stored locally in your browser
          </p>
        </div>
      </div>

      <!-- Skip (dev mode) -->
      <div class="text-center mt-4">
        <button
          @click="emit('login', 'dev-mode')"
          class="text-[10px] font-mono text-[var(--text-ghost)] hover:text-[var(--text-muted)] transition-colors"
        >
          Skip authentication (dev mode)
        </button>
      </div>
    </div>
  </div>
</template>
