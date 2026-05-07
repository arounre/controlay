use crate::app::{AppInfo, EventProxy, LogicCommand, ReceiverState, UiUpdate};
use crate::config::{ControllerType, LogicSettings, ProfileLogic};
use crate::discovery;
use crate::protocol::{PacketType, parse_ds4_state, parse_x360_state};
use anyhow::{Context, Result, anyhow};
use std::net::{SocketAddr, UdpSocket};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::mpsc::{self, Receiver, TryRecvError};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

use local_ip_address::local_ip;
use vigem_rust::TargetHandle;
use vigem_rust::client::Client;
use vigem_rust::target::{DualShock4, Xbox360};

enum ActiveTarget {
    Xbox(TargetHandle<Xbox360>),
    Ds4(TargetHandle<DualShock4>),
}

struct SlotData {
    target: ActiveTarget,
    last_packet_time: std::time::Instant,
}

fn setup_controller(
    client: &Client,
    slot_id: u8,
    profile: &ProfileLogic,
    rumble_tx: mpsc::Sender<(u8, u8, u8)>,
) -> Result<ActiveTarget> {
    match profile.controller_type {
        ControllerType::X360 => {
            let target = client
                .new_x360_target()
                .plugin()
                .context("Failed to plugin X360 target")?;

            target.wait_for_ready().context("X360 target not ready")?;

            let notif_rx = target
                .register_notification()
                .context("Failed to register X360 notifications")?;

            thread::spawn(move || {
                while let Ok(Ok(n)) = notif_rx.recv() {
                    let _ = rumble_tx.send((slot_id, n.large_motor, n.small_motor));
                }
            });

            Ok(ActiveTarget::Xbox(target))
        }
        ControllerType::DS4 => {
            let target = client
                .new_ds4_target()
                .plugin()
                .context("Failed to plugin DS4 target")?;

            target.wait_for_ready().context("DS4 target not ready")?;

            let notif_rx = target
                .register_notification()
                .context("Failed to register DS4 notifications")?;

            thread::spawn(move || {
                while let Ok(Ok(n)) = notif_rx.recv() {
                    let _ = rumble_tx.send((slot_id, n.large_motor, n.small_motor));
                }
            });

            Ok(ActiveTarget::Ds4(target))
        }
    }
}

