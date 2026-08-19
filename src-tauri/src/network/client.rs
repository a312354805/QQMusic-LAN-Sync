use std::io::ErrorKind;
use std::sync::{Arc, RwLock};
use std::time::Duration;

use futures_util::{SinkExt, StreamExt};
use tauri::{AppHandle, Emitter};
use tokio::net::TcpStream;
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::{Error as WebSocketError, Message};
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use tokio_util::sync::CancellationToken;
use url::Url;

use crate::models::{
    now_ms, AppRole, ConnectionIssue, ConnectionIssueKind, ConnectionState, HostInfo, RuntimeStatus,
};

use super::discovery;
use super::protocol::{ClientMessage, ServerMessage};
use super::WS_PORT;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(4);
const AUTO_RETRY_DELAY: Duration = Duration::from_secs(2);
const MANUAL_RETRY_DELAY: Duration = Duration::from_secs(3);

type ClientSocket = WebSocketStream<MaybeTlsStream<TcpStream>>;

pub async fn run(
    app: AppHandle,
    runtime: Arc<RwLock<RuntimeStatus>>,
    mut outbound: mpsc::UnboundedReceiver<ClientMessage>,
    preferred_host: Option<HostInfo>,
    cancel: CancellationToken,
) -> Result<(), String> {
    let client_name = hostname::get()
        .ok()
        .and_then(|name| name.into_string().ok())
        .unwrap_or_else(|| "Windows 客户端".into());

    loop {
        let hosts = if let Some(host) = &preferred_host {
            let host = host.clone();
            mutate_runtime(&app, &runtime, |status| {
                status.connection = ConnectionState::Connecting;
                status.hosts = vec![host.clone()];
            });
            vec![host]
        } else {
            mutate_runtime(&app, &runtime, |status| {
                status.connection = ConnectionState::Discovering;
                status.server_address = None;
            });
            match discovery::discover_hosts(&client_name).await {
                Ok(hosts) if !hosts.is_empty() => {
                    mutate_runtime(&app, &runtime, |status| status.hosts = hosts.clone());
                    hosts
                }
                Ok(_) => {
                    let issue = discovery_timeout_issue();
                    mutate_runtime(&app, &runtime, |status| {
                        status.connection = ConnectionState::Offline;
                        status.connection_error = Some(issue);
                        status.hosts.clear();
                    });
                    if wait_before_retry(&cancel, AUTO_RETRY_DELAY).await {
                        return Ok(());
                    }
                    continue;
                }
                Err(error) => {
                    let issue = discovery_failed_issue(error);
                    mutate_runtime(&app, &runtime, |status| {
                        status.connection = ConnectionState::Offline;
                        status.connection_error = Some(issue);
                        status.hosts.clear();
                    });
                    if wait_before_retry(&cancel, AUTO_RETRY_DELAY).await {
                        return Ok(());
                    }
                    continue;
                }
            }
        };

        let mut connected = None::<(HostInfo, ClientSocket)>;
        let mut last_issue = None::<ConnectionIssue>;
        for host in hosts {
            mutate_runtime(&app, &runtime, |status| {
                status.connection = ConnectionState::Connecting;
                status.server_address = Some(host_endpoint(&host));
            });
            match connect_host(&host).await {
                Ok(socket) => {
                    connected = Some((host, socket));
                    break;
                }
                Err(issue) => last_issue = Some(issue),
            }
        }

        let Some((host, socket)) = connected else {
            mutate_runtime(&app, &runtime, |status| {
                status.connection = ConnectionState::Offline;
                status.connection_error = last_issue;
            });
            let delay = if preferred_host.is_some() {
                MANUAL_RETRY_DELAY
            } else {
                AUTO_RETRY_DELAY
            };
            if wait_before_retry(&cancel, delay).await {
                return Ok(());
            }
            continue;
        };

        let endpoint = host_endpoint(&host);
        mutate_runtime(&app, &runtime, |status| {
            status.connection = ConnectionState::Connected;
            status.connection_error = None;
            status.server_address = Some(endpoint.clone());
        });
        let (mut sender, mut receiver) = socket.split();
        if let Err(error) = send(
            &mut sender,
            &ClientMessage::Hello {
                client_name: client_name.clone(),
            },
        )
        .await
        {
            let issue = disconnected_issue(&endpoint, Some(error));
            mutate_runtime(&app, &runtime, |status| {
                status.connection = ConnectionState::Offline;
                status.connection_error = Some(issue);
            });
            continue;
        }

        let mut ping = tokio::time::interval(Duration::from_secs(2));
        let mut clock_offset_ms = 0_i64;
        let disconnect_issue = loop {
            tokio::select! {
                _ = cancel.cancelled() => return Ok(()),
                _ = ping.tick() => {
                    if let Err(error) = send(&mut sender, &ClientMessage::Ping { client_time_ms: now_ms() }).await {
                        break disconnected_issue(&endpoint, Some(error));
                    }
                }
                command = outbound.recv() => {
                    let Some(command) = command else { return Ok(()); };
                    if let Err(error) = send(&mut sender, &command).await {
                        break disconnected_issue(&endpoint, Some(error));
                    }
                }
                incoming = receiver.next() => {
                    let message = match incoming {
                        Some(Ok(message)) => message,
                        Some(Err(error)) => break connection_issue_from_websocket_error(&endpoint, &error),
                        None => break disconnected_issue(&endpoint, None),
                    };
                    let Message::Text(text) = message else {
                        if matches!(message, Message::Close(_)) {
                            break disconnected_issue(&endpoint, None);
                        }
                        continue;
                    };
                    let message = match serde_json::from_str::<ServerMessage>(&text) {
                        Ok(message) => message,
                        Err(error) => break protocol_issue(&endpoint, error.to_string()),
                    };
                    match message {
                        ServerMessage::Runtime(remote) => {
                            let mut remote = *remote;
                            adjust_snapshot_clock(&mut remote, clock_offset_ms);
                            let local = runtime.read().unwrap_or_else(|error| error.into_inner()).clone();
                            remote.role = AppRole::Client;
                            remote.connection = ConnectionState::Connected;
                            remote.connection_error = None;
                            remote.preferred_server_address = local.preferred_server_address;
                            remote.hosts = local.hosts;
                            remote.server_address = Some(endpoint.clone());
                            *runtime.write().unwrap_or_else(|error| error.into_inner()) = remote.clone();
                            let _ = app.emit("runtime://status", &remote);
                            let _ = app.emit("playback://snapshot", &remote.playback);
                            let _ = app.emit("lyrics://document", &remote.lyrics);
                        }
                        ServerMessage::Lyrics(document) => {
                            runtime.write().unwrap_or_else(|error| error.into_inner()).lyrics = document.clone();
                            let _ = app.emit("lyrics://document", document);
                        }
                        ServerMessage::Pong { client_time_ms, server_time_ms } => {
                            let round_trip = now_ms().saturating_sub(client_time_ms);
                            let midpoint = client_time_ms.saturating_add(round_trip / 2);
                            clock_offset_ms = server_time_ms as i64 - midpoint as i64;
                        }
                        ServerMessage::CommandResult { accepted: false, error, .. } => {
                            log::warn!("Remote player command was rejected: {}", error.unwrap_or_default());
                        }
                        ServerMessage::CommandResult { .. } => {}
                    }
                }
            }
        };

        mutate_runtime(&app, &runtime, |status| {
            status.connection = ConnectionState::Offline;
            status.connection_error = Some(disconnect_issue);
        });
        let delay = if preferred_host.is_some() {
            MANUAL_RETRY_DELAY
        } else {
            Duration::from_millis(700)
        };
        if wait_before_retry(&cancel, delay).await {
            return Ok(());
        }
    }
}

