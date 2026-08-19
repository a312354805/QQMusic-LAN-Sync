import {
  AlertTriangle,
  Cable,
  ChevronRight,
  CircleCheck,
  Laptop,
  MonitorSpeaker,
  Radar,
  RefreshCw,
  Search,
  ShieldAlert,
  WifiOff,
} from 'lucide-react'
import clsx from 'clsx'
import { useEffect, useState, type FormEvent } from 'react'
import type { ConnectionIssueKind, RuntimeStatus } from '../shared/types'
import './ConnectionManager.scss'

type Props = {
  runtime: RuntimeStatus
  busy: boolean
  connectManualHost: (address: string) => Promise<void>
  startAutomaticDiscovery: () => Promise<void>
  discoverHosts: () => Promise<void>
}

const savedAddressKey = 'qqmusic-lan-sync:last-manual-host'

const connectionLabels: Record<RuntimeStatus['connection'], string> = {
  idle: '尚未启动',
  discovering: '正在发现主机',
  connecting: '正在建立连接',
  connected: '已连接',
  offline: '连接失败',
}

const issueIcons: Partial<Record<ConnectionIssueKind, typeof AlertTriangle>> = {
  discovery_timeout: Radar,
  discovery_failed: WifiOff,
  connection_timeout: ShieldAlert,
  connection_refused: Cable,
  network_unreachable: WifiOff,
  firewall_or_security: ShieldAlert,
  protocol_mismatch: AlertTriangle,
  disconnected: Cable,
}

const loadSavedAddress = () => {
  try {
    return window.localStorage.getItem(savedAddressKey) ?? ''
  } catch {
    return ''
  }
}

export function ConnectionManager({
  runtime,
  busy,
  connectManualHost,
  startAutomaticDiscovery,
  discoverHosts,
}: Props) {
  const [address, setAddress] = useState(loadSavedAddress)
  const [validationError, setValidationError] = useState<string | null>(null)
  const connected = runtime.connection === 'connected'
  const issue = runtime.connectionError
  const IssueIcon = issue ? issueIcons[issue.kind] ?? AlertTriangle : AlertTriangle

  useEffect(() => {
    if (runtime.preferredServerAddress) setAddress(runtime.preferredServerAddress)
  }, [runtime.preferredServerAddress])

  const connect = async (event: FormEvent) => {
    event.preventDefault()
    const value = address.trim()
    if (!value) {
      setValidationError('请输入主机 IP 地址或计算机名')
      return
    }
    setValidationError(null)
    try {
      window.localStorage.setItem(savedAddressKey, value)
    } catch {
      // Remembering the address is optional.
    }
    await connectManualHost(value)
  }

  return (
    <div className="connection-manager">
      <header className="connection-overview">
        <span className={clsx('connection-overview__icon', connected && 'is-online')}>
          {connected ? <CircleCheck size={23} /> : <Radar size={23} />}
        </span>
        <div>
          <span>客户端连接</span>
          <h2>{runtime.role === 'host' ? '本机正在提供同步服务' : connectionLabels[runtime.connection]}</h2>
          <p>{runtime.serverAddress ?? '尚未选择局域网主机'}</p>
        </div>
        {runtime.role === 'client' && (
          <span className={clsx('connection-mode', runtime.preferredServerAddress && 'is-manual')}>
            {runtime.preferredServerAddress ? '手动地址' : '自动发现'}
          </span>
        )}
      </header>

      {issue && runtime.role === 'client' && (
        <section className="connection-diagnostic" aria-live="polite" aria-atomic="true">
          <IssueIcon size={20} />
          <div>
            <h3>{issue.title}</h3>
            <p>{issue.message}</p>
            <strong>{issue.suggestion}</strong>
            {issue.endpoint && <code>{issue.endpoint}</code>}
            {issue.detail && <details><summary>查看技术详情</summary><pre>{issue.detail}</pre></details>}
          </div>
        </section>
      )}

      {runtime.role === 'host' ? (
        <div className="connection-columns">
          <section className="connection-section">
            <div className="connection-section__heading"><div><MonitorSpeaker size={18} /><h3>主机地址</h3></div></div>
            <div className="host-address-display">
              <code>{runtime.serverAddress ?? '正在获取局域网地址'}</code>
              <p>客户端可自动发现本机，也可以直接输入此地址连接。</p>
            </div>
          </section>
          <section className="connection-section">
            <div className="connection-section__heading"><div><Laptop size={18} /><h3>已连接设备</h3></div><span>{runtime.peers.length}</span></div>
            <div className="connection-device-list">
              {runtime.peers.map((peer) => (
                <div className="connection-device" key={peer.id}>
                  <span><Laptop size={17} /></span>
                  <div><strong>{peer.name}</strong><small>{peer.address}</small></div>
                  <em>{peer.canControl ? '可控制' : '只读'}</em>
                </div>
              ))}
              {runtime.peers.length === 0 && <div className="connection-empty">还没有客户端连接</div>}
            </div>
          </section>
        </div>
      ) : (
        <div className="connection-columns">
          <section className="connection-section">
            <div className="connection-section__heading"><div><Cable size={18} /><h3>手动连接</h3></div></div>
            <form className="manual-connect-form" onSubmit={(event) => void connect(event)}>
              <label htmlFor="manual-host-address">主机 IP 或计算机名</label>
              <div>
                <input
                  id="manual-host-address"
                  value={address}
                  onChange={(event) => { setAddress(event.target.value); setValidationError(null) }}
                  placeholder="例如 192.168.0.9"
                  spellCheck={false}
                  autoComplete="off"
                  disabled={busy}
                />
                <button type="submit" disabled={busy}><Cable size={16} />连接</button>
              </div>
              <small>未填写端口时默认使用 17636，也支持 192.168.0.9:17636。</small>
              {validationError && <p role="alert">{validationError}</p>}
            </form>
            <button className="automatic-discovery-button" type="button" disabled={busy} onClick={() => void startAutomaticDiscovery()}>
              <Radar size={17} /><span><strong>使用自动发现</strong><small>清除固定地址并在所有有效网卡上重新搜索</small></span><RefreshCw size={15} />
            </button>
          </section>

          <section className="connection-section">
            <div className="connection-section__heading">
              <div><Search size={18} /><h3>发现的主机</h3></div>
              <button type="button" disabled={busy} title="重新扫描" aria-label="重新扫描局域网主机" onClick={() => void discoverHosts()}><RefreshCw size={15} /></button>
            </div>
            <div className="connection-device-list">
              {runtime.hosts.map((host) => (
                <button className="connection-device is-action" type="button" key={host.id} disabled={busy} onClick={() => void connectManualHost(`${host.address}:${host.port}`)}>
                  <span><MonitorSpeaker size={17} /></span>
                  <div><strong>{host.name}</strong><small>{host.address}:{host.port}{host.latencyMs == null ? '' : ` · ${host.latencyMs} ms`}</small></div>
                  <ChevronRight size={16} />
                </button>
              ))}
              {runtime.hosts.length === 0 && <div className="connection-empty">暂未发现主机，可直接输入主机 IP</div>}
            </div>
          </section>
        </div>
      )}
    </div>
  )
}
