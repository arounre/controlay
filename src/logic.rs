use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use anyhow::{Context, Result};
use tokio::net::UdpSocket;
use tokio::sync::{broadcast, mpsc, watch};
use vigem_rust::TargetHandle;
use vigem_rust::client::Client;
use vigem_rust::target::{DualShock4, Xbox360};

use crate::config::{AppConfig, ControllerType, RawDeadzone};
use crate::core::{AppEvent, AppInfo, ReceiverState, ServerCommand};
use crate::discovery;
use crate::protocol::{
    PKT_BATTERY, PKT_STATE, get_empty_ds4_report, get_empty_x360_report, parse_ds4_state,
    parse_x360_state,
};

enum TargetHandleEnum {
    Xbox(TargetHandle<Xbox360>),
    Ds4(TargetHandle<DualShock4>),
}

struct ActiveTarget {
    handle: TargetHandleEnum,
    stop_rumble: Arc<AtomicBool>,
}

impl Drop for ActiveTarget {
    fn drop(&mut self) {
        self.stop_rumble.store(true, Ordering::Relaxed);
    }
}

enum SlotMessage {
    Data { len: usize, buf: [u8; 64] },
    Disconnect,
}

pub async fn run_backend(
    mut cmd_rx: mpsc::Receiver<ServerCommand>,
    config_rx: watch::Receiver<AppConfig>,
    event_tx: broadcast::Sender<AppEvent>,
) {
    let mut server_task: Option<tokio::task::JoinHandle<()>> = None;
    let mut current_server_tx: Option<mpsc::Sender<ServerCommand>> = None;

    loop {
        tokio::select! {
            cmd_opt = cmd_rx.recv() => {
                match cmd_opt {
                    Some(ServerCommand::Start) => {
                        if server_task.is_none() {
                            let (tx, rx) = mpsc::channel(10);
                            current_server_tx = Some(tx);

                            server_task = Some(tokio::spawn(run_server(
                                config_rx.clone(),
                                event_tx.clone(),
                                rx,
                            )));

                            let _ = event_tx.send(AppEvent::ReceiverStateChanged(ReceiverState::Starting));
                        }
                    }
                    Some(ServerCommand::Stop) => {
                        if let Some(task) = server_task.take() {
                            task.abort();
                            current_server_tx = None;
                            let _ = event_tx.send(AppEvent::ReceiverStateChanged(ReceiverState::Off));
                            let _ = event_tx.send(AppEvent::Log("Stopped.".into()));
                        }
                    }
                    Some(ServerCommand::DisconnectSlot(slot)) => {
                        if let Some(tx) = &current_server_tx {
                            let _ = tx.try_send(ServerCommand::DisconnectSlot(slot));
                        }
                    }
                    None => {
                        if let Some(task) = server_task.take() {
                            task.abort();
                        }
                        break;
                    }
                }
            }
            _ = async {
                if let Some(task) = server_task.as_mut() {
                    let _ = task.await;
                } else {
                    std::future::pending().await
                }
            } => {
                server_task = None;
                current_server_tx = None;
            }
        }
    }
}