pub fn parse_manual_host(input: &str) -> Result<HostInfo, String> {
    let input = input.trim();
    if input.is_empty() {
        return Err("请输入主机 IP 地址，例如 192.168.0.9".into());
    }
    let normalized = if input.contains("://") {
        input.to_owned()
    } else {
        format!("ws://{input}")
    };
    let mut url =
        Url::parse(&normalized).map_err(|error| format!("主机地址格式不正确: {error}"))?;
    match url.scheme() {
        "ws" => {}
        "http" => {
            url.set_scheme("ws")
                .map_err(|_| "无法识别主机地址协议".to_string())?;
        }
        _ => return Err("主机地址只支持 IP、计算机名或 ws:// 地址".into()),
    }
    if !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
    {
        return Err("主机地址不能包含账号、查询参数或片段".into());
    }
    if !matches!(url.path(), "" | "/" | "/ws") {
        return Err("主机地址路径只能是 /ws".into());
    }
    let address = url
        .host_str()
        .ok_or_else(|| "主机地址缺少 IP 或计算机名".to_string())?
        .to_owned();
    let port = url.port().unwrap_or(WS_PORT);
    let endpoint = format_endpoint(&address, port);
    Ok(HostInfo {
        id: format!("manual:{endpoint}"),
        name: format!("手动主机 {endpoint}"),
        address,
        port,
        latency_ms: None,
    })
}

