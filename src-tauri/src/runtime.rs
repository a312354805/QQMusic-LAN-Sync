use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use tauri::{AppHandle, Emitter};
use tokio::sync::{mpsc, Mutex};
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

use crate::lyrics::LyricsService;
use crate::models::{
    AppRole, ConnectionIssue, ConnectionIssueKind, ConnectionState, HostInfo, PlayerCommand,
    RuntimeStatus,
};
use crate::network::protocol::ClientMessage;
use crate::network::{client, discovery, host};
use crate::player::PlayerService;
use crate::storage::Storage;

pub struct RuntimeManager {
    pub status: Arc<RwLock<RuntimeStatus>>,
    player: Arc<PlayerService>,
    lyrics: Arc<LyricsService>,
    host_id: String,
    lifecycle: Mutex<Option<RuntimeTask>>,
    generation: Arc<AtomicU64>,
    client_sender: RwLock<Option<mpsc::UnboundedSender<ClientMessage>>>,
}

struct RuntimeTask {
    cancel: CancellationToken,
    handle: tauri::async_runtime::JoinHandle<()>,
}

impl RuntimeManager {
    pub fn new(app_dir: PathBuf) -> Result<Arc<Self>, String> {
        let storage = Arc::new(Storage::open(app_dir)?);
        let lyrics = Arc::new(LyricsService::new(storage)?);
        Ok(Arc::new(Self {
            status: Arc::new(RwLock::new(RuntimeStatus::default())),
            player: Arc::new(PlayerService::default()),
            lyrics,
            host_id: Uuid::new_v4().to_string(),
            lifecycle: Mutex::new(None),
            generation: Arc::new(AtomicU64::new(0)),
            client_sender: RwLock::new(None),
        }))
    }

    pub async fn start_role(self: &Arc<Self>, app: AppHandle, role: AppRole) -> RuntimeStatus {
        self.start_role_with_host(app, role, None).await
    }

