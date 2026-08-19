use serde::{Deserialize, Serialize};

use crate::models::{HostInfo, LyricsDocument, PlayerCommand, RuntimeStatus};

pub const PROTOCOL_VERSION: u16 = 1;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum ServerMessage {
    Runtime(Box<RuntimeStatus>),
    Lyrics(Option<LyricsDocument>),
    CommandResult {
        request_id: String,
        accepted: bool,
        error: Option<String>,
    },
    Pong {
        client_time_ms: u64,
        server_time_ms: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", content = "payload", rename_all = "snake_case")]
pub enum ClientMessage {
    Hello {
        client_name: String,
    },
    Command {
        request_id: String,
        command: PlayerCommand,
    },
    Ping {
        client_time_ms: u64,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryRequest {
    pub protocol_version: u16,
    pub client_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryResponse {
    pub protocol_version: u16,
    pub host: HostInfo,
}
