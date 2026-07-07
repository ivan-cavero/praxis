/**
 * usePwaNotifications — Desktop push notifications for session events.
 *
 * Uses the Web Notifications API to show system notifications when
 * sessions complete or fail. Permission is requested on first use
 * and persisted in localStorage.
 *
 * In Tauri mode, these appear as native OS notifications.
 * In browser mode, they appear as browser notifications.
 */

import { ref, onScopeDispose } from 'vue'
import { useWebSocket, filterEvents, type AgentCompletedEvent } from './useWebSocket'

const PERMISSION_KEY = 'praxis:notifications-enabled'

const permission = ref<NotificationPermission>(
  typeof Notification !== 'undefined' ? Notification.permission : 'denied'
)
const enabled = ref(localStorage.getItem(PERMISSION_KEY) === 'true')

function requestPermission(): Promise<boolean> {
  if (typeof Notification === 'undefined') return Promise.resolve(false)
  return Notification.requestPermission()
    .then(result => {
      permission.value = result
      const granted = result === 'granted'
      enabled.value = granted
      localStorage.setItem(PERMISSION_KEY, String(granted))
      return granted
    })
    .catch(() => false)
}

function showNotification(title: string, body: string) {
  if (!enabled.value || permission.value !== 'granted') return
  if (typeof Notification === 'undefined') return

  const notif = new Notification(title, {
    body,
    icon: '/favicon.svg',
    badge: '/favicon.svg',
    tag: 'praxis-session',
  })

  // Auto-close after 5 seconds
  setTimeout(() => notif.close(), 5000)

  // Focus window on click
  notif.onclick = () => {
    window.focus()
    notif.close()
  }
}

export function usePwaNotifications() {
  const ws = useWebSocket()

  // Watch for AgentCompleted events and show notifications
  const stopWatch = watchAgentCompletion()

  function watchAgentCompletion() {
    // Use a simple interval to check for new completion events
    let lastSeenCount = 0

    const interval = setInterval(() => {
      const events = ws.events.value
      const completes = filterEvents<AgentCompletedEvent>(events, 'AgentCompleted')
      if (completes.length > lastSeenCount) {
        const newCompletes = completes.slice(lastSeenCount)
        for (const c of newCompletes) {
          const status = c.status === 'completed' ? 'completed' : 'failed'
          showNotification(
            `Agent ${c.agent} ${status}`,
            `Duration: ${c.duration_ms}ms`
          )
        }
        lastSeenCount = completes.length
      }
    }, 2000)

    return () => clearInterval(interval)
  }

  onScopeDispose(() => {
    if (stopWatch) stopWatch()
  })

  function enable() {
    return requestPermission()
  }

  function disable() {
    enabled.value = false
    localStorage.setItem(PERMISSION_KEY, 'false')
  }

  return {
    permission,
    enabled,
    enable,
    disable,
    showNotification,
  }
}
