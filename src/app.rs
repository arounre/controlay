use std::sync::mpsc;
use std::thread;

use eframe::egui;
use winit::raw_window_handle::Win32WindowHandle;

use crate::config::{AppConfig, LogicSettings, ThemeMode};
use crate::licenses::LicenseWindow;
use crate::updater::{self, UpdateInfo};
use crate::{flip_window_visibility, notifier, ui};

const MAX_LOG_MESSAGES: usize = 25;

#[derive(Clone)]
pub struct EventProxy {
    tx: mpsc::Sender<UiUpdate>,
    ctx: egui::Context,
}

impl EventProxy {
    pub fn send(&self, update: UiUpdate) {
        if self.tx.send(update).is_ok() {
            self.ctx.request_repaint();
        }
    }
}

// Command sent to logic thread
pub enum LogicCommand {
    Start(LogicSettings),
    Stop,
    UpdateSettings(LogicSettings),
}

// Update received from logic thread
pub enum UiUpdate {
    BatteryUpdate(u8, AppInfo),
    ControllerDisconnected(u8),
    ReceiverStateChanged(ReceiverState),
    Error(String),
    Log(String),
    MissingDriver,
    UpdateAvailable(UpdateInfo),
}

#[derive(Debug, PartialEq, Clone)]
pub struct AppInfo {
    pub controller_battery: i8,
    pub phone_battery: i8,
}

#[derive(Debug, PartialEq, Clone)]
pub struct SlotState {
    pub info: AppInfo,
    pub sent_controller_low_warn: bool,
    pub sent_phone_low_warn: bool,
}

#[derive(Debug, PartialEq, Copy, Clone)]
pub enum ReceiverState {
    Off,
    Starting,
    On,
}

#[derive(Debug, PartialEq)]
pub enum Tab {
    Home,
    Settings,
    About,
}

pub struct ControlayApp {
    // Config
    pub config: AppConfig,

    // Runtime state
    pub receiver_state: ReceiverState,
    pub controllers: [Option<SlotState>; 4],
    pub active_tab: Tab,
    pub selected_slot: usize, // For settings UI

    // Internal state
    pub status_log: Vec<String>,
    pub show_driver_alert: bool,

    // Update checker state
    pub show_update_alert: bool,
    pub pending_update: Option<UpdateInfo>,

    // Sub-systems
    pub license_window: LicenseWindow,
    notifier: Option<notifier::AppNotifier>,

    // Threading and OS
    logic_tx: mpsc::Sender<LogicCommand>,
    ui_rx: mpsc::Receiver<UiUpdate>,
    _logic_thread_handle: Option<thread::JoinHandle<()>>,
    handle: Win32WindowHandle,
}

impl ControlayApp {
    pub fn new(
        cc: &eframe::CreationContext<'_>,
        handle: Win32WindowHandle,
        notifier: Option<notifier::AppNotifier>,
    ) -> Self {
        let (logic_tx, command_rx) = mpsc::channel();
        let (update_tx_raw, ui_rx) = mpsc::channel();

        let update_proxy = EventProxy {
            tx: update_tx_raw,
            ctx: cc.egui_ctx.clone(),
        };

        let logic_update_proxy = update_proxy.clone();
        let logic_handle = thread::spawn(move || {
            crate::logic::run(command_rx, logic_update_proxy);
        });

        let saved_config: AppConfig = cc
            .storage
            .and_then(|storage| eframe::get_value(storage, eframe::APP_KEY))
            .unwrap_or_default();

        if saved_config.check_updates {
            updater::check_for_updates(update_proxy.clone());
        }

        let mut app = Self {
            config: saved_config,
            receiver_state: ReceiverState::Off,
            controllers: [None, None, None, None],
            active_tab: Tab::Home,
            selected_slot: 0,
            status_log: vec!["Application started.".to_string()],
            show_driver_alert: false,
            show_update_alert: false,
            pending_update: None,
            license_window: LicenseWindow::default(),

            notifier,
            logic_tx,
            ui_rx,
            _logic_thread_handle: Some(logic_handle),
            handle,
        };

        if app.config.auto_start {
            app.start_broadcasting();
        }

        app
    }

    pub fn start_broadcasting(&mut self) {
        let settings = LogicSettings::from(&self.config);
        self.log_status("Starting services...".to_string());
        self.receiver_state = ReceiverState::Starting;

        if let Err(e) = self.logic_tx.send(LogicCommand::Start(settings)) {
            self.log_status(format!("Failed to start: {}", e));
            self.receiver_state = ReceiverState::Off;
        }
    }