fn run_session(
    settings: LogicSettings,
    command_rx: &Receiver<LogicCommand>,
    update_tx: &EventProxy,
    last_client_addr: Arc<Mutex<Option<SocketAddr>>>,
) -> Result<()> {
    let bind_port = match settings.port {
        Some(p) => p,
        None => {
            let temp_socket =
                UdpSocket::bind("0.0.0.0:0").context("Failed to bind temporary socket")?;
            temp_socket
                .local_addr()
                .context("Could not get local address")?
                .port()
        }
    };

    let client = match Client::connect() {
        Ok(c) => c,
        Err(e) => {
            let _ = update_tx.send(UiUpdate::MissingDriver);
            return Err(anyhow!("ViGEmBus Client failed: {}", e));
        }
    };

    let socket =
        UdpSocket::bind(format!("0.0.0.0:{}", bind_port)).context("Failed to bind UDP socket")?;

    socket
        .set_read_timeout(Some(Duration::from_millis(100)))
        .context("Failed to set read timeout")?;

    let local_port = socket
        .local_addr()
        .context("Could not get local address")?
        .port();

    let shared_socket = Arc::new(socket);

    // Rumble array per-slot
    let shared_rumble_bits = Arc::new([
        AtomicU32::new(settings.profiles[0].rumble_strength.to_bits()),
        AtomicU32::new(settings.profiles[1].rumble_strength.to_bits()),
        AtomicU32::new(settings.profiles[2].rumble_strength.to_bits()),
        AtomicU32::new(settings.profiles[3].rumble_strength.to_bits()),
    ]);

    let (rumble_tx, rumble_rx) = mpsc::channel::<(u8, u8, u8)>();

    // Broadcast
    let stop_broadcast_flag = Arc::new(AtomicBool::new(false));
    let hostname = settings.hostname.clone();

    let spawn_discovery = |stop_flag: Arc<AtomicBool>, tx: EventProxy| {
        discovery::start_beacon(local_port, stop_flag, tx, hostname.clone());
    };

    spawn_discovery(Arc::clone(&stop_broadcast_flag), update_tx.clone());

    // Rumble thread
    let socket_clone = Arc::clone(&shared_socket);
    let client_addr_clone = Arc::clone(&last_client_addr);
    let rumble_bits_ref = Arc::clone(&shared_rumble_bits);

    thread::spawn(move || {
        while let Ok((slot_id, large, small)) = rumble_rx.recv() {
            if slot_id >= 4 {
                continue;
            }
            let strength =
                f32::from_bits(rumble_bits_ref[slot_id as usize].load(Ordering::Relaxed));

            let large_motor = (large as f32 * strength).clamp(0.0, 255.0) as u8;
            let small_motor = (small as f32 * strength).clamp(0.0, 255.0) as u8;

            if let Some(addr) = *client_addr_clone.lock().unwrap() {
                let _ = socket_clone.send_to(&[slot_id, large_motor, small_motor], addr);
            }
        }
    });

    let _ = update_tx.send(UiUpdate::ReceiverStateChanged(ReceiverState::On));
    let ip_str = local_ip()
        .ok()
        .map(|ip| ip.to_string())
        .unwrap_or("Unknown IP".to_string());
    let _ = update_tx.send(UiUpdate::Log(format!(
        "Listening on {}:{}",
        ip_str, local_port
    )));

    let mut connected_source: Option<SocketAddr> = None;
    let mut buf = [0u8; 32];

    let mut local_profiles = settings.profiles;
    let mut slots: [Option<SlotData>; 4] = [None, None, None, None];
    let mut is_connected = false;

    loop {
        match command_rx.try_recv() {
            Ok(LogicCommand::Stop) | Err(TryRecvError::Disconnected) => break,
            Ok(LogicCommand::UpdateSettings(new_settings)) => {
                local_profiles = new_settings.profiles.clone();
                for i in 0..4 {
                    shared_rumble_bits[i].store(
                        new_settings.profiles[i].rumble_strength.to_bits(),
                        Ordering::Relaxed,
                    );
                }
            }
            _ => {}
        }

        match shared_socket.recv_from(&mut buf) {
            Ok((amt, src)) => {
                if let Some(locked_ip) = connected_source {
                    if src != locked_ip {
                        continue;
                    }
                } else {
                    connected_source = Some(src);
                    *last_client_addr.lock().unwrap() = Some(src);
                    stop_broadcast_flag.store(true, Ordering::Relaxed);
                    is_connected = true;
                }

                let packet = &buf[..amt];
                let slot_id = packet[1];
                let slot_idx = slot_id as usize;

                if slot_idx >= 4 {
                    continue;
                }

                match PacketType::try_from(packet[0]) {
                    Ok(PacketType::State) => {
                        let payload = &packet[2..];
                        if let Some(slot) = &mut slots[slot_idx] {
                            slot.last_packet_time = std::time::Instant::now();
                            match &slot.target {
                                ActiveTarget::Xbox(handle) => {
                                    let report = parse_x360_state(
                                        payload,
                                        &local_profiles[slot_idx].deadzone,
                                    );
                                    let _ = handle.update(&report);
                                }
                                ActiveTarget::Ds4(handle) => {
                                    let report = parse_ds4_state(
                                        payload,
                                        &local_profiles[slot_idx].deadzone,
                                    );
                                    let _ = handle.update(&report);
                                }
                            }
                        }
                    }
                    Ok(PacketType::Battery) => {
                        if slots[slot_idx].is_none() {
                            let target_res = setup_controller(
                                &client,
                                slot_id,
                                &local_profiles[slot_idx],
                                rumble_tx.clone(),
                            );
                            match target_res {
                                Ok(target) => {
                                    slots[slot_idx] = Some(SlotData {
                                        target,
                                        last_packet_time: std::time::Instant::now(),
                                    })
                                }
                                Err(e) => {
                                    let _ = update_tx.send(UiUpdate::Log(format!(
                                        "Slot {} failed: {}",
                                        slot_idx + 1,
                                        e
                                    )));
                                }
                            }
                        }

                        if let Some(slot) = &mut slots[slot_idx] {
                            slot.last_packet_time = std::time::Instant::now();
                        }

                        let controller_battery = packet[2] as i8;
                        let phone_battery = packet[3] as i8;
                        let _ = update_tx.send(UiUpdate::BatteryUpdate(
                            slot_id,
                            AppInfo {
                                controller_battery,
                                phone_battery,
                            },
                        ));
                    }
                    Err(_) => {
                        let _ = update_tx.send(UiUpdate::Error(format!(
                            "Unsupported packet header: {}",
                            buf[0]
                        )));
                    }
                }
            }
            Err(e)
                if e.kind() == std::io::ErrorKind::WouldBlock
                    || e.kind() == std::io::ErrorKind::TimedOut =>
            {
                let now = std::time::Instant::now();
                let mut any_active = false;

                for i in 0..4 {
                    if let Some(slot) = &slots[i] {
                        if now.duration_since(slot.last_packet_time) > Duration::from_secs_f32(2.5)
                        {
                            slots[i] = None;
                            let _ = update_tx.send(UiUpdate::ControllerDisconnected(i as u8));
                        } else {
                            any_active = true;
                        }
                    }
                }

                // If completely disconnected, safely unlock to allow searching new IP
                if is_connected && !any_active {
                    is_connected = false;
                    connected_source = None;
                    *last_client_addr.lock().unwrap() = None;

                    stop_broadcast_flag.store(false, Ordering::Relaxed);
                    spawn_discovery(Arc::clone(&stop_broadcast_flag), update_tx.clone());
                    let _ =
                        update_tx.send(UiUpdate::Log("Listening for new devices...".to_string()));
                }

                continue;
            }
            Err(e) => {
                return Err(anyhow::Error::new(e).context("Socket fatal error"));
            }
        }
    }

    stop_broadcast_flag.store(true, Ordering::Relaxed);
    Ok(())
}

pub fn run(command_rx: Receiver<LogicCommand>, update_tx: EventProxy) {
    let last_client_addr = Arc::new(Mutex::new(None));

    loop {
        match command_rx.recv() {
            Ok(LogicCommand::Start(config)) => {
                if let Err(e) = run_session(
                    config,
                    &command_rx,
                    &update_tx,
                    Arc::clone(&last_client_addr),
                ) {
                    let _ = update_tx.send(UiUpdate::Error(e.to_string()));
                }

                *last_client_addr.lock().unwrap() = None;
                let _ = update_tx.send(UiUpdate::ReceiverStateChanged(ReceiverState::Off));
                let _ = update_tx.send(UiUpdate::Log("Stopped.".to_string()));
            }
            Err(_) => {
                break;
            }
            _ => {}
        }
    }
}
