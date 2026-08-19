import { emit, listen } from '@tauri-apps/api/event'
import { useCallback, useEffect, useState } from 'react'
import {
  defaultDesktopLyricsSettings,
  desktopLyricsSettingsEvent,
  desktopLyricsStorageKey,
  loadDesktopLyricsSettings,
  normalizeDesktopLyricsSettings,
  saveDesktopLyricsSettings,
  type DesktopLyricsSettings,
} from '../shared/desktopLyrics'
import { isTauriRuntime } from '../shared/api'

export function useDesktopLyricsSettings() {
  const [settings, setSettings] = useState(loadDesktopLyricsSettings)

  useEffect(() => {
    const onLocalSettings = (event: Event) => {
      setSettings(normalizeDesktopLyricsSettings(
        (event as CustomEvent<DesktopLyricsSettings>).detail,
      ))
    }
    const onStorage = (event: StorageEvent) => {
      if (event.key !== desktopLyricsStorageKey || !event.newValue) return
      try {
        setSettings(normalizeDesktopLyricsSettings(
          JSON.parse(event.newValue) as Partial<DesktopLyricsSettings>,
        ))
      } catch {
        // Ignore malformed values written outside the application.
      }
    }

    window.addEventListener(desktopLyricsSettingsEvent, onLocalSettings)
    window.addEventListener('storage', onStorage)

    const unlisten = isTauriRuntime()
      ? listen<DesktopLyricsSettings>(desktopLyricsSettingsEvent, ({ payload }) => {
          setSettings(normalizeDesktopLyricsSettings(payload))
        })
      : null

    return () => {
      window.removeEventListener(desktopLyricsSettingsEvent, onLocalSettings)
      window.removeEventListener('storage', onStorage)
      if (unlisten) void unlisten.then((dispose) => dispose())
    }
  }, [])

  const replace = useCallback((next: DesktopLyricsSettings) => {
    const normalized = saveDesktopLyricsSettings(next)
    setSettings(normalized)
    if (isTauriRuntime()) void emit(desktopLyricsSettingsEvent, normalized)
  }, [])

  const update = useCallback((patch: Partial<DesktopLyricsSettings>) => {
    setSettings((current) => {
      const next = saveDesktopLyricsSettings({ ...current, ...patch })
      if (isTauriRuntime()) void emit(desktopLyricsSettingsEvent, next)
      return next
    })
  }, [])

  const toggle = useCallback((key: 'enabled' | 'locked') => {
    setSettings((current) => {
      const next = saveDesktopLyricsSettings({ ...current, [key]: !current[key] })
      if (isTauriRuntime()) void emit(desktopLyricsSettingsEvent, next)
      return next
    })
  }, [])

  const reset = useCallback(() => replace(defaultDesktopLyricsSettings), [replace])

  return { settings, update, toggle, replace, reset }
}