async fn run_server(
    config_rx: watch::Receiver<AppConfig>,
    event_tx: broadcast::Sender<AppEvent>,
    mut server_cmd_rx: mpsc::Receiver<ServerCommand>,
) {
    let vigem_client = match Client::connect() {
        Ok(c) => Arc::new(c),
        Err(e) => {
            let _ = event_tx.send(AppEvent::MissingDriver);
            let _ = event_tx.send(AppEvent::Log(format!("ViGEmBus Client failed: {}", e)));
            let _ = event_tx.send(AppEvent::ReceiverStateChanged(ReceiverState::Off));
            return;
        }
    };

    let bind_port = {
        let cfg = config_rx.borrow();
        if cfg.use_custom_port { cfg.port } else { 0 }
    };
    let socket = match UdpSocket::bind(format!("0.0.0.0:{}", bind_port)).await {
        Ok(s) => Arc::new(s),
        Err(e) => {
            let _ = event_tx.send(AppEvent::Log(format!("Failed to bind socket: {}", e)));
            let _ = event_tx.send(AppEvent::ReceiverStateChanged(ReceiverState::Off));
            return;
        }
    };
    let local_port = match socket.local_addr() {
        Ok(addr) => addr.port(),
        Err(e) => {
            let _ = event_tx.send(AppEvent::Log(format!("Failed to get local port: {}", e)));
            let _ = event_tx.send(AppEvent::ReceiverStateChanged(ReceiverState::Off));
            return;
        }
    };

    let _ = event_tx.send(AppEvent::ReceiverStateChanged(ReceiverState::On));
    let _ = event_tx.send(AppEvent::Log(format!("Listening on port {}", local_port)));

    let (client_ip_tx, client_ip_rx) = watch::channel::<Option<SocketAddr>>(None);

    tokio::spawn(discovery::start_beacon(
        local_port,
        config_rx.clone(),
        client_ip_rx.clone(),
    ));

    let mut slot_senders = vec![];
    for slot_id in 0..4 {
        let (tx, rx) = mpsc::channel::<SlotMessage>(100);
        slot_senders.push(tx);
        tokio::spawn(run_controller_actor(
            slot_id as u8,
            rx,
            config_rx.clone(),
            event_tx.clone(),
            Arc::clone(&socket),
            client_ip_rx.clone(),
            Arc::clone(&vigem_client),
        ));
    }

    let mut buf = [0u8; 64];

    let sleep = tokio::time::sleep(Duration::from_secs(3));
    tokio::pin!(sleep);
    let mut has_client = false;

    loop {
        tokio::select! {
            cmd = server_cmd_rx.recv() => {
                match cmd {
                    Some(ServerCommand::DisconnectSlot(s)) => {
                        let _ = slot_senders[s as usize].try_send(SlotMessage::Disconnect);
                    }
                    None => break,
                    _ => {}
                }
            }
            res = socket.recv_from(&mut buf) => {
                if let Ok((size, src)) = res {
                    let current_ip = *client_ip_tx.borrow();

                    if let Some(locked_ip) = current_ip {
                        if src != locked_ip {
                            continue;
                        }
                    } else {
                        let _ = client_ip_tx.send(Some(src));
                        let _ = event_tx.send(AppEvent::Log(format!("Device connected ({})", src.ip())));
                        has_client = true;
                    }

                    sleep.as_mut().reset(tokio::time::Instant::now() + Duration::from_secs(3));

                    if size < 2 { continue; }
                    let slot_idx = buf[1] as usize;

                    if slot_idx < 4 {
                        let mut payload = [0u8; 64];
                        payload[..size].copy_from_slice(&buf[..size]);

                        let _ = slot_senders[slot_idx].try_send(SlotMessage::Data {
                            len: size,
                            buf: payload
                        });
                    }
                }
            }
            _ = &mut sleep, if has_client => {
                let _ = client_ip_tx.send(None);
                let _ = event_tx.send(AppEvent::Log("Client connection timed out.".into()));
                has_client = false;
            }
        }
    }
}

