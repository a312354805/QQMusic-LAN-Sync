use std::collections::HashSet;
use std::net::{IpAddr, Ipv4Addr, SocketAddr, UdpSocket as StdUdpSocket};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use if_addrs::IfAddr;
use tokio::net::UdpSocket;
use tokio::time::{interval, sleep_until, Instant as TokioInstant, MissedTickBehavior};
use tokio_util::sync::CancellationToken;

use crate::models::{HostInfo, RuntimeStatus};

use super::protocol::{DiscoveryRequest, DiscoveryResponse, PROTOCOL_VERSION};
use super::{DISCOVERY_PORT, WS_PORT};

const DISCOVERY_ROUNDS: usize = 3;
const DISCOVERY_ROUND_INTERVAL: Duration = Duration::from_millis(450);
const DISCOVERY_TIMEOUT: Duration = Duration::from_secs(3);

pub async fn run_responder(
    runtime: Arc<RwLock<RuntimeStatus>>,
    host_id: String,
    cancel: CancellationToken,
) -> Result<(), String> {
    let socket = UdpSocket::bind(("0.0.0.0", DISCOVERY_PORT))
        .await
        .map_err(|error| format!("无法绑定局域网发现端口 {DISCOVERY_PORT}: {error}"))?;
    let mut buffer = [0_u8; 2048];
    loop {
        tokio::select! {
            _ = cancel.cancelled() => return Ok(()),
            result = socket.recv_from(&mut buffer) => {
                let (length, peer) = result.map_err(|error| error.to_string())?;
                let Ok(request) = serde_json::from_slice::<DiscoveryRequest>(&buffer[..length]) else { continue; };
                if request.protocol_version != PROTOCOL_VERSION { continue; }
                let status = runtime.read().unwrap_or_else(|error| error.into_inner()).clone();
                let address = local_address_for_peer(peer)
                    .or_else(|| local_ip_address::local_ip().ok())
                    .ok_or_else(|| "无法确定用于回复客户端的局域网地址".to_string())?
                    .to_string();
                let response = DiscoveryResponse {
                    protocol_version: PROTOCOL_VERSION,
                    host: HostInfo { id: host_id.clone(), name: status.server_name, address, port: WS_PORT, latency_ms: None },
                };
                let payload = serde_json::to_vec(&response).map_err(|error| error.to_string())?;
                let _ = socket.send_to(&payload, peer).await;
            }
        }
    }
}

pub async fn discover_hosts(client_name: &str) -> Result<Vec<HostInfo>, String> {
    let socket = UdpSocket::bind(("0.0.0.0", 0))
        .await
        .map_err(|error| format!("无法创建局域网发现套接字: {error}"))?;
    socket
        .set_broadcast(true)
        .map_err(|error| format!("无法启用 UDP 广播: {error}"))?;
    let request = serde_json::to_vec(&DiscoveryRequest {
        protocol_version: PROTOCOL_VERSION,
        client_name: client_name.into(),
    })
    .map_err(|error| error.to_string())?;
    let targets = broadcast_targets();
    let started = Instant::now();
    let deadline = TokioInstant::now() + DISCOVERY_TIMEOUT;
    let mut deadline_sleep = Box::pin(sleep_until(deadline));
    let mut round_timer = interval(DISCOVERY_ROUND_INTERVAL);
    round_timer.set_missed_tick_behavior(MissedTickBehavior::Skip);
    let mut rounds_sent = 0_usize;
    let mut any_packet_sent = false;
    let mut last_send_error = None::<String>;
    let mut hosts = Vec::<HostInfo>::new();
    let mut buffer = [0_u8; 2048];

    loop {
        tokio::select! {
            _ = &mut deadline_sleep => break,
            _ = round_timer.tick(), if rounds_sent < DISCOVERY_ROUNDS => {
                rounds_sent += 1;
                for target in &targets {
                    match socket.send_to(&request, (*target, DISCOVERY_PORT)).await {
                        Ok(_) => any_packet_sent = true,
                        Err(error) => last_send_error = Some(format!("{target}:{DISCOVERY_PORT}: {error}")),
                    }
                }
            }
            received = socket.recv_from(&mut buffer) => {
                let (length, _) = received.map_err(|error| format!("接收局域网发现响应失败: {error}"))?;
                let Ok(mut response) = serde_json::from_slice::<DiscoveryResponse>(&buffer[..length]) else { continue; };
                if response.protocol_version != PROTOCOL_VERSION { continue; }
                response.host.latency_ms = Some(started.elapsed().as_millis() as u64);
                if !hosts.iter().any(|host| host.id == response.host.id) {
                    hosts.push(response.host);
                }
            }
        }
    }

    if !any_packet_sent {
        return Err(format!(
            "所有局域网广播发送均失败{}",
            last_send_error
                .map(|error| format!(": {error}"))
                .unwrap_or_default()
        ));
    }
    Ok(hosts)
}

fn broadcast_targets() -> Vec<Ipv4Addr> {
    let mut targets = HashSet::from([Ipv4Addr::BROADCAST]);
    match if_addrs::get_if_addrs() {
        Ok(interfaces) => {
            for interface in interfaces {
                let IfAddr::V4(address) = interface.addr else {
                    continue;
                };
                if !is_usable_ipv4(address.ip) {
                    continue;
                }
                targets.insert(
                    address
                        .broadcast
                        .unwrap_or_else(|| directed_broadcast(address.ip, address.netmask)),
                );
            }
        }
        Err(error) => log::warn!("Failed to enumerate network interfaces for discovery: {error}"),
    }
    let mut targets = targets.into_iter().collect::<Vec<_>>();
    targets.sort_unstable();
    targets
}

fn directed_broadcast(ip: Ipv4Addr, netmask: Ipv4Addr) -> Ipv4Addr {
    Ipv4Addr::from(u32::from(ip) | !u32::from(netmask))
}

fn is_usable_ipv4(address: Ipv4Addr) -> bool {
    !address.is_loopback()
        && !address.is_unspecified()
        && !address.is_multicast()
        && !address.is_link_local()
}

fn local_address_for_peer(peer: SocketAddr) -> Option<IpAddr> {
    let bind_address = match peer.ip() {
        IpAddr::V4(_) => "0.0.0.0:0",
        IpAddr::V6(_) => "[::]:0",
    };
    let socket = StdUdpSocket::bind(bind_address).ok()?;
    socket.connect(peer).ok()?;
    socket.local_addr().ok().map(|address| address.ip())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn calculates_directed_broadcast_from_subnet_mask() {
        assert_eq!(
            directed_broadcast(
                Ipv4Addr::new(192, 168, 0, 9),
                Ipv4Addr::new(255, 255, 255, 0)
            ),
            Ipv4Addr::new(192, 168, 0, 255)
        );
        assert_eq!(
            directed_broadcast(
                Ipv4Addr::new(10, 12, 35, 9),
                Ipv4Addr::new(255, 255, 240, 0)
            ),
            Ipv4Addr::new(10, 12, 47, 255)
        );
    }
}
