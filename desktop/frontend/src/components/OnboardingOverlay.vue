<script setup lang="ts">
/**
 * OnboardingOverlay — First-run guide for new users.
 *
 * Shows a 3-step intro overlay on first visit (stored in localStorage).
 * Explains what praxis is and how to get started.
 */
import { ref, onMounted } from 'vue'
import { useRouter } from 'vue-router'
import Icon from '../components/ui/Icon.vue'

const STORAGE_KEY = 'praxis-onboarding-completed'

const router = useRouter()
const isVisible = ref(false)
const currentStep = ref(0)

const steps = [
  {
    icon: 'robot',
    title: 'Welcome to praxis',
    description: 'praxis is an autonomous multi-agent system. You give it a goal, and a pipeline of specialized agents (architect, coder, reviewer, security, tester) works through it automatically.',
    action: 'Next',
  },
  {
    icon: 'folder',
    title: 'Create a project',
    description: 'Projects organize your goals and agent configurations. Click the + button in the sidebar to create your first project, or use the CLI: praxis init my-project',
    action: 'Next',
  },
  {
    icon: 'send',
    title: 'Send a goal',
    description: 'Open a project chat and describe what you want to build. The orchestrator distributes your goal across agents automatically. Use Shift+Enter for multi-line input, or separate goals with --- for multi-goal dispatch.',
    action: 'Get started',
  },
]

onMounted(() => {
  const completed = localStorage.getItem(STORAGE_KEY)
  if (!completed) {
    isVisible.value = true
  }
})

function nextStep(): void {
  if (currentStep.value < steps.length - 1) {
    currentStep.value++
  } else {
    dismiss()
  }
}

function skipStep(): void {
  dismiss()
}

function dismiss(): void {
  isVisible.value = false
  localStorage.setItem(STORAGE_KEY, 'true')
}

function goToAgents(): void {
  dismiss()
  router.push('/agents')
}
</script>

<template>
  <div v-if="isVisible" class="onboarding-overlay" @click.self="skipStep">
    <div class="onboarding-card">
      <button class="onboarding-skip" @click="skipStep" aria-label="Skip onboarding">
        <Icon name="x" :size="18" />
      </button>

      <div class="onboarding-step">
        <div class="step-icon">
          <Icon :name="steps[currentStep].icon" :size="48" />
        </div>
        <h2 class="step-title">{{ steps[currentStep].title }}</h2>
        <p class="step-desc">{{ steps[currentStep].description }}</p>
      </div>

      <div class="onboarding-progress">
        <div
          v-for="(_, i) in steps"
          :key="i"
          class="progress-dot"
          :class="{ active: i === currentStep, done: i < currentStep }"
        />
      </div>

      <div class="onboarding-actions">
        <button class="btn btn-ghost" @click="skipStep">Skip</button>
        <button class="btn btn-primary" @click="currentStep === steps.length - 1 ? goToAgents() : nextStep()">
          {{ steps[currentStep].action }}
        </button>
      </div>
    </div>
  </div>
</template>

<style scoped>
.onboarding-overlay {
  position: fixed;
  inset: 0;
  background: rgba(0, 0, 0, 0.7);
  backdrop-filter: blur(8px);
  display: flex;
  align-items: center;
  justify-content: center;
  z-index: 200;
  animation: overlayFade 0.2s ease-out;
}

@keyframes overlayFade {
  from { opacity: 0; }
  to { opacity: 1; }
}

.onboarding-card {
  position: relative;
  width: 480px;
  max-width: 90vw;
  background: var(--bg-surface);
  border: 1px solid var(--border-default);
  border-radius: var(--radius-xl);
  padding: var(--space-8);
  display: flex;
  flex-direction: column;
  gap: var(--space-6);
  box-shadow: 0 8px 40px rgba(0, 0, 0, 0.4);
  animation: cardSlide 0.25s ease-out;
}

@keyframes cardSlide {
  from { opacity: 0; transform: translateY(20px) scale(0.96); }
  to { opacity: 1; transform: translateY(0) scale(1); }
}

.onboarding-skip {
  position: absolute;
  top: 16px;
  right: 16px;
  display: flex;
  align-items: center;
  justify-content: center;
  width: 32px;
  height: 32px;
  border: none;
  border-radius: var(--radius-md);
  background: transparent;
  color: var(--text-muted);
  cursor: pointer;
  transition: all 0.15s;
}

.onboarding-skip:hover {
  color: var(--text-primary);
  background: var(--bg-hover);
}

.onboarding-step {
  display: flex;
  flex-direction: column;
  align-items: center;
  text-align: center;
  gap: var(--space-4);
}

.step-icon {
  display: flex;
  align-items: center;
  justify-content: center;
  width: 80px;
  height: 80px;
  border-radius: var(--radius-xl);
  background: var(--primary-muted);
  color: var(--primary);
}

.step-title {
  font-size: 20px;
  font-weight: 700;
  color: var(--text-primary);
  letter-spacing: -0.02em;
}

.step-desc {
  font-size: 14px;
  line-height: 1.6;
  color: var(--text-muted);
  max-width: 380px;
}

.onboarding-progress {
  display: flex;
  justify-content: center;
  gap: 8px;
}

.progress-dot {
  width: 8px;
  height: 8px;
  border-radius: 50%;
  background: var(--border-default);
  transition: all 0.2s;
}

.progress-dot.active {
  background: var(--primary);
  transform: scale(1.25);
}

.progress-dot.done {
  background: var(--primary);
  opacity: 0.5;
}

.onboarding-actions {
  display: flex;
  justify-content: flex-end;
  gap: var(--space-3);
}

.btn {
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