    pub fn stop_broadcasting(&mut self) {
        self.controllers = [None, None, None, None];
        self.log_status("Stopping...".to_string());
        let _ = self.logic_tx.send(LogicCommand::Stop);
    }

    pub fn trigger_live_update(&self) {
        let settings = LogicSettings::from(&self.config);
        let _ = self.logic_tx.send(LogicCommand::UpdateSettings(settings));
    }

    fn handle_incoming_messages(&mut self) {
        while let Ok(update) = self.ui_rx.try_recv() {
            match update {
                UiUpdate::ReceiverStateChanged(s) => self.receiver_state = s,
                UiUpdate::Error(msg) => {
                    self.log_status(format!("ERROR: {}", msg));
                    self.receiver_state = ReceiverState::Off;
                }
                UiUpdate::Log(msg) => self.log_status(msg),
                UiUpdate::MissingDriver => {
                    self.receiver_state = ReceiverState::Off;
                    self.show_driver_alert = true;
                    self.log_status("Failed: Driver missing.".to_string());
                }
                UiUpdate::BatteryUpdate(slot, info) => self.update_battery(slot, info),
                UiUpdate::ControllerDisconnected(slot) => self.disconnect_controller(slot),
                UiUpdate::UpdateAvailable(update_info) => {
                    self.log_status(format!("New version available: {}", update_info.version));
                    if Some(&update_info.version) != self.config.dismissed_update_version.as_ref() {
                        self.show_update_alert = true;
                    }
                    self.pending_update = Some(update_info);
                }
            }
        }
    }

    fn update_battery(&mut self, slot_id: u8, info: AppInfo) {
        let idx = slot_id as usize;
        if idx >= 4 {
            return;
        }

        let was_connected = self.controllers[idx].is_some();
        if !was_connected {
            self.log_status(format!("Controller connected (Slot {}).", idx + 1));
            if self.config.enable_connection_notifications {
                if let Some(notifier) = &self.notifier {
                    let _ =
                        notifier.notify("Controlay", &format!("Controller {} connected", idx + 1));
                }
            }
        }

        if let Some(state) = &mut self.controllers[idx] {
            state.info = info;
        } else {
            self.controllers[idx] = Some(SlotState {
                info,
                sent_controller_low_warn: false,
                sent_phone_low_warn: false,
            });
        }

        self.check_battery_levels(idx);
    }

    fn disconnect_controller(&mut self, slot_id: u8) {
        let idx = slot_id as usize;
        if idx < 4 && self.controllers[idx].is_some() {
            self.controllers[idx] = None;
            self.log_status(format!("Controller disconnected (Slot {}).", idx + 1));

            if self.config.enable_connection_notifications {
                if let Some(notifier) = &self.notifier {
                    let _ = notifier
                        .notify("Controlay", &format!("Controller {} disconnected", idx + 1));
                }
            }
        }
    }

    fn check_battery_levels(&mut self, idx: usize) {
        if !self.config.enable_battery_notifications || self.notifier.is_none() {
            return;
        }

        if let Some(state) = &mut self.controllers[idx] {
            // Check Controller Battery
            if state.info.controller_battery >= 0 {
                let val = state.info.controller_battery as u8;
                if val <= self.config.battery_warn_threshold_controller {
                    if !state.sent_controller_low_warn {
                        if let Some(notifier) = &self.notifier {
                            let _ = notifier.notify(
                                "Low Battery",
                                &format!("P{} Controller battery is at {}%", idx + 1, val),
                            );
                        }
                        state.sent_controller_low_warn = true;
                    }
                } else {
                    state.sent_controller_low_warn = false;
                }
            }

            // Check Phone Battery
            if state.info.phone_battery >= 0 {
                let val = state.info.phone_battery as u8;
                if val <= self.config.battery_warn_threshold_phone {
                    if !state.sent_phone_low_warn {
                        if let Some(notifier) = &self.notifier {
                            let _ = notifier
                                .notify("Low Battery", &format!("Phone battery is at {}%", val));
                        }
                        state.sent_phone_low_warn = true;
                    }
                } else {
                    state.sent_phone_low_warn = false;
                }
            }
        }
    }

    fn log_status(&mut self, message: String) {
        self.status_log.push(message);
        if self.status_log.len() > MAX_LOG_MESSAGES {
            self.status_log.remove(0);
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
            flip_window_visibility(self.handle);
        }

        match self.config.theme {
            ThemeMode::Dark => ui.set_visuals(egui::Visuals::dark()),
            ThemeMode::Light => ui.set_visuals(egui::Visuals::light()),
            ThemeMode::System => ui.set_visuals(egui::Visuals::default()),
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
