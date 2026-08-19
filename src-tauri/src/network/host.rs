use std::net::SocketAddr;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{ConnectInfo, State};
use axum::response::IntoResponse;
use axum::routing::get;
use axum::Router;
use futures_util::{SinkExt, StreamExt};
use tauri::{AppHandle, Emitter};
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::lyrics::LyricsService;
use crate::models::{now_ms, ConnectionState, PeerInfo, RuntimeStatus};
use crate::player::PlayerService;

use super::discovery;
use super::protocol::{ClientMessage, ServerMessage};
use super::WS_PORT;

#[derive(Clone)]
struct HostState {
    app: AppHandle,
    runtime: Arc<RwLock<RuntimeStatus>>,
    messages: broadcast::Sender<ServerMessage>,
    player: Arc<PlayerService>,
    cancel: CancellationToken,
}

pub async fn run(
    app: AppHandle,
    runtime: Arc<RwLock<RuntimeStatus>>,
    player: Arc<PlayerService>,
    lyrics: Arc<LyricsService>,
    host_id: String,
    cancel: CancellationToken,
) -> Result<(), String> {
    {
        let mut status = runtime.write().unwrap_or_else(|error| error.into_inner());
        status.connection = ConnectionState::Connected;
        status.server_address = local_ip_address::local_ip()
            .ok()
            .map(|address| format!("{address}:{WS_PORT}"));
    }
    emit_runtime(&app, &runtime);
    let (messages, _) = broadcast::channel(64);
    let state = HostState {
        app: app.clone(),
        runtime: runtime.clone(),
        messages: messages.clone(),
        player: player.clone(),
        cancel: cancel.clone(),
    };
    let server_cancel = cancel.child_token();
    let discovery_cancel = cancel.child_token();
    let monitor_cancel = cancel.child_token();
    let server = run_server(state, server_cancel);
    let discovery = discovery::run_responder(runtime.clone(), host_id, discovery_cancel);
    let monitor = monitor_player(app, runtime, player, lyrics, messages, monitor_cancel);
    tokio::select! {
        result = server => result,
        result = discovery => result,
        result = monitor => result,
        _ = cancel.cancelled() => Ok(()),
    }
}

async fn run_server(state: HostState, cancel: CancellationToken) -> Result<(), String> {
    let router = Router::new()
        .route("/health", get(|| async { "ok" }))
        .route("/ws", get(ws_upgrade))
        .with_state(state);
    let listener = TcpListener::bind(("0.0.0.0", WS_PORT))
        .await
        .map_err(|error| format!("无法绑定同步端口 {WS_PORT}: {error}"))?;
    axum::serve(
        listener,
        router.into_make_service_with_connect_info::<SocketAddr>(),
    )
    .with_graceful_shutdown(cancel.cancelled_owned())
    .await
    .map_err(|error| error.to_string())
}

async fn ws_upgrade(
    websocket: WebSocketUpgrade,
    ConnectInfo(address): ConnectInfo<SocketAddr>,
    State(state): State<HostState>,
) -> impl IntoResponse {
    websocket.on_upgrade(move |socket| handle_socket(socket, address, state))
}

async fn handle_socket(socket: WebSocket, address: SocketAddr, state: HostState) {
    let peer_id = Uuid::new_v4().to_string();
    {
        let mut runtime = state
            .runtime
            .write()
            .unwrap_or_else(|error| error.into_inner());
        let can_control = runtime.allow_control;
        runtime.peers.push(PeerInfo {
            id: peer_id.clone(),
            name: "局域网客户端".into(),
            address: address.ip().to_string(),
            connected_at_ms: now_ms(),
            can_control,
        });
    }
    publish_runtime(&state);
    let (mut sender, mut receiver) = socket.split();
    let initial = state
        .runtime
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .clone();
    let _ = send_message(&mut sender, &ServerMessage::Runtime(Box::new(initial))).await;
    let mut subscription = state.messages.subscribe();
    let socket_cancel = state.cancel.clone();

    loop {
        tokio::select! {
            _ = socket_cancel.cancelled() => break,
            broadcast = subscription.recv() => {
                let Ok(message) = broadcast else { continue; };
                if send_message(&mut sender, &message).await.is_err() { break; }
            }
            incoming = receiver.next() => {
                let Some(Ok(Message::Text(text))) = incoming else { break; };
                let Ok(message) = serde_json::from_str::<ClientMessage>(&text) else { continue; };
                match message {
                    ClientMessage::Hello { client_name } => {
                        let mut runtime = state.runtime.write().unwrap_or_else(|error| error.into_inner());
                        if let Some(peer) = runtime.peers.iter_mut().find(|peer| peer.id == peer_id) { peer.name = client_name; }
                        drop(runtime);
                        publish_runtime(&state);
                    }
                    ClientMessage::Ping { client_time_ms } => {
                        let _ = send_message(&mut sender, &ServerMessage::Pong { client_time_ms, server_time_ms: now_ms() }).await;
                    }
                    ClientMessage::Command { request_id, command } => {
                        let allowed = state.runtime.read().unwrap_or_else(|error| error.into_inner()).allow_control;
                        let result = if allowed {
                            let player = state.player.clone();
                            tauri::async_runtime::spawn_blocking(move || player.execute(command)).await
                                .unwrap_or_else(|error| Err(error.to_string()))
                        } else { Err("主机已关闭客户端控制权限".into()) };
                        let response = ServerMessage::CommandResult { request_id, accepted: result.is_ok(), error: result.err() };
                        let _ = send_message(&mut sender, &response).await;
                    }
                }
            }
        }
    }
    state
        .runtime
        .write()
        .unwrap_or_else(|error| error.into_inner())
        .peers
        .retain(|peer| peer.id != peer_id);
    publish_runtime(&state);
}

