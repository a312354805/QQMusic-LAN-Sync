import {
  ChevronRight, CirclePause, CirclePlay, Headphones, Laptop, ListMusic,
  MonitorSpeaker, Network, Radio, RefreshCw, Settings2, ShieldCheck,
  SkipBack, SkipForward, TriangleAlert, Users, Wifi,
} from 'lucide-react'
import clsx from 'clsx'
import { listen } from '@tauri-apps/api/event'
import { useEffect, useState } from 'react'
import { DesktopLyricsSettingsView } from './components/DesktopLyricsSettingsView'
import { ConnectionManager } from './components/ConnectionManager'
import { IconButton } from './components/IconButton'
import { LyricsPanel } from './components/LyricsPanel'
import { useDesktopLyricsSettings } from './hooks/useDesktopLyricsSettings'
import { useRuntime } from './hooks/useRuntime'
import { api, isTauriRuntime } from './shared/api'
import {
  desktopLyricsTrayActionEvent,
  type DesktopLyricsTrayAction,
} from './shared/desktopLyrics'
import type { AppRole } from './shared/types'

type AppView = 'player' | 'lyrics' | 'connections' | 'settings'

const formatTime = (milliseconds: number | null) => {
  const seconds = Math.max(0, Math.floor((milliseconds ?? 0) / 1000))
  return `${Math.floor(seconds / 60)}:${String(seconds % 60).padStart(2, '0')}`
}

function RoleSwitch({ role, busy, onChange }: { role: AppRole; busy: boolean; onChange: (role: AppRole) => void }) {
  return (
    <div className="role-switch" role="group" aria-label="运行模式">
      <button disabled={busy || role === 'host'} className={clsx(role === 'host' && 'is-active')} onClick={() => onChange('host')}><MonitorSpeaker size={15} />主机</button>
      <button disabled={busy || role === 'client'} className={clsx(role === 'client' && 'is-active')} onClick={() => onChange('client')}><Laptop size={15} />客户端</button>
    </div>
  )
}

