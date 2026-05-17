use crate::updater::UpdateInfo;

#[derive(Debug, PartialEq, Clone, Copy)]
pub enum ReceiverState {
    Off,
    Starting,
    On,
}

#[derive(Debug, PartialEq, Clone)]
pub struct AppInfo {
    pub controller_battery: i8,
    pub phone_battery: i8,
}

// The central event bus
#[derive(Clone)]
pub enum AppEvent {
    Log(String),
    ReceiverStateChanged(ReceiverState),
    ControllerConnected(u8),
    ControllerDisconnected(u8),
    BatteryUpdate(u8, AppInfo),
    MissingDriver,
    UpdateAvailable(UpdateInfo),
}

// Commands sent from the UI down to the Logic backend
#[derive(Clone)]
pub enum ServerCommand {
    Start,
    Stop,
    DisconnectSlot(u8),
}
