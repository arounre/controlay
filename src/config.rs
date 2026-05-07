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
}

impl RawDeadzone {
    pub fn apply(&self, value: i16) -> i16 {
        if value == i16::MIN {
            return -MAX_STICK_VALUE;
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

        let range = (self.outer - self.inner) as f32;
        let val_offset = (abs_val - self.inner) as f32;
        let magnitude = ((val_offset / range) * MAX_STICK_VALUE as f32) as i16;

        if value > 0 { magnitude } else { -magnitude }
    }
}

impl From<DeadzoneConfig> for RawDeadzone {
    fn from(cfg: DeadzoneConfig) -> Self {
        let max = MAX_STICK_VALUE as f32;
        RawDeadzone {
            inner: (cfg.inner_percent / 100.0 * max) as i16,
            outer: (cfg.outer_percent / 100.0 * max) as i16,
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
            check_updates: false,
            enable_battery_notifications: true,
            enable_connection_notifications: true,
            battery_warn_threshold_controller: 20,
            battery_warn_threshold_phone: 15,
            dismissed_update_version: None,
        }
    }
}

// DTO for the Logic Thread
#[derive(Clone)]
pub struct ProfileLogic {
    pub controller_type: ControllerType,
    pub deadzone: RawDeadzone,
    pub rumble_strength: f32,
}

impl From<&ControllerProfile> for ProfileLogic {
    fn from(profile: &ControllerProfile) -> Self {
        let max = MAX_STICK_VALUE as f32;

        let raw_deadzone = RawDeadzone {
            inner: (profile.deadzone.inner_percent / 100.0 * max) as i16,
            outer: (profile.deadzone.outer_percent / 100.0 * max) as i16,
        };

        ProfileLogic {
            controller_type: profile.controller_type,
            deadzone: raw_deadzone,
            rumble_strength: (profile.rumble_strength / 100.0).clamp(0.0, 2.0),
        }
    }
}

#[derive(Clone)]
pub struct LogicSettings {
    pub port: Option<u16>,
    pub profiles: [ProfileLogic; 4],
    pub hostname: Option<String>,
}

impl From<&AppConfig> for LogicSettings {
    fn from(cfg: &AppConfig) -> Self {
        LogicSettings {
            port: if cfg.use_custom_port {
                Some(cfg.port)
            } else {
                None
            },
            profiles: std::array::from_fn(|i| ProfileLogic::from(&cfg.profiles[i])),
            hostname: if cfg.use_default_hostname {
                None
            } else {
                Some(cfg.hostname.clone())
            },
        }
    }
}
