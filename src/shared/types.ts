export type AppRole = 'host' | 'client'

export type ConnectionState = 'idle' | 'discovering' | 'connecting' | 'connected' | 'offline'

export type ConnectionIssueKind =
  | 'discovery_timeout'
  | 'discovery_failed'
  | 'connection_timeout'
  | 'connection_refused'
  | 'network_unreachable'
  | 'firewall_or_security'
  | 'protocol_mismatch'
  | 'disconnected'
  | 'other'

export type ConnectionIssue = {
  kind: ConnectionIssueKind
  title: string
  message: string
  suggestion: string
  detail: string | null
  endpoint: string | null
  occurredAtMs: number
}

export type PlayerCommand = 'toggle_play_pause' | 'previous' | 'next'

export type PlaybackCapabilities = {
  playPause: boolean
  previous: boolean
  next: boolean
  seek: boolean
}

export type PlaybackSnapshot = {
  sequence: number
  trackKey: string | null
  title: string | null
  artist: string | null
  album: string | null
  coverUrl: string | null
  sourceApp: string | null
  durationMs: number | null
  positionMs: number | null
  observedAtMs: number
  playing: boolean
  capabilities: PlaybackCapabilities
  error: string | null
}

export type LyricsWord = {
  text: string
  startMs: number
  endMs: number
}

export type LyricsLine = {
  text: string
  startMs: number
  endMs: number | null
  words: LyricsWord[] | null
}

export type LyricsTrack = { lines: LyricsLine[] }

export type LyricsDocument = {
  trackKey: string
  source: string
  offsetMs: number
  original: LyricsTrack
  translation: LyricsTrack | null
  romanization: LyricsTrack | null
}

export type PeerInfo = {
  id: string
  name: string
  address: string
  connectedAtMs: number
  canControl: boolean
}

export type HostInfo = {
  id: string
  name: string
  address: string
  port: number
  latencyMs: number | null
}

export type RuntimeStatus = {
  role: AppRole
  connection: ConnectionState
  connectionError: ConnectionIssue | null
  preferredServerAddress: string | null
  serverName: string
  serverAddress: string | null
  allowControl: boolean
  peers: PeerInfo[]
  hosts: HostInfo[]
  playback: PlaybackSnapshot
  lyrics: LyricsDocument | null
}
