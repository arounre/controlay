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

#[derive(Debug, Clone, Copy, Deserialize, Serialize)]
#[serde(default)]
pub struct ControllerProfile {
    pub controller_type: ControllerType,
    pub deadzone: DeadzoneConfig,
    pub rumble_strength: f32,
}

impl Default for ControllerProfile {
    fn default() -> Self {
        Self {
            controller_type: ControllerType::X360,
            deadzone: DeadzoneConfig::default(),
            rumble_strength: 100.0,
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
