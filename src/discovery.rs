use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::UdpSocket;
use tokio::sync::watch;

use crate::config::AppConfig;

const BROADCAST_TARGET_PORT: u16 = 8888;
const BROADCAST_INTERVAL: Duration = Duration::from_secs(1);

#[derive(serde::Serialize)]
struct DiscoveryMsg<'a> {
    name: &'a str,
    port: u16,
}

pub async fn start_beacon(
    listening_port: u16,
    config_rx: watch::Receiver<AppConfig>,
    mut client_ip_rx: watch::Receiver<Option<SocketAddr>>,
) {
    let Ok(socket) = UdpSocket::bind("0.0.0.0:0").await else {
        return;
    };
    let _ = socket.set_broadcast(true);

    let target = format!("255.255.255.255:{}", BROADCAST_TARGET_PORT);

    let sys_hostname = hostname::get()
        .ok()
        .and_then(|s| s.into_string().ok())
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| "Unknown-PC".to_string());

    let mut cached_name = String::new();
    let mut cached_payload = Vec::new();

    loop {
        if client_ip_rx.borrow().is_some() {
            // Gracefully exit if the server drops the channel lock
            if client_ip_rx.wait_for(|ip| ip.is_none()).await.is_err() {
                break;
            }
            continue;
        }

        {
            let cfg = config_rx.borrow();
            let desired_name = if cfg.use_default_hostname || cfg.hostname.trim().is_empty() {
                sys_hostname.as_str()
            } else {
                cfg.hostname.trim()
            };

            if desired_name != cached_name || cached_payload.is_empty() {
                cached_name = desired_name.to_string();

                let msg = DiscoveryMsg {
                    name: &cached_name,
                    port: listening_port,
                };

                if let Ok(payload) = serde_json::to_vec(&msg) {
                    cached_payload = payload;
                }
            }
        }

        if !cached_payload.is_empty() {
            let _ = socket.send_to(&cached_payload, &target).await;
        }

        tokio::select! {
            _ = tokio::time::sleep(BROADCAST_INTERVAL) => {}
            res = client_ip_rx.changed() => {
                if res.is_err() {
                    break;
                }
            }
        }
    }
}
