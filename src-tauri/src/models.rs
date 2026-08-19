use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum AppRole {
    #[default]
    Host,
    Client,
}

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionState {
    #[default]
    Idle,
    Discovering,
    Connecting,
    Connected,
    Offline,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ConnectionIssueKind {
    DiscoveryTimeout,
    DiscoveryFailed,
    ConnectionTimeout,
    ConnectionRefused,
    NetworkUnreachable,
    FirewallOrSecurity,
    ProtocolMismatch,
    Disconnected,
    Other,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ConnectionIssue {
    pub kind: ConnectionIssueKind,
    pub title: String,
    pub message: String,
    pub suggestion: String,
    pub detail: Option<String>,
    pub endpoint: Option<String>,
    pub occurred_at_ms: u64,
}

impl ConnectionIssue {
    pub fn new(
        kind: ConnectionIssueKind,
        title: impl Into<String>,
        message: impl Into<String>,
        suggestion: impl Into<String>,
        detail: Option<String>,
        endpoint: Option<String>,
    ) -> Self {
        Self {
            kind,
            title: title.into(),
            message: message.into(),
            suggestion: suggestion.into(),
            detail,
            endpoint,
            occurred_at_ms: now_ms(),
        }
    }
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PlayerCommand {
    TogglePlayPause,
    Previous,
    Next,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackCapabilities {
    pub play_pause: bool,
    pub previous: bool,
    pub next: bool,
    pub seek: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackSnapshot {
    pub sequence: u64,
    pub track_key: Option<String>,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub cover_url: Option<String>,
    pub source_app: Option<String>,
    pub duration_ms: Option<u64>,
    pub position_ms: Option<u64>,
    pub observed_at_ms: u64,
    pub playing: bool,
    pub capabilities: PlaybackCapabilities,
    pub error: Option<String>,
}

impl Default for PlaybackSnapshot {
    fn default() -> Self {
        Self {
            sequence: 0,
            track_key: None,
            title: None,
            artist: None,
            album: None,
            cover_url: None,
            source_app: None,
            duration_ms: None,
            position_ms: None,
            observed_at_ms: now_ms(),
            playing: false,
            capabilities: PlaybackCapabilities::default(),
            error: Some("等待 QQ 音乐媒体会话".into()),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LyricsWord {
    pub text: String,
    pub start_ms: u64,
    pub end_ms: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LyricsLine {
    pub text: String,
    pub start_ms: u64,
    pub end_ms: Option<u64>,
    pub words: Option<Vec<LyricsWord>>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct LyricsTrack {
    pub lines: Vec<LyricsLine>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LyricsDocument {
    pub track_key: String,
    pub source: String,
    pub offset_ms: i64,
    pub original: LyricsTrack,
    pub translation: Option<LyricsTrack>,
    pub romanization: Option<LyricsTrack>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PeerInfo {
    pub id: String,
    pub name: String,
    pub address: String,
    pub connected_at_ms: u64,
    pub can_control: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct HostInfo {
    pub id: String,
    pub name: String,
    pub address: String,
    pub port: u16,
    pub latency_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeStatus {
    pub role: AppRole,
    pub connection: ConnectionState,
    #[serde(default)]
    pub connection_error: Option<ConnectionIssue>,
    #[serde(default)]
    pub preferred_server_address: Option<String>,
    pub server_name: String,
    pub server_address: Option<String>,
    pub allow_control: bool,
    pub peers: Vec<PeerInfo>,
    pub hosts: Vec<HostInfo>,
    pub playback: PlaybackSnapshot,
    pub lyrics: Option<LyricsDocument>,
}

impl Default for RuntimeStatus {
    fn default() -> Self {
        let server_name = hostname::get()
            .ok()
            .and_then(|name| name.into_string().ok())
            .unwrap_or_else(|| "Windows 播放电脑".into());
        let server_address = local_ip_address::local_ip()
            .ok()
            .map(|address| format!("{address}:{}", crate::network::WS_PORT));
        Self {
            role: AppRole::Host,
            connection: ConnectionState::Idle,
            connection_error: None,
            preferred_server_address: None,
            server_name,
            server_address,
            allow_control: true,
            peers: Vec::new(),
            hosts: Vec::new(),
            playback: PlaybackSnapshot::default(),
            lyrics: None,
        }
    }
}

pub fn now_ms() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