async fn run_controller_actor(
    slot_id: u8,
    mut packet_rx: mpsc::Receiver<SlotMessage>,
    mut config_rx: watch::Receiver<AppConfig>,
    event_tx: broadcast::Sender<AppEvent>,
    socket: Arc<UdpSocket>,
    client_ip_rx: watch::Receiver<Option<SocketAddr>>,
    client: Arc<Client>,
) {
    let mut target: Option<ActiveTarget> = None;
    let mut is_active = false;

    let mut current_profile = config_rx.borrow().profiles[slot_id as usize].clone();
    let mut raw_deadzone = RawDeadzone::from(current_profile.deadzone);

    loop {
        tokio::select! {
            res = config_rx.changed() => {
                if res.is_ok() {
                    current_profile = config_rx.borrow().profiles[slot_id as usize].clone();
                    raw_deadzone = RawDeadzone::from(current_profile.deadzone);
                } else {
                    break;
                }
            }
            res = tokio::time::timeout(Duration::from_secs(3), packet_rx.recv()) => {
                match res {
                    Ok(Some(msg)) => {
                        match msg {
                            SlotMessage::Disconnect => {
                                target.take();
                                is_active = false;

                                continue;
                            }
                            SlotMessage::Data { len, buf: packet } => {
                                if target.is_none() {
                                    match setup_controller(
                                        &client,
                                        slot_id,
                                        current_profile.controller_type,
                                        Arc::clone(&socket),
                                        client_ip_rx.clone(),
                                        config_rx.clone()
                                    ) {
                                        Ok(t) => {
                                            target = Some(t);
                                        }
                                        Err(e) => {
                                            let _ = event_tx.send(AppEvent::Log(format!("Slot {} error: {}", slot_id + 1, e)));
                                            tokio::time::sleep(Duration::from_secs(1)).await;
                                            continue;
                                        }
                                    }
                                }

                                if !is_active {
                                    is_active = true;
                                    let _ = event_tx.send(AppEvent::ControllerConnected(slot_id));
                                }

                                if let Some(t) = &target {
                                    match packet[0] {
                                        PKT_STATE if len >= 12 => {
                                            match &t.handle {
                                                TargetHandleEnum::Xbox(h) => { let _ = h.update(&parse_x360_state(&packet[2..], &raw_deadzone)); }
                                                TargetHandleEnum::Ds4(h) => { let _ = h.update(&parse_ds4_state(&packet[2..], &raw_deadzone)); }
                                            }
                                        }
                                        PKT_BATTERY if len >= 4 => {
                                            let _ = event_tx.send(AppEvent::BatteryUpdate(
                                                slot_id,
                                                AppInfo { controller_battery: packet[2] as i8, phone_battery: packet[3] as i8 }
                                            ));
                                        }
                                        _ => {}
                                    }
                                }
                            }
                        }
                    }
                    Ok(None) => break,
                    Err(_) => { // Timeout triggers organically when packets stop arriving
                        if is_active {
                            is_active = false;

                            let _ = event_tx.send(AppEvent::ControllerDisconnected(slot_id));

                            if let Some(t) = &target {
                                match &t.handle {
                                    TargetHandleEnum::Xbox(h) => { let _ = h.update(&get_empty_x360_report()); }
                                    TargetHandleEnum::Ds4(h) => { let _ = h.update(&get_empty_ds4_report()); }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

fn setup_controller(
    client: &Client,
    slot_id: u8,
    profile_type: ControllerType,
    socket: Arc<UdpSocket>,
    client_ip_rx: watch::Receiver<Option<SocketAddr>>,
    config_rx: watch::Receiver<AppConfig>,
) -> Result<ActiveTarget> {
    let stop_rumble = Arc::new(AtomicBool::new(false));

    macro_rules! spawn_rumble {
        ($target:expr) => {{
            let notif_rx = $target
                .register_notification()
                .context("Failed to register notifications")?;
            let stop_flag = Arc::clone(&stop_rumble);
            let socket = Arc::clone(&socket);
            let client_ip_rx = client_ip_rx.clone();
            let config_rx = config_rx.clone();

            tokio::task::spawn_blocking(move || {
                while !stop_flag.load(Ordering::Relaxed) {
                    if let Ok(Ok(n)) = notif_rx.recv_timeout(Duration::from_millis(100)) {
                        if let Some(addr) = *client_ip_rx.borrow() {
                            let strength = config_rx.borrow().profiles[slot_id as usize]
                                .rumble_strength
                                / 100.0;
                            let large_motor =
                                (n.large_motor as f32 * strength).clamp(0.0, 255.0) as u8;
                            let small_motor =
                                (n.small_motor as f32 * strength).clamp(0.0, 255.0) as u8;
                            let _ = socket.try_send_to(&[slot_id, large_motor, small_motor], addr);
                        }
                    }
                }
            });
        }};
    }

    match profile_type {
        ControllerType::X360 => {
            let target = client
                .new_x360_target()
                .plugin()
                .context("Failed to plugin X360")?;
            target.wait_for_ready().context("X360 not ready")?;

            spawn_rumble!(target);

            Ok(ActiveTarget {
                handle: TargetHandleEnum::Xbox(target),
                stop_rumble,
            })
        }
        ControllerType::DS4 => {
            let target = client
                .new_ds4_target()
                .plugin()
                .context("Failed to plugin DS4")?;
            target.wait_for_ready().context("DS4 not ready")?;

            spawn_rumble!(target);

            Ok(ActiveTarget {
                handle: TargetHandleEnum::Ds4(target),
                stop_rumble,
            })
        }
    }
}
