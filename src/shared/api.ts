import { invoke } from '@tauri-apps/api/core'
import type { DesktopLyricsSettings } from './desktopLyrics'
import type { AppRole, PlayerCommand, RuntimeStatus } from './types'
import { mockRuntime } from './mock'

export const isTauriRuntime = () => '__TAURI_INTERNALS__' in window

let browserRuntime = structuredClone(mockRuntime)

const notifyBrowser = () => {
  window.dispatchEvent(new CustomEvent('mock-runtime', { detail: structuredClone(browserRuntime) }))
}

const browserApi = {
  async getRuntimeStatus() { return structuredClone(browserRuntime) },
  async setRole(role: AppRole) {
    browserRuntime.role = role
    browserRuntime.connection = role === 'host' ? 'connected' : 'discovering'
    window.setTimeout(() => { browserRuntime.connection = 'connected'; notifyBrowser() }, 700)
    return structuredClone(browserRuntime)
  },
  async setAllowControl(allowControl: boolean) {
    browserRuntime.allowControl = allowControl
    return structuredClone(browserRuntime)
  },
  async sendPlayerCommand(command: PlayerCommand) {
    if (command === 'toggle_play_pause') browserRuntime.playback.playing = !browserRuntime.playback.playing
    browserRuntime.playback.sequence += 1
    browserRuntime.playback.observedAtMs = Date.now()
    notifyBrowser()
  },
  async discoverHosts() {
    browserRuntime.connection = 'discovering'
    window.setTimeout(() => { browserRuntime.connection = 'connected'; notifyBrowser() }, 600)
    return browserRuntime.hosts
  },
  async connectManualHost(address: string) {
    const endpoint = address.includes(':') ? address : `${address}:17636`
    browserRuntime.role = 'client'
    browserRuntime.connection = 'connecting'
    browserRuntime.preferredServerAddress = endpoint
    browserRuntime.serverAddress = endpoint
    browserRuntime.connectionError = null
    notifyBrowser()
    window.setTimeout(() => { browserRuntime.connection = 'connected'; notifyBrowser() }, 500)
    return structuredClone(browserRuntime)
  },
  async startAutomaticDiscovery() {
    browserRuntime.role = 'client'
    browserRuntime.connection = 'discovering'
    browserRuntime.preferredServerAddress = null
    browserRuntime.connectionError = null
    notifyBrowser()
    window.setTimeout(() => { browserRuntime.connection = 'connected'; notifyBrowser() }, 700)
    return structuredClone(browserRuntime)
  },
  async configureDesktopLyricsWindow() {},
  async resetDesktopLyricsPosition() {},
}

export const api = {
  getRuntimeStatus: (): Promise<RuntimeStatus> => isTauriRuntime() ? invoke('get_runtime_status') : browserApi.getRuntimeStatus(),
  setRole: (role: AppRole): Promise<RuntimeStatus> => isTauriRuntime() ? invoke('set_role', { role }) : browserApi.setRole(role),
  setAllowControl: (allowControl: boolean): Promise<RuntimeStatus> => isTauriRuntime() ? invoke('set_allow_control', { allowControl }) : browserApi.setAllowControl(allowControl),
  sendPlayerCommand: (command: PlayerCommand): Promise<void> => isTauriRuntime() ? invoke('send_player_command', { command }) : browserApi.sendPlayerCommand(command),
  discoverHosts: (): Promise<RuntimeStatus['hosts']> => isTauriRuntime() ? invoke('discover_hosts') : browserApi.discoverHosts(),
  connectManualHost: (address: string): Promise<RuntimeStatus> => isTauriRuntime()
    ? invoke('connect_manual_host', { address })
    : browserApi.connectManualHost(address),
  startAutomaticDiscovery: (): Promise<RuntimeStatus> => isTauriRuntime()
    ? invoke('start_automatic_discovery')
    : browserApi.startAutomaticDiscovery(),
  configureDesktopLyricsWindow: (
    settings: Pick<DesktopLyricsSettings, 'enabled' | 'alwaysOnTop' | 'locked'>,
  ): Promise<void> => isTauriRuntime()
    ? invoke('configure_desktop_lyrics_window', {
        visible: settings.enabled,
        alwaysOnTop: settings.alwaysOnTop,
        locked: settings.locked,
      })
    : browserApi.configureDesktopLyricsWindow(),
  resetDesktopLyricsPosition: (): Promise<void> => isTauriRuntime()
    ? invoke('reset_desktop_lyrics_position')
    : browserApi.resetDesktopLyricsPosition(),
}
