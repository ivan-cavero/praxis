/**
 * useToast — Global toast notification system.
 *
 * Provides success, error, info, and warning toasts that auto-dismiss.
 * Usage:
 *   const toast = useToast()
 *   toast.success('Saved!')
 *   toast.error('Failed to save', { duration: 5000 })
 */

import { ref } from 'vue'

export type ToastKind = 'success' | 'error' | 'info' | 'warning'

export interface ToastOptions {
  duration?: number
  dismissible?: boolean
}

export interface ToastItem {
  id: string
  kind: ToastKind
  message: string
  dismissible: boolean
}

const toasts = ref<ToastItem[]>([])
const DEFAULT_DURATION = 3500

function generateId(): string {
  return typeof crypto !== 'undefined' && crypto.randomUUID
    ? crypto.randomUUID()
    : Date.now().toString(36) + Math.random().toString(36).slice(2)
}

function dismiss(id: string): void {
  toasts.value = toasts.value.filter(t => t.id !== id)
}

function show(kind: ToastKind, message: string, options?: ToastOptions): void {
  const id = generateId()
  const dismissible = options?.dismissible ?? true
  const duration = options?.duration ?? DEFAULT_DURATION

  toasts.value = [...toasts.value, { id, kind, message, dismissible }]

  if (duration > 0) {
    setTimeout(() => dismiss(id), duration)
  }
}

export function useToast() {
  return {
    toasts,
    dismiss,
    success: (msg: string, opts?: ToastOptions) => show('success', msg, opts),
    error: (msg: string, opts?: ToastOptions) => show('error', msg, { duration: 6000, ...opts }),
    info: (msg: string, opts?: ToastOptions) => show('info', msg, opts),
    warning: (msg: string, opts?: ToastOptions) => show('warning', msg, { duration: 5000, ...opts }),
  }
}