    async fn start_role_with_host(
        self: &Arc<Self>,
        app: AppHandle,
        role: AppRole,
        preferred_host: Option<HostInfo>,
    ) -> RuntimeStatus {
        let mut lifecycle = self.lifecycle.lock().await;
        stop_runtime_task(lifecycle.take()).await;

        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let cancel = CancellationToken::new();
        {
            let mut status = self
                .status
                .write()
                .unwrap_or_else(|error| error.into_inner());
            status.role = role;
            status.connection = match role {
                AppRole::Host => ConnectionState::Connecting,
                AppRole::Client => ConnectionState::Discovering,
            };
            status.connection_error = None;
            status.preferred_server_address = preferred_host
                .as_ref()
                .map(crate::network::client::host_endpoint);
            status.server_address = preferred_host
                .as_ref()
                .map(crate::network::client::host_endpoint);
            status.peers.clear();
            status.hosts = if role == AppRole::Client {
                preferred_host.iter().cloned().collect()
            } else {
                Vec::new()
            };
        }

        let handle = match role {
            AppRole::Host => {
                *self
                    .client_sender
                    .write()
                    .unwrap_or_else(|error| error.into_inner()) = None;
                let runtime = self.status.clone();
                let player = self.player.clone();
                let lyrics = self.lyrics.clone();
                let host_id = self.host_id.clone();
                let generation_counter = generation;
                let current_generation = self.generation.clone();
                let task_cancel = cancel.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(error) = host::run(
                        app.clone(),
                        runtime.clone(),
                        player,
                        lyrics,
                        host_id,
                        task_cancel,
                    )
                    .await
                    {
                        log::error!("Host runtime stopped: {error}");
                        if current_generation.load(Ordering::SeqCst) == generation_counter {
                            let status = {
                                let mut status = runtime
                                    .write()
                                    .unwrap_or_else(|failure| failure.into_inner());
                                status.connection = ConnectionState::Offline;
                                status.connection_error = Some(ConnectionIssue::new(
                                    ConnectionIssueKind::Other,
                                    "主机服务启动失败",
                                    "同步端口或自动发现服务未能正常启动。",
                                    "稍后重新切换到主机模式，并检查端口 17635 和 17636 是否被其它程序占用。",
                                    Some(error),
                                    status.server_address.clone(),
                                ));
                                status.clone()
                            };
                            let _ = app.emit("runtime://status", status);
                        }
                    }
                })
            }
            AppRole::Client => {
                let (sender, receiver) = mpsc::unbounded_channel();
                *self
                    .client_sender
                    .write()
                    .unwrap_or_else(|error| error.into_inner()) = Some(sender);
                let runtime = self.status.clone();
                let generation_counter = generation;
                let current_generation = self.generation.clone();
                let task_cancel = cancel.clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(error) = client::run(
                        app.clone(),
                        runtime.clone(),
                        receiver,
                        preferred_host,
                        task_cancel,
                    )
                    .await
                    {
                        log::error!("Client runtime stopped: {error}");
                        if current_generation.load(Ordering::SeqCst) != generation_counter {
                            return;
                        }
                        let status = {
                            let mut status = runtime
                                .write()
                                .unwrap_or_else(|failure| failure.into_inner());
                            status.connection = ConnectionState::Offline;
                            status.connection_error = Some(ConnectionIssue::new(
                                ConnectionIssueKind::Other,
                                "客户端连接任务已停止",
                                "连接主机时发生了未处理的错误。",
                                "重新连接；如果问题持续出现，请记录错误详情。",
                                Some(error),
                                status.server_address.clone(),
                            ));
                            status.clone()
                        };
                        let _ = app.emit("runtime://status", status);
                    }
                })
            }
        };
        *lifecycle = Some(RuntimeTask { cancel, handle });
        self.snapshot()
    }

    pub async fn connect_manual_host(
        self: &Arc<Self>,
        app: AppHandle,
        address: &str,
    ) -> Result<RuntimeStatus, String> {
        let host = client::parse_manual_host(address)?;
        Ok(self
            .start_role_with_host(app, AppRole::Client, Some(host))
            .await)
    }

    pub async fn start_automatic_discovery(self: &Arc<Self>, app: AppHandle) -> RuntimeStatus {
        self.start_role_with_host(app, AppRole::Client, None).await
    }

    pub fn snapshot(&self) -> RuntimeStatus {
        self.status
            .read()
            .unwrap_or_else(|error| error.into_inner())
            .clone()
    }

    pub fn set_allow_control(&self, allow: bool) -> RuntimeStatus {
        let mut status = self
            .status
            .write()
            .unwrap_or_else(|error| error.into_inner());
        status.allow_control = allow;
        for peer in &mut status.peers {
            peer.can_control = allow;
        }
        status.clone()
    }

    pub async fn send_player_command(&self, command: PlayerCommand) -> Result<(), String> {
        match self.snapshot().role {
            AppRole::Host => {
                let player = self.player.clone();
                tauri::async_runtime::spawn_blocking(move || player.execute(command))
                    .await
                    .map_err(|error| error.to_string())?
            }
            AppRole::Client => {
                let sender = self
                    .client_sender
                    .read()
                    .unwrap_or_else(|error| error.into_inner())
                    .clone()
                    .ok_or_else(|| "客户端尚未连接主机".to_string())?;
                sender
                    .send(ClientMessage::Command {
                        request_id: Uuid::new_v4().to_string(),
                        command,
                    })
                    .map_err(|_| "客户端连接已经断开".to_string())
            }
        }
    }

    pub async fn discover_hosts(&self) -> Result<Vec<HostInfo>, String> {
        let client_name = hostname::get()
            .ok()
            .and_then(|name| name.into_string().ok())
            .unwrap_or_else(|| "Windows 客户端".into());
        let hosts = discovery::discover_hosts(&client_name).await?;
        self.status
            .write()
            .unwrap_or_else(|error| error.into_inner())
            .hosts = hosts.clone();
        Ok(hosts)
    }
}

async fn stop_runtime_task(previous: Option<RuntimeTask>) {
    let Some(previous) = previous else {
        return;
    };
    previous.cancel.cancel();
    let mut handle = previous.handle;
    if tokio::time::timeout(Duration::from_secs(2), &mut handle)
        .await
        .is_err()
    {
        log::warn!("Previous runtime task did not stop in time; aborting it");
        handle.abort();
        let _ = handle.await;
    }
}
