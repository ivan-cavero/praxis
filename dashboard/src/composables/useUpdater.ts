/**
 * useUpdater — Tauri auto-update composable.
 *
 * Checks for updates via tauri-plugin-updater on mount,
 * exposes state for a notification banner in App.vue.
 *
 * Gracefully degrades when not running inside Tauri (browser dev mode).
 */

import { ref, readonly } from 'vue'

// ─── State (module-level singleton) ──────────────────────────────

const updateAvailable = ref(false)
const updateVersion = ref<string | null>(null)
const updateBody = ref<string | null>(null)
const updateDate = ref<string | null>(null)
const checking = ref(false)
const downloading = ref(false)
const downloadProgress = ref(0)
const downloadTotal = ref(0)
const installing = ref(false)
const installDone = ref(false)
const dismissed = ref(false)
const error = ref<string | null>(null)

// ─── Composable ──────────────────────────────────────────────────

export function useUpdater() {
  /** Check for updates. Safe to call outside Tauri — catches the import error. */
  async function checkForUpdates(): Promise<void> {
    if (checking.value) return
    checking.value = true
    error.value = null

    try {
      const { check } = await import('@tauri-apps/plugin-updater')
      const update = await check()

      if (update) {
        updateAvailable.value = true
        updateVersion.value = update.version
        updateBody.value = update.body ?? null
        updateDate.value = update.date ?? null
      }
    } catch {
      // Not running in Tauri — silently ignore
    } finally {
      checking.value = false
    }
  }

  /** Download and install the available update. */
  async function installUpdate(): Promise<void> {
    if (!updateAvailable.value) return
    downloading.value = true
    error.value = null

    try {
      const { check } = await import('@tauri-apps/plugin-updater')
      const update = await check()

      if (!update) {
        error.value = 'No update available'
        downloading.value = false
        return
      }

      await update.downloadAndInstall((event) => {
        switch (event.event) {
          case 'Started': {
            downloadTotal.value = event.data.contentLength ?? 0
            downloadProgress.value = 0
            break
          }
          case 'Progress': {
            downloadProgress.value += event.data.chunkLength
            break
          }
          case 'Finished': {
            installing.value = true
            break
          }
        }
      })

      installDone.value = true
      downloading.value = false
      installing.value = false
    } catch (cause) {
      error.value = String(cause)
      downloading.value = false
      installing.value = false
    }
  }

  /** Dismiss the update notification. */
  function dismissUpdate(): void {
    dismissed.value = true
  }

  /** Reset all state. */
  function reset(): void {
    updateAvailable.value = false
    updateVersion.value = null
    updateBody.value = null
    updateDate.value = null
    checking.value = false
    downloading.value = false
    downloadProgress.value = 0
    downloadTotal.value = 0
    installing.value = false
    installDone.value = false
    dismissed.value = false
    error.value = null
  }

  /** Download progress as a percentage (0–100), or 0 if unknown. */
  function progressPercent(): number {
    if (downloadTotal.value > 0) {
      return Math.round((downloadProgress.value / downloadTotal.value) * 100)
    }
    return 0
  }

  return {
    updateAvailable: readonly(updateAvailable),
    updateVersion: readonly(updateVersion),
    updateBody: readonly(updateBody),
    updateDate: readonly(updateDate),
    checking: readonly(checking),
    downloading: readonly(downloading),
    downloadProgress: readonly(downloadProgress),
    downloadTotal: readonly(downloadTotal),
    progressPercent,
    installing: readonly(installing),
    installDone: readonly(installDone),
    dismissed: readonly(dismissed),
    error: readonly(error),

    checkForUpdates,
    installUpdate,
    dismissUpdate,
    reset,
  }
}
