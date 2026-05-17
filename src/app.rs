use eframe::egui;
use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, mpsc};
use tokio::sync::{mpsc as tokio_mpsc, watch};
use tray_icon::TrayIcon;

use crate::config::{AppConfig, ThemeMode};
use crate::core::{AppEvent, AppInfo, ReceiverState, ServerCommand};
use crate::licenses::LicenseWindow;
use crate::ui;
use crate::updater::UpdateInfo;

const MAX_LOG_MESSAGES: usize = 25;

#[derive(Debug, PartialEq, Clone)]
pub struct SlotState {
    pub info: AppInfo,
    pub is_active: bool,
}

#[derive(Debug, PartialEq)]
pub enum Tab {
    Home,
    Settings,
    About,
}

pub struct ControlayApp {
    pub config: AppConfig,
    pub receiver_state: ReceiverState,
    pub controllers: [Option<SlotState>; 4],
    pub active_tab: Tab,
    pub selected_slot: usize,
    pub status_log: VecDeque<String>,
    pub show_driver_alert: bool,
    pub show_update_alert: bool,
    pub pending_update: Option<UpdateInfo>,
    pub license_window: LicenseWindow,

    // Channels bridging UI to Tokio
    config_tx: watch::Sender<AppConfig>,
    cmd_tx: tokio_mpsc::Sender<ServerCommand>,
    ui_event_rx: mpsc::Receiver<AppEvent>,

    is_visible: Arc<AtomicBool>,

    // Needs to be kept alive so the tray icon works
    #[allow(dead_code)]
    tray_icon: TrayIcon,
}

impl ControlayApp {
    pub fn new(
        _cc: &eframe::CreationContext<'_>,
        config_tx: watch::Sender<AppConfig>,
        cmd_tx: tokio_mpsc::Sender<ServerCommand>,
        ui_event_rx: mpsc::Receiver<AppEvent>,
        saved_config: AppConfig,
        is_visible: Arc<AtomicBool>,
        tray_icon: TrayIcon,
    ) -> Self {
        let mut app = Self {
            config: saved_config.clone(),
            receiver_state: ReceiverState::Off,
            controllers: [None, None, None, None],
            active_tab: Tab::Home,
            selected_slot: 0,
            status_log: vec!["Application started".to_string()].into(),
            show_driver_alert: false,
            show_update_alert: false,
            pending_update: None,
            license_window: LicenseWindow::default(),
            config_tx,
            cmd_tx,
            ui_event_rx,
            is_visible,
            tray_icon,
        };

        if app.config.auto_start {
            app.start_broadcasting();
        }

        app
    }

    pub fn start_broadcasting(&mut self) {
        self.trigger_live_update();
        let _ = self.cmd_tx.try_send(ServerCommand::Start);
    }

    pub fn stop_broadcasting(&mut self) {
        self.controllers = [None, None, None, None];
        let _ = self.cmd_tx.try_send(ServerCommand::Stop);
    }

    pub fn trigger_live_update(&self) {
        let _ = self.config_tx.send(self.config.clone());
    }

    pub fn force_disconnect(&mut self, slot_id: u8) {
        self.controllers[slot_id as usize] = None;
        let _ = self.cmd_tx.try_send(ServerCommand::DisconnectSlot(slot_id));
    }

    fn handle_incoming_messages(&mut self) {
        while let Ok(event) = self.ui_event_rx.try_recv() {
            match event {
                AppEvent::ReceiverStateChanged(s) => self.receiver_state = s,
                AppEvent::MissingDriver => self.show_driver_alert = true,
                AppEvent::Log(msg) => {
                    self.status_log.push_back(msg);
                    if self.status_log.len() > MAX_LOG_MESSAGES {
                        self.status_log.pop_front();
                    }
                }
                AppEvent::UpdateAvailable(info) => {
                    self.status_log
                        .push_back(format!("New version available: {}", info.version));
                    if Some(&info.version) != self.config.dismissed_update_version.as_ref() {
                        self.show_update_alert = true;
                    }
                    self.pending_update = Some(info);
                }
                AppEvent::ControllerConnected(slot) => {
                    let idx = slot as usize;
                    if let Some(state) = &mut self.controllers[idx] {
                        state.is_active = true;
                    } else {
                        self.controllers[idx] = Some(SlotState {
                            info: AppInfo {
                                controller_battery: -1,
                                phone_battery: -1,
                            },
                            is_active: true,
                        });
                    }
                }
                AppEvent::ControllerDisconnected(slot) => {
                    let idx = slot as usize;
                    if let Some(state) = &mut self.controllers[idx] {
                        state.is_active = false;
                    }
                }
                AppEvent::BatteryUpdate(slot, info) => {
                    let idx = slot as usize;
                    if let Some(state) = &mut self.controllers[idx] {
                        state.info = info;
                    }
                }
            }
        }
    }
}
impl eframe::App for ControlayApp {
    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        eframe::set_value(storage, eframe::APP_KEY, &self.config);
    }

    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        if ui.input(|i| i.viewport().close_requested() && self.config.tray_on_close) {
            ui.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.is_visible.store(false, Ordering::Relaxed);
            ui.send_viewport_cmd(egui::ViewportCommand::Visible(false));
        }

        match self.config.theme {
            ThemeMode::Dark => ui.ctx().set_visuals(egui::Visuals::dark()),
            ThemeMode::Light => ui.ctx().set_visuals(egui::Visuals::light()),
            ThemeMode::System => ui.ctx().set_visuals(egui::Visuals::default()),
        }

        self.handle_incoming_messages();

        egui::CentralPanel::default().show_inside(ui, |ui| {
            if self.show_driver_alert || self.show_update_alert {
                ui.disable();
            }
            ui::draw_main_layout(self, ui);
        });

        self.license_window.show(ui);

        if self.show_driver_alert {
            ui::draw_driver_alert(self, ui);
        }
        if self.show_update_alert {
            ui::draw_update_alert(self, ui);
        }
    }
}