async fn send_message<S>(sender: &mut S, message: &ServerMessage) -> Result<(), String>
where
    S: futures_util::Sink<Message> + Unpin,
    S::Error: std::fmt::Display,
{
    let payload = serde_json::to_string(message).map_err(|error| error.to_string())?;
    sender
        .send(Message::Text(payload.into()))
        .await
        .map_err(|error| error.to_string())
}

async fn monitor_player(
    app: AppHandle,
    runtime: Arc<RwLock<RuntimeStatus>>,
    player: Arc<PlayerService>,
    lyrics: Arc<LyricsService>,
    messages: broadcast::Sender<ServerMessage>,
    cancel: CancellationToken,
) -> Result<(), String> {
    let mut active_track = None::<String>;
    let mut lyrics_loaded_for = None::<String>;
    let mut lyric_retry_at = tokio::time::Instant::now();
    loop {
        let player_for_query = player.clone();
        let mut snapshot =
            tauri::async_runtime::spawn_blocking(move || player_for_query.snapshot())
                .await
                .map_err(|error| error.to_string())?;
        let sequence = runtime
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .playback
            .sequence
            + 1;
        snapshot.sequence = sequence;
        let next_track = snapshot.track_key.clone();
        {
            runtime
                .write()
                .unwrap_or_else(|error| error.into_inner())
                .playback = snapshot.clone();
        }
        let _ = app.emit("playback://snapshot", &snapshot);
        let _ = messages.send(ServerMessage::Runtime(Box::new(
            runtime
                .read()
                .unwrap_or_else(|error| error.into_inner())
                .clone(),
        )));
        if next_track != active_track {
            active_track = next_track.clone();
            lyrics_loaded_for = None;
            lyric_retry_at = tokio::time::Instant::now();
            runtime
                .write()
                .unwrap_or_else(|error| error.into_inner())
                .lyrics = None;
            let _ = app.emit(
                "lyrics://document",
                Option::<crate::models::LyricsDocument>::None,
            );
            let _ = messages.send(ServerMessage::Lyrics(None));
        }
        if active_track.is_some()
            && lyrics_loaded_for != active_track
            && tokio::time::Instant::now() >= lyric_retry_at
        {
            match lyrics.get_or_search(&snapshot).await {
                Ok(document) => {
                    runtime
                        .write()
                        .unwrap_or_else(|error| error.into_inner())
                        .lyrics = Some(document.clone());
                    let _ = app.emit("lyrics://document", &document);
                    let _ = messages.send(ServerMessage::Lyrics(Some(document)));
                    lyrics_loaded_for = active_track.clone();
                }
                Err(error) => {
                    log::warn!("Lyrics lookup failed for {:?}: {error}", active_track);
                    lyric_retry_at = tokio::time::Instant::now() + Duration::from_secs(10);
                }
            }
        }
        tokio::select! {
            _ = cancel.cancelled() => return Ok(()),
            _ = tokio::time::sleep(Duration::from_secs(1)) => {}
        }
    }
}

fn publish_runtime(state: &HostState) {
    emit_runtime(&state.app, &state.runtime);
    let runtime = state
        .runtime
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .clone();
    let _ = state
        .messages
        .send(ServerMessage::Runtime(Box::new(runtime)));
}

fn emit_runtime(app: &AppHandle, runtime: &Arc<RwLock<RuntimeStatus>>) {
    let status = runtime
        .read()
        .unwrap_or_else(|error| error.into_inner())
        .clone();
    let _ = app.emit("runtime://status", status);
}