pub fn host_endpoint(host: &HostInfo) -> String {
    format_endpoint(&host.address, host.port)
}

async fn connect_host(host: &HostInfo) -> Result<ClientSocket, ConnectionIssue> {
    let endpoint = host_endpoint(host);
    let url = websocket_url(host);
    match tokio::time::timeout(CONNECT_TIMEOUT, tokio_tungstenite::connect_async(&url)).await {
        Ok(Ok((socket, _))) => Ok(socket),
        Ok(Err(error)) => Err(connection_issue_from_websocket_error(&endpoint, &error)),
        Err(_) => Err(connection_timeout_issue(&endpoint)),
    }
}

async fn send<S>(sender: &mut S, message: &ClientMessage) -> Result<(), String>
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

fn connection_issue_from_websocket_error(
    endpoint: &str,
    error: &WebSocketError,
) -> ConnectionIssue {
    if let WebSocketError::Io(io_error) = error {
        return connection_issue_from_io_error(endpoint, io_error);
    }
    match error {
        WebSocketError::Http(response) => ConnectionIssue::new(
            ConnectionIssueKind::ProtocolMismatch,
            "目标端口不是同步服务",
            format!(
                "{endpoint} 返回了 HTTP {}，但没有完成 WebSocket 握手。",
                response.status()
            ),
            "确认地址和端口正确，并确认目标电脑运行的是相同版本的 QQMusic LAN Sync。",
            Some(error.to_string()),
            Some(endpoint.into()),
        ),
        WebSocketError::Protocol(_) | WebSocketError::Url(_) => {
            protocol_issue(endpoint, error.to_string())
        }
        _ => ConnectionIssue::new(
            ConnectionIssueKind::Other,
            "连接主机失败",
            format!("无法建立到 {endpoint} 的同步连接。"),
            "检查主机地址、服务运行状态以及双方的网络和安全软件设置。",
            Some(error.to_string()),
            Some(endpoint.into()),
        ),
    }
}

fn connection_issue_from_io_error(endpoint: &str, error: &std::io::Error) -> ConnectionIssue {
    let windows_code = error.raw_os_error();
    let kind = error.kind();
    let detail = Some(error.to_string());
    if kind == ErrorKind::ConnectionRefused || windows_code == Some(10061) {
        return ConnectionIssue::new(
            ConnectionIssueKind::ConnectionRefused,
            "同步服务未接受连接",
            format!("电脑 {endpoint} 可以到达，但同步端口拒绝了连接。"),
            "确认主机端软件正在运行且处于主机模式，并检查端口是否被其它程序占用。",
            detail,
            Some(endpoint.into()),
        );
    }
    if kind == ErrorKind::TimedOut || windows_code == Some(10060) {
        return connection_timeout_issue_with_detail(endpoint, detail);
    }
    if matches!(
        kind,
        ErrorKind::NetworkUnreachable | ErrorKind::HostUnreachable
    ) || matches!(windows_code, Some(10051) | Some(10065))
    {
        return ConnectionIssue::new(
            ConnectionIssueKind::NetworkUnreachable,
            "找不到主机网络路径",
            format!("当前电脑没有可用路径连接 {endpoint}。"),
            "确认两台电脑的 IP 和子网掩码，并暂时断开可能改变路由的 VPN 或虚拟网卡。",
            detail,
            Some(endpoint.into()),
        );
    }
    if windows_code == Some(11001) {
        return ConnectionIssue::new(
            ConnectionIssueKind::NetworkUnreachable,
            "无法解析主机名称",
            format!("系统找不到 {endpoint} 对应的网络地址。"),
            "检查计算机名是否正确，或改用主机的 IPv4 地址连接。",
            detail,
            Some(endpoint.into()),
        );
    }
    if kind == ErrorKind::PermissionDenied || windows_code == Some(10013) {
        return ConnectionIssue::new(
            ConnectionIssueKind::FirewallOrSecurity,
            "连接被安全策略阻止",
            format!("系统不允许连接 {endpoint}。"),
            "检查 Windows 防火墙、杀毒软件和公司终端安全策略是否允许 TCP 17636。",
            detail,
            Some(endpoint.into()),
        );
    }
    ConnectionIssue::new(
        ConnectionIssueKind::Other,
        "网络连接失败",
        format!("连接 {endpoint} 时发生网络错误。"),
        "先测试能否 ping 通主机，再使用 Test-NetConnection 检查 TCP 17636。",
        detail,
        Some(endpoint.into()),
    )
}

