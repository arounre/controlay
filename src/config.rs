use serde::{Deserialize, Serialize};

pub const MAX_STICK_VALUE: i16 = i16::MAX; // 32767

#[derive(Debug, Copy, Clone, Deserialize, Serialize)]
pub struct DeadzoneConfig {
    pub inner_percent: f32,
    pub outer_percent: f32,
}

impl Default for DeadzoneConfig {
    fn default() -> Self {
        Self {
            inner_percent: 15.0,
            outer_percent: 90.0,
        }
    }
}

#[derive(Debug, Copy, Clone)]
pub struct RawDeadzone {
    pub inner: i16,
    pub outer: i16,
    pub scale_factor: f32,
}

impl RawDeadzone {
    #[inline(always)]
    pub fn apply(&self, mut value: i16) -> i16 {
        if value == i16::MIN {
            value = -MAX_STICK_VALUE;
        }

        let abs_val = value.abs();
        if abs_val < self.inner {
            return 0;
        }

        if abs_val >= self.outer {
            return if value > 0 {
                MAX_STICK_VALUE
            } else {
                -MAX_STICK_VALUE
            };
        }

        let val_offset = (abs_val - self.inner) as f32;
        let magnitude = (val_offset * self.scale_factor) as i16;

        if value > 0 { magnitude } else { -magnitude }
    }
}

impl From<DeadzoneConfig> for RawDeadzone {
    fn from(cfg: DeadzoneConfig) -> Self {
        let max = MAX_STICK_VALUE as f32;
        let inner = (cfg.inner_percent / 100.0 * max) as i16;
        let outer = (cfg.outer_percent / 100.0 * max) as i16;

        let range = (outer - inner) as f32;
        let scale_factor = if range > 0.0 { max / range } else { 0.0 };

        RawDeadzone {
            inner,
            outer,
            scale_factor,
        }
    }
}

#[derive(Debug, PartialEq, Clone, Copy, Deserialize, Serialize)]
pub enum ControllerType {
    X360,
    DS4,
}

#[derive(Debug, PartialEq, Deserialize, Serialize, Clone, Copy)]
pub enum ThemeMode {
    System,
    Light,
    Dark,
}

// XInput / Xbox 360 digital bits. Matches the wire protocol and ViGEm X360Button.
pub const BTN_DPAD_UP: u16 = 0x0001;
pub const BTN_DPAD_DOWN: u16 = 0x0002;
pub const BTN_DPAD_LEFT: u16 = 0x0004;
pub const BTN_DPAD_RIGHT: u16 = 0x0008;
pub const BTN_START: u16 = 0x0010;
pub const BTN_BACK: u16 = 0x0020;
pub const BTN_LEFT_THUMB: u16 = 0x0040;
pub const BTN_RIGHT_THUMB: u16 = 0x0080;
pub const BTN_LEFT_SHOULDER: u16 = 0x0100;
pub const BTN_RIGHT_SHOULDER: u16 = 0x0200;
pub const BTN_GUIDE: u16 = 0x0400;
pub const BTN_A: u16 = 0x1000;
pub const BTN_B: u16 = 0x2000;
pub const BTN_X: u16 = 0x4000;
pub const BTN_Y: u16 = 0x8000;

pub const BTN_DPAD_ALL: u16 = BTN_DPAD_UP | BTN_DPAD_DOWN | BTN_DPAD_LEFT | BTN_DPAD_RIGHT;

pub const ANTI_DOUBLE_CLICK_WINDOW_DEFAULT_MS: u16 = 50;
pub const ANTI_DOUBLE_CLICK_WINDOW_MIN_MS: u16 = 10;
pub const ANTI_DOUBLE_CLICK_WINDOW_MAX_MS: u16 = 150;

/// Per-button rising-edge cooldown. A second press of the same selected button
/// inside `window_ms` is dropped. releases are never delayed.
#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(default)]
pub struct AntiDoubleClickConfig {
    pub enabled: bool,
    pub window_ms: u16,
    /// Bitmask of buttons to filter (same bits as the state packet).
    pub buttons: u16,
}

impl Default for AntiDoubleClickConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            window_ms: ANTI_DOUBLE_CLICK_WINDOW_DEFAULT_MS,
            buttons: BTN_DPAD_ALL,
        }
    }
}

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(default)]
pub struct ControllerProfile {
    pub controller_type: ControllerType,
    pub deadzone: DeadzoneConfig,
    pub rumble_strength: f32,
    #[serde(default)]
    pub anti_double_click: AntiDoubleClickConfig,
}

impl Default for ControllerProfile {
    fn default() -> Self {
        Self {
            controller_type: ControllerType::X360,
            deadzone: DeadzoneConfig::default(),
            rumble_strength: 100.0,
            anti_double_click: AntiDoubleClickConfig::default(),
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(default)]
pub struct AppConfig {
    // Hardware / Network
    pub port: u16,
    pub use_custom_port: bool,
    pub hostname: String,
    pub use_default_hostname: bool,

    // Per-Controller Configs (Supports up to 4 slots)
    pub profiles: [ControllerProfile; 4],

    // App Preferences
    pub theme: ThemeMode,
    pub auto_start: bool,
    pub tray_on_close: bool,
    pub check_updates: bool,

    // Notifications
    pub enable_battery_notifications: bool,
    pub enable_connection_notifications: bool,
    pub battery_warn_threshold_controller: u8,
    pub battery_warn_threshold_phone: u8,

    // Updater
    pub dismissed_update_version: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            port: 0,
            use_custom_port: false,
            hostname: "".into(),
            use_default_hostname: true,
            profiles: [ControllerProfile::default(); 4],
            theme: ThemeMode::System,
            auto_start: false,
            tray_on_close: false,
            check_updates: true,
            enable_battery_notifications: false,
            enable_connection_notifications: false,
            battery_warn_threshold_controller: 20,
            battery_warn_threshold_phone: 15,
            dismissed_update_version: None,
        }
    }
}
