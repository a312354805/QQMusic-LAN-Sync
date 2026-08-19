import { useEffect, useMemo, useState } from 'react'
import { listen } from '@tauri-apps/api/event'
import { api, isTauriRuntime } from '../shared/api'
import type { AppRole, LyricsDocument, PlaybackSnapshot, PlayerCommand, RuntimeStatus } from '../shared/types'

export function useRuntime() {
  const [runtime, setRuntime] = useState<RuntimeStatus | null>(null)
  const [clock, setClock] = useState(Date.now())
  const [error, setError] = useState<string | null>(null)
  const [busy, setBusy] = useState(false)

  useEffect(() => {
    void api.getRuntimeStatus().then(setRuntime).catch((reason) => setError(String(reason)))
    if (!isTauriRuntime()) {
      const onMockRuntime = (event: Event) => setRuntime((event as CustomEvent<RuntimeStatus>).detail)
      window.addEventListener('mock-runtime', onMockRuntime)
      return () => window.removeEventListener('mock-runtime', onMockRuntime)
    }
    const unlisteners = [
      listen<RuntimeStatus>('runtime://status', ({ payload }) => setRuntime(payload)),
      listen<PlaybackSnapshot>('playback://snapshot', ({ payload }) => setRuntime((current) => current ? { ...current, playback: payload } : current)),
      listen<LyricsDocument | null>('lyrics://document', ({ payload }) => setRuntime((current) => current ? { ...current, lyrics: payload } : current)),
    ]
    return () => { void Promise.all(unlisteners).then((values) => values.forEach((unlisten) => unlisten())) }
  }, [])

  useEffect(() => {
    const timer = window.setInterval(() => setClock(Date.now()), 100)
    return () => window.clearInterval(timer)
  }, [])

  const positionMs = useMemo(() => {
    if (!runtime) return 0
    const snapshot = runtime.playback
    const base = snapshot.positionMs ?? 0
    if (!snapshot.playing) return base
    return Math.min(snapshot.durationMs ?? Number.MAX_SAFE_INTEGER, base + Math.max(0, clock - snapshot.observedAtMs))
  }, [clock, runtime])

  const activeLineIndex = useMemo(() => {
    const lines = runtime?.lyrics?.original.lines ?? []
    const adjusted = positionMs + (runtime?.lyrics?.offsetMs ?? 0)
    let active = -1
    for (let index = 0; index < lines.length; index += 1) {
      if (lines[index].startMs > adjusted) break
      active = index
    }
    return active
  }, [positionMs, runtime])

  const perform = async (operation: () => Promise<unknown>) => {
    setError(null)
    setBusy(true)
    try {
      await operation()
      setRuntime(await api.getRuntimeStatus())
    } catch (reason) {
      setError(reason instanceof Error ? reason.message : String(reason))
    } finally {
      setBusy(false)
    }
  }

  return {
    runtime,
    positionMs,
    activeLineIndex,
    error,
    busy,
    setRole: (role: AppRole) => perform(() => api.setRole(role)),
    setAllowControl: (allow: boolean) => perform(() => api.setAllowControl(allow)),
    sendPlayerCommand: (command: PlayerCommand) => perform(() => api.sendPlayerCommand(command)),
    discoverHosts: () => perform(() => api.discoverHosts()),
    connectManualHost: (address: string) => perform(() => api.connectManualHost(address)),
    startAutomaticDiscovery: () => perform(() => api.startAutomaticDiscovery()),
  }
}
