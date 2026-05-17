use anyhow::Result;
use notify_rust::Notification;
use tokio::sync::{broadcast, watch};

use crate::config::AppConfig;
use crate::core::AppEvent;

pub struct AppNotifier {
    app_id: String,
}

impl AppNotifier {
    pub fn new(app_id: &str) -> Result<Self> {
        #[allow(unused_mut)]
        let mut final_id = app_id.to_string();

        #[cfg(target_os = "windows")]
        if !Self::shortcut_exists("Controlay") {
            final_id = "Microsoft.Windows.Explorer".to_string();
        }

        Ok(Self { app_id: final_id })
    }

    pub fn notify(&self, title: &str, body: &str) -> Result<()> {
        Notification::new()
            .app_id(&self.app_id)
            .summary(title)
            .body(body)
            .show()?;

        Ok(())
    }

    #[cfg(target_os = "windows")]
    fn shortcut_exists(shortcut_name: &str) -> bool {
        let filename = format!("{}.lnk", shortcut_name);
        let check = |root: &str| {
            std::path::PathBuf::from(root)
                .join(r"Microsoft\Windows\Start Menu\Programs")
                .join(shortcut_name)
                .join(&filename)
                .exists()
        };

        std::env::var("APPDATA").map(|p| check(&p)).unwrap_or(false)
            || std::env::var("ProgramData")
                .map(|p| check(&p))
                .unwrap_or(false)
    }
}

pub async fn run_notification_service(
    mut event_rx: broadcast::Receiver<AppEvent>,
    config_rx: watch::Receiver<AppConfig>,
) {
    let Ok(notifier) = AppNotifier::new("com.arounre.controlay") else {
        return;
    };
    let mut sent_c_warn = [false; 4];
    let mut sent_p_warn = [false; 4];

    loop {
        match event_rx.recv().await {
            Ok(event) => {
                let cfg = config_rx.borrow().clone();

                match event {
                    AppEvent::ControllerConnected(slot) if cfg.enable_connection_notifications => {
                        let _ = notifier
                            .notify("Controlay", &format!("Controller {} connected", slot + 1));
                    }
                    AppEvent::ControllerDisconnected(slot)
                        if cfg.enable_connection_notifications =>
                    {
                        let _ = notifier.notify(
                            "Controlay",
                            &format!("Controller {} disconnected", slot + 1),
                        );
                    }
                    AppEvent::BatteryUpdate(slot, info) if cfg.enable_battery_notifications => {
                        let s = slot as usize;

                        // Controller Warning
                        if info.controller_battery >= 0
                            && info.controller_battery as u8
                                <= cfg.battery_warn_threshold_controller
                        {
                            if !sent_c_warn[s] {
                                let _ = notifier.notify(
                                    "Low Battery",
                                    &format!(
                                        "P{} Controller is at {}%",
                                        slot + 1,
                                        info.controller_battery
                                    ),
                                );
                                sent_c_warn[s] = true;
                            }
                        } else {
                            sent_c_warn[s] = false;
                        }

                        // Phone Warning
                        if info.phone_battery >= 0
                            && info.phone_battery as u8 <= cfg.battery_warn_threshold_phone
                        {
                            if !sent_p_warn[s] {
                                let _ = notifier.notify(
                                    "Low Battery",
                                    &format!("Phone battery is at {}%", info.phone_battery),
                                );
                                sent_p_warn[s] = true;
                            }
                        } else {
                            sent_p_warn[s] = false;
                        }
                    }
                    _ => {}
                }
            }
            Err(broadcast::error::RecvError::Lagged(_)) => continue,
            Err(broadcast::error::RecvError::Closed) => break,
        }
    }
}