fn discovery_timeout_issue() -> ConnectionIssue {
    ConnectionIssue::new(
        ConnectionIssueKind::DiscoveryTimeout,
        "没有发现局域网主机",
        "已通过所有有效 IPv4 网卡发送三轮广播，但 3 秒内没有收到响应。",
        "广播可能被 VLAN、无线终端隔离或防火墙拦截；可以在右侧直接输入主机 IP。",
        None,
        None,
    )
}

fn discovery_failed_issue(error: String) -> ConnectionIssue {
    ConnectionIssue::new(
        ConnectionIssueKind::DiscoveryFailed,
        "自动发现启动失败",
        "当前电脑无法正常发送或接收局域网发现广播。",
        "检查网卡是否已连接，以及防火墙或安全软件是否允许 UDP 17635。",
        Some(error),
        None,
    )
}

fn connection_timeout_issue(endpoint: &str) -> ConnectionIssue {
    connection_timeout_issue_with_detail(endpoint, None)
}

fn connection_timeout_issue_with_detail(endpoint: &str, detail: Option<String>) -> ConnectionIssue {
    ConnectionIssue::new(
        ConnectionIssueKind::ConnectionTimeout,
        "连接超时，可能被网络策略拦截",
        format!("{endpoint} 在 4 秒内没有响应 TCP 连接。"),
        "确认主机防火墙允许 TCP 17636，并检查 Wi-Fi 隔离、VLAN 或公司网络 ACL。",
        detail,
        Some(endpoint.into()),
    )
}

fn protocol_issue(endpoint: &str, detail: String) -> ConnectionIssue {
    ConnectionIssue::new(
        ConnectionIssueKind::ProtocolMismatch,
        "同步协议不兼容",
        format!("{endpoint} 已建立连接，但返回了无法识别的数据。"),
        "确认主机和客户端使用相同版本的软件，并确认端口没有指向其它服务。",
        Some(detail),
        Some(endpoint.into()),
    )
}

fn disconnected_issue(endpoint: &str, detail: Option<String>) -> ConnectionIssue {
    ConnectionIssue::new(
        ConnectionIssueKind::Disconnected,
        "与主机的连接已中断",
        format!("{endpoint} 关闭了连接或网络发生了变化。"),
        "软件会自动重连；如果持续失败，请检查主机是否退出以及网络是否切换。",
        detail,
        Some(endpoint.into()),
    )
}

fn websocket_url(host: &HostInfo) -> String {
    format!("ws://{}/ws", host_endpoint(host))
}

fn format_endpoint(address: &str, port: u16) -> String {
    if address.contains(':') && !address.starts_with('[') {
        format!("[{address}]:{port}")
    } else {
        format!("{address}:{port}")
    }
}

fn adjust_snapshot_clock(runtime: &mut RuntimeStatus, clock_offset_ms: i64) {
    runtime.playback.observed_at_ms =
        (runtime.playback.observed_at_ms as i64 - clock_offset_ms).max(0) as u64;
}

fn mutate_runtime<F>(app: &AppHandle, runtime: &Arc<RwLock<RuntimeStatus>>, update: F)
where
    F: FnOnce(&mut RuntimeStatus),
{
    let status = {
        let mut status = runtime.write().unwrap_or_else(|error| error.into_inner());
        update(&mut status);
        status.clone()
    };
    let _ = app.emit("runtime://status", status);
}

async fn wait_before_retry(cancel: &CancellationToken, duration: Duration) -> bool {
    tokio::select! {
        _ = cancel.cancelled() => true,
        _ = tokio::time::sleep(duration) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_manual_ip_and_optional_port() {
        let default_port = parse_manual_host("192.168.0.9").unwrap();
        assert_eq!(default_port.address, "192.168.0.9");
        assert_eq!(default_port.port, WS_PORT);

        let custom_port = parse_manual_host("ws://office-pc:18000/ws").unwrap();
        assert_eq!(custom_port.address, "office-pc");
        assert_eq!(custom_port.port, 18_000);
    }

    #[test]
    fn rejects_manual_address_with_unexpected_path() {
        let error = parse_manual_host("192.168.0.9/not-sync").unwrap_err();
        assert!(error.contains("/ws"));
    }

    #[test]
    fn classifies_refused_and_timeout_errors() {
        let refused = std::io::Error::from(ErrorKind::ConnectionRefused);
        assert_eq!(
            connection_issue_from_io_error("192.168.0.9:17636", &refused).kind,
            ConnectionIssueKind::ConnectionRefused
        );
        let timeout = std::io::Error::from(ErrorKind::TimedOut);
        assert_eq!(
            connection_issue_from_io_error("192.168.0.9:17636", &timeout).kind,
            ConnectionIssueKind::ConnectionTimeout
        );
    }
}
