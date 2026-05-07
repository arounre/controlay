use std::net::UdpSocket;
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};
use std::thread;
use std::time::Duration;

use crate::app::EventProxy;

const BROADCAST_TARGET_PORT: u16 = 8888;

#[derive(serde::Serialize)]
struct DiscoveryMsg {
    name: String,
    port: u16,
}

pub fn start_beacon(
    listening_port: u16,
    stop_signal: Arc<AtomicBool>,
    log_tx: EventProxy,
    host_name: Option<String>,
) {
    thread::spawn(move || {
        let socket = match UdpSocket::bind("0.0.0.0:0") {
            Ok(s) => s,
            Err(e) => {
                log_tx.send(crate::app::UiUpdate::Log(format!("Discovery Error: {}", e)));
                return;
            }
        };

        if let Err(e) = socket.set_broadcast(true) {
            log_tx.send(crate::app::UiUpdate::Log(format!("Discovery Error: {}", e)));
            return;
        }

        let device_name = host_name.unwrap_or_else(|| {
            hostname::get()
                .ok()
                .and_then(|s| s.into_string().ok())
                .unwrap_or_else(|| "Unknown-PC".to_string())
        });

        let msg = DiscoveryMsg {
            name: device_name,
            port: listening_port,
        };

        let payload_bytes = serde_json::to_vec(&msg).unwrap_or_default();
        let target = format!("255.255.255.255:{}", BROADCAST_TARGET_PORT);

        log_tx.send(crate::app::UiUpdate::Log(
            "Broadcasting availability...".to_string(),
        ));

        loop {
            if stop_signal.load(Ordering::Relaxed) {
                log_tx.send(crate::app::UiUpdate::Log("Stopping broadcasts".to_string()));
                break;
            }

            let _ = socket.send_to(&payload_bytes, &target);

            thread::sleep(Duration::from_secs(2));
        }
    });
}
