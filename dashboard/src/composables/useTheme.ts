/**
 * useTheme — Accent color and font size customization.
 *
 * Persists user preferences in localStorage and applies them
 * as CSS custom properties on `:root` at load time.
 *
 * Supported accent colors: green (default), blue, purple, orange, pink, cyan.
 * Supported font sizes: small (13px), medium (14px, default), large (16px).
 */

import { ref, watch } from 'vue'

export type AccentColor = 'green' | 'blue' | 'purple' | 'orange' | 'pink' | 'cyan'
export type FontSize = 'small' | 'medium' | 'large'

const STORAGE_KEY = 'praxis:theme'

const ACCENT_COLORS: Record<AccentColor, { primary: string; hover: string; muted: string; glow: string }> = {
  green: { primary: '#22c55e', hover: '#16a34a', muted: 'rgba(34, 197, 94, 0.12)', glow: 'rgba(34, 197, 94, 0.25)' },
  blue: { primary: '#3b82f6', hover: '#2563eb', muted: 'rgba(59, 130, 246, 0.12)', glow: 'rgba(59, 130, 246, 0.25)' },
  purple: { primary: '#a855f7', hover: '#9333ea', muted: 'rgba(168, 85, 247, 0.12)', glow: 'rgba(168, 85, 247, 0.25)' },
  orange: { primary: '#f59e0b', hover: '#d97706', muted: 'rgba(245, 158, 11, 0.12)', glow: 'rgba(245, 158, 11, 0.25)' },
  pink: { primary: '#ec4899', hover: '#db2777', muted: 'rgba(236, 72, 153, 0.12)', glow: 'rgba(236, 72, 153, 0.25)' },
  cyan: { primary: '#14b8a6', hover: '#0d9488', muted: 'rgba(20, 184, 166, 0.12)', glow: 'rgba(20, 184, 166, 0.25)' },
}

const FONT_SIZES: Record<FontSize, string> = {
  small: '13px',
  medium: '14px',
  large: '16px',
}

interface ThemeSettings {
  accent: AccentColor
  fontSize: FontSize
}

function loadSettings(): ThemeSettings {
  const stored = localStorage.getItem(STORAGE_KEY)
  if (stored) {
    try {
      return JSON.parse(stored) as ThemeSettings
    } catch {
      // fall through to defaults
    }
  }
  return { accent: 'green', fontSize: 'medium' }
}

function applySettings(settings: ThemeSettings) {
  const root = document.documentElement
  const colors = ACCENT_COLORS[settings.accent]
  root.style.setProperty('--primary', colors.primary)
  root.style.setProperty('--primary-hover', colors.hover)
  root.style.setProperty('--primary-muted', colors.muted)
  root.style.setProperty('--primary-glow', colors.glow)
  root.style.setProperty('--shadow-glow', `0 0 20px ${colors.muted}`)
  root.style.fontSize = FONT_SIZES[settings.fontSize]
}

const settings = ref<ThemeSettings>(loadSettings())

// Apply on load
applySettings(settings.value)

// Watch and persist + apply
watch(settings, (newSettings) => {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(newSettings))
  applySettings(newSettings)
}, { deep: true })

export function useTheme() {
  function setAccent(color: AccentColor) {
    settings.value = { ...settings.value, accent: color }
  }

  function setFontSize(size: FontSize) {
    settings.value = { ...settings.value, fontSize: size }
  }

  function reset() {
    settings.value = { accent: 'green', fontSize: 'medium' }
  }

  return {
    accent: ref(settings.value.accent),
    fontSize: ref(settings.value.fontSize),
    setAccent,
    setFontSize,
    reset,
  }
}

export { ACCENT_COLORS, FONT_SIZES }