function App() {
  const [view, setView] = useState<AppView>('player')
  const [desktopLyricsError, setDesktopLyricsError] = useState<string | null>(null)
  const {
    runtime,
    positionMs,
    error,
    busy,
    setRole,
    setAllowControl,
    sendPlayerCommand,
    discoverHosts,
    connectManualHost,
    startAutomaticDiscovery,
  } = useRuntime()
  const desktopLyrics = useDesktopLyricsSettings()
  const toggleDesktopLyricsSetting = desktopLyrics.toggle

  useEffect(() => {
    setDesktopLyricsError(null)
    void api.configureDesktopLyricsWindow({
      enabled: desktopLyrics.settings.enabled,
      alwaysOnTop: desktopLyrics.settings.alwaysOnTop,
      locked: desktopLyrics.settings.locked,
    }).catch((reason) => {
      setDesktopLyricsError(reason instanceof Error ? reason.message : String(reason))
    })
  }, [desktopLyrics.settings.alwaysOnTop, desktopLyrics.settings.enabled, desktopLyrics.settings.locked])

  useEffect(() => {
    if (!isTauriRuntime()) return
    const unlisten = listen<DesktopLyricsTrayAction>(desktopLyricsTrayActionEvent, ({ payload }) => {
      if (payload === 'toggle_enabled') toggleDesktopLyricsSetting('enabled')
      if (payload === 'toggle_locked') toggleDesktopLyricsSetting('locked')
    })
    return () => { void unlisten.then((dispose) => dispose()) }
  }, [toggleDesktopLyricsSetting])

  if (!runtime) return <div className="boot-screen"><img src="/app-icon.png" alt="" /><span>正在启动 QQMusic LAN Sync</span></div>

  const { playback, lyrics } = runtime
  const progress = playback.durationMs ? Math.min(100, (positionMs / playback.durationMs) * 100) : 0
  const connected = runtime.connection === 'connected'
  const devices = runtime.role === 'host' ? runtime.peers : runtime.hosts
  const connectionLabel = connected
    ? '已连接'
    : runtime.connection === 'connecting'
      ? '连接中'
      : runtime.connection === 'discovering'
        ? '发现中'
        : '未连接'

  return (
    <main className="app-shell">
      <aside className="sidebar">
        <div className="brand"><img src="/app-icon.png" alt="" /><div><strong>QQMusic Sync</strong><span>局域网歌词伴侣</span></div></div>
        <RoleSwitch role={runtime.role} busy={busy} onChange={(role) => void setRole(role)} />
        <nav aria-label="主导航">
          <button className={clsx('nav-item', view === 'player' && 'is-active')} aria-current={view === 'player' ? 'page' : undefined} onClick={() => setView('player')}><Radio size={18} /><span>正在播放</span></button>
          <button className={clsx('nav-item', view === 'lyrics' && 'is-active')} aria-current={view === 'lyrics' ? 'page' : undefined} onClick={() => setView('lyrics')}><ListMusic size={18} /><span>歌词显示</span></button>
          <button className={clsx('nav-item', view === 'connections' && 'is-active')} aria-current={view === 'connections' ? 'page' : undefined} onClick={() => setView('connections')}><Users size={18} /><span>连接设备</span><em>{devices.length}</em></button>
          <button className={clsx('nav-item', view === 'settings' && 'is-active')} aria-current={view === 'settings' ? 'page' : undefined} onClick={() => setView('settings')}><Settings2 size={18} /><span>设置</span></button>
        </nav>
        <div className="sidebar-status">
          <div className={clsx('status-light', connected && 'is-online')} />
          <div><strong>{runtime.role === 'host' ? '正在提供同步服务' : connectionLabel}</strong><span>{runtime.serverAddress ?? '等待网络地址'}</span></div>
        </div>
      </aside>

      <section className="workspace">
        <header className="topbar">
          <div><span className="eyebrow">{runtime.role === 'host' ? '主机控制台' : connected ? `已连接 ${runtime.serverName}` : '局域网客户端'}</span><h1>{view === 'lyrics' ? '同步歌词' : view === 'connections' ? '连接设备' : view === 'settings' ? '显示设置' : runtime.role === 'host' ? '办公室同步播放' : '同步收听'}</h1></div>
          <div className="topbar-actions">
            <div className={clsx('connection-pill', connected && 'is-online')}><Wifi size={15} />{runtime.role === 'host' ? '主机在线' : connectionLabel}</div>
            {runtime.role === 'client' && <IconButton label="重新自动发现主机" size="sm" disabled={busy} onClick={() => void startAutomaticDiscovery()}><RefreshCw size={17} /></IconButton>}
          </div>
        </header>

        {error && <div className="error-banner" role="alert">{error}</div>}
        {runtime.role === 'client' && runtime.connectionError && view !== 'connections' && (
          <button className="connection-error-banner" type="button" onClick={() => setView('connections')}>
            <TriangleAlert size={17} />
            <span><strong>{runtime.connectionError.title}</strong><small>{runtime.connectionError.message}</small></span>
            <ChevronRight size={16} />
          </button>
        )}

        {view === 'player' ? <div className="content-grid">
          <section className="now-playing" aria-label="当前播放">
            <div className="track-summary">
              <div className="cover-wrap">
                <img src={playback.coverUrl ?? '/app-icon.png'} alt="当前歌曲封面" />
                <div className={clsx('playing-bars', playback.playing && 'is-playing')} aria-hidden="true"><i /><i /><i /></div>
              </div>
              <div className="track-copy">
                <span className="source-label"><Headphones size={14} />{playback.sourceApp ?? '等待 QQ 音乐'}</span>
                <h2>{playback.title ?? '未检测到歌曲'}</h2>
                <p>{playback.artist ?? '请在主机上打开 QQ 音乐'}<span>·</span>{playback.album ?? '未知专辑'}</p>
              </div>
            </div>

            <div className="progress-block">
              <div className="progress-track"><span style={{ width: `${progress}%` }} /></div>
              <div className="time-row"><span>{formatTime(positionMs)}</span><span>{formatTime(playback.durationMs)}</span></div>
            </div>

            <div className="transport" aria-label="播放控制">
              <IconButton label="上一首" size="lg" disabled={!playback.capabilities.previous} onClick={() => void sendPlayerCommand('previous')}><SkipBack size={23} /></IconButton>
              <IconButton label={playback.playing ? '暂停' : '播放'} tone="primary" size="lg" disabled={!playback.capabilities.playPause} onClick={() => void sendPlayerCommand('toggle_play_pause')}>{playback.playing ? <CirclePause size={34} /> : <CirclePlay size={34} />}</IconButton>
              <IconButton label="下一首" size="lg" disabled={!playback.capabilities.next} onClick={() => void sendPlayerCommand('next')}><SkipForward size={23} /></IconButton>
            </div>

            <LyricsPanel
              document={lyrics}
              positionMs={positionMs}
              title={playback.title}
              artist={playback.artist}
              mode="compact"
            />
          </section>

          <aside className="right-rail">
            <section className="rail-section">
              <div className="section-heading"><div><Network size={17} /><h3>{runtime.role === 'host' ? '主机网络' : '当前主机'}</h3></div><span>{connected ? '在线' : '离线'}</span></div>
              <div className="host-detail"><strong>{runtime.serverName}</strong><code>{runtime.serverAddress ?? '自动发现中'}</code><p>{runtime.role === 'host' ? '客户端会在同一局域网内自动发现此电脑。' : '连接中断后会自动重新发现并连接。'}</p></div>
            </section>

            {runtime.role === 'host' && <section className="rail-section">
              <div className="section-heading"><div><ShieldCheck size={17} /><h3>控制权限</h3></div></div>
              <label className="toggle-row"><span><strong>允许客户端控制</strong><small>播放、暂停和上下曲</small></span><input type="checkbox" checked={runtime.allowControl} onChange={(event) => void setAllowControl(event.target.checked)} /><i aria-hidden="true" /></label>
            </section>}

            <section className="rail-section peers-section">
              <div className="section-heading"><div><Users size={17} /><h3>{runtime.role === 'host' ? '已连接设备' : '发现的主机'}</h3></div><span>{devices.length}</span></div>
              <div className="device-list">
                {devices.map((item) => <button className="device-row" key={item.id}><span className="device-icon">{runtime.role === 'host' ? <Laptop size={17} /> : <MonitorSpeaker size={17} />}</span><span><strong>{item.name}</strong><small>{item.address}</small></span><ChevronRight size={16} /></button>)}
                {devices.length === 0 && <div className="empty-row">暂无设备</div>}
              </div>
            </section>
          </aside>
        </div> : view === 'lyrics' ? <div className="lyrics-focus-layout">
          <LyricsPanel
            document={lyrics}
            positionMs={positionMs}
            title={playback.title}
            artist={playback.artist}
            mode="focus"
          />
        </div> : view === 'connections' ? <ConnectionManager
          runtime={runtime}
          busy={busy}
          connectManualHost={connectManualHost}
          startAutomaticDiscovery={startAutomaticDiscovery}
          discoverHosts={discoverHosts}
        /> : <DesktopLyricsSettingsView
          settings={desktopLyrics.settings}
          document={lyrics}
          positionMs={positionMs}
          title={playback.title}
          artist={playback.artist}
          error={desktopLyricsError}
          update={desktopLyrics.update}
          reset={desktopLyrics.reset}
        />}
      </section>
    </main>
  )
}

export default App
