use crate::app::ControlayApp;
use crate::app::Tab;
use crate::config::{
    ANTI_DOUBLE_CLICK_WINDOW_MAX_MS, ANTI_DOUBLE_CLICK_WINDOW_MIN_MS, BTN_A, BTN_B, BTN_BACK,
    BTN_DPAD_DOWN, BTN_DPAD_LEFT, BTN_DPAD_RIGHT, BTN_DPAD_UP, BTN_GUIDE, BTN_LEFT_SHOULDER,
    BTN_LEFT_THUMB, BTN_RIGHT_SHOULDER, BTN_RIGHT_THUMB, BTN_START, BTN_X, BTN_Y, ControllerType,
    ThemeMode,
};
use crate::core::ReceiverState;
use eframe::egui::{self, Ui};
use egui::RichText;
use egui::Vec2;

const VIGEM_URL: &str = "https://github.com/nefarius/ViGEmBus/releases/latest";

pub fn draw_main_layout(app: &mut ControlayApp, ui: &mut Ui) {
    ui.horizontal(|ui| {
        ui.group(|ui| {
            ui.selectable_value(&mut app.active_tab, Tab::Home, "Home");
            ui.selectable_value(&mut app.active_tab, Tab::Settings, "Settings");
            ui.selectable_value(&mut app.active_tab, Tab::About, "About");
        });
    });

    let margin = ui.spacing().window_margin;
    egui::Area::new("controllers_overlay".into())
        .anchor(
            egui::Align2::RIGHT_TOP,
            Vec2::new(-margin.right as f32, margin.top as f32),
        )
        .order(egui::Order::Foreground)
        .show(ui.ctx(), |ui| {
            ui.vertical(|ui| {
                let mut any_connected = false;
                let mut disconnect_requests = Vec::new();

                for (i, slot) in app.controllers.iter().enumerate() {
                    if let Some(state) = slot {
                        any_connected = true;

                        egui::Frame::group(ui.style())
                            .fill(ui.visuals().window_fill())
                            .show(ui, |ui| {
                                ui.horizontal(|ui| {
                                    ui.label(
                                        RichText::new(format!("P{}", i + 1)).size(11.0).strong(),
                                    );
                                    ui.add_space(4.0);

                                    let is_active = state.is_active;

                                    ui.add_enabled_ui(is_active, |ui| {
                                        ui.label(RichText::new("🎮").size(14.0));
                                        draw_battery_icon(
                                            ui,
                                            state.info.controller_battery,
                                            "Controller",
                                            is_active,
                                        );
                                        ui.add_space(4.0);

                                        ui.label(RichText::new("📱").size(14.0));
                                        draw_battery_icon(
                                            ui,
                                            state.info.phone_battery,
                                            "Phone",
                                            is_active,
                                        );
                                    });

                                    ui.add_space(4.0);

                                    if ui
                                        .button(RichText::new("❌").size(10.0))
                                        .on_hover_text("Disconnect Controller")
                                        .clicked()
                                    {
                                        disconnect_requests.push(i as u8);
                                    }
                                });
                            });

                        ui.add_space(4.0);
                    }
                }

                // Process disconnect requests
                for slot_id in disconnect_requests {
                    app.force_disconnect(slot_id);
                }

                if !any_connected {
                    egui::Frame::group(ui.style())
                        .fill(ui.visuals().window_fill())
                        .show(ui, |ui| {
                            ui.label(RichText::new("No Devices").color(egui::Color32::DARK_GRAY));
                        });
                }
            });
        });

    ui.add_space(10.0);

    if app.pending_update.is_some() && !app.show_update_alert {
        egui::Panel::bottom("update_footer")
            .show_separator_line(false)
            .frame(egui::Frame::NONE.inner_margin(egui::Margin::same(5)))
            .show(ui, |ui| {
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    if ui
                        .link(RichText::new("New update available").small())
                        .on_hover_text("Click to view update details")
                        .clicked()
                    {
                        app.show_update_alert = true;
                    }
                });
            });
    }

    match app.active_tab {
        Tab::Home => draw_home_tab(app, ui),
        Tab::Settings => draw_settings_tab(app, ui),
        Tab::About => draw_about_tab(app, ui),
    }

    ui.add_space(10.0);
}
pub fn draw_home_tab(app: &mut ControlayApp, ui: &mut Ui) {
    ui.vertical_centered(|ui| {
        ui.add_space(20.0);

        match app.receiver_state {
            ReceiverState::Off => {
                let btn_size = egui::vec2(150.0, 50.0);
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new("BROADCAST")
                                .size(20.0)
                                .color(egui::Color32::WHITE),
                        )
                        .min_size(btn_size)
                        .fill(egui::Color32::from_rgb(0, 100, 200)),
                    )
                    .clicked()
                {
                    app.start_broadcasting();
                }
                ui.label("Click to start broadcasting.");
            }
            ReceiverState::Starting => {
                ui.spinner();
                ui.label("Initializing...");
            }
            ReceiverState::On => {
                let btn_size = egui::vec2(150.0, 50.0);
                if ui
                    .add(
                        egui::Button::new(
                            RichText::new("STOP").size(20.0).color(egui::Color32::WHITE),
                        )
                        .min_size(btn_size)
                        .fill(egui::Color32::from_rgb(200, 50, 50)),
                    )
                    .clicked()
                {
                    app.stop_broadcasting();
                }

                let active_count = app
                    .controllers
                    .iter()
                    .flatten()
                    .filter(|state| state.is_active)
                    .count();

                if active_count > 0 {
                    ui.label(
                        RichText::new(format!("{} Controllers Active", active_count))
                            .color(egui::Color32::GREEN),
                    );
                } else {
                    ui.label("Waiting for connection...");
                }
            }
        }
    });

    ui.with_layout(egui::Layout::top_down(egui::Align::LEFT), |ui| {
        ui.separator();
        ui.label(RichText::new("Status Log:").strong());
        egui::ScrollArea::vertical()
            .stick_to_bottom(true)
            .show(ui, |ui| {
                for msg in &app.status_log {
                    ui.label(msg);
                }
            });
    });
}

pub fn draw_settings_tab(app: &mut ControlayApp, ui: &mut Ui) {
    let is_running = app.receiver_state != ReceiverState::Off;
    let mut send_live_update = false;

    egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .show(ui, |ui| {
            ui.add_space(10.0);

            ui.label(
                RichText::new("HARDWARE CONFIGURATION")
                    .strong()
                    .size(12.0)
                    .color(ui.visuals().weak_text_color()),
            );
            ui.add_space(5.0);

            ui.group(|ui| {
                if is_running {
                    ui.disable();
                }

                ui.label(RichText::new("Host Name").strong());
                ui.horizontal(|ui| {
                    ui.checkbox(&mut app.config.use_default_hostname, "Use Default");

                    let text_edit = egui::TextEdit::singleline(&mut app.config.hostname)
                        .hint_text("e.g. My-PC-Receiver")
                        .desired_width(200.0);

                    ui.add_enabled(!app.config.use_default_hostname, text_edit);
                });

                if app.config.use_default_hostname {
                    ui.small(RichText::new("Using system network name.").weak());
                }

                ui.add_space(8.0);
                ui.separator();
                ui.add_space(8.0);

                ui.label(RichText::new("Connection").strong());
                ui.horizontal(|ui| {
                    ui.checkbox(&mut app.config.use_custom_port, "Manual Port");
                    if app.config.use_custom_port {
                        ui.add(egui::DragValue::new(&mut app.config.port).range(1024..=65535));
                    } else {
                        ui.label(RichText::new("(Random Available)").italics().weak());
                    }
                });
            });

            if is_running {
                ui.add_space(5.0);
                ui.label(
                    RichText::new("⚠ Stop broadcast to change hardware configuration.")
                        .color(egui::Color32::ORANGE)
                        .size(11.0),
                );
            }

            ui.add_space(20.0);

            ui.label(
                RichText::new("CONTROLLER PROFILES")
                    .strong()
                    .size(12.0)
                    .color(ui.visuals().weak_text_color()),
            );
            ui.add_space(5.0);

            ui.horizontal(|ui| {
                ui.label("Editing Slot:");
                for i in 0..4 {
                    ui.selectable_value(&mut app.selected_slot, i, format!("P{}", i + 1));
                }
            });
            ui.separator();

            let profile = &mut app.config.profiles[app.selected_slot];

            ui.label(RichText::new("Emulation").strong());
            ui.horizontal(|ui| {
                ui.add_enabled_ui(!is_running, |ui| {
                    ui.radio_value(
                        &mut profile.controller_type,
                        ControllerType::X360,
                        "Xbox 360",
                    );
                    ui.radio_value(
                        &mut profile.controller_type,
                        ControllerType::DS4,
                        "DualShock 4",
                    );
                });
            });
            ui.small(
                RichText::new("Determines how the device appears to Windows. Requires restart.")
                    .weak(),
            );

            ui.add_space(10.0);

            ui.label(RichText::new("Stick Deadzones (Live)").strong());
            ui.indent("deadzone_indent", |ui| {
                ui.add_space(5.0);

                let r_inner = ui.add(
                    egui::Slider::new(&mut profile.deadzone.inner_percent, 0.0..=40.0)
                        .suffix("%")
                        .text("Inner")
                        .trailing_fill(true)
                        .clamping(egui::SliderClamping::Always),
                );
                if r_inner.drag_stopped() || r_inner.lost_focus() {
                    send_live_update = true;
                }

                if profile.deadzone.outer_percent < profile.deadzone.inner_percent {
                    profile.deadzone.outer_percent = profile.deadzone.inner_percent;
                }

                let r_outer = ui.add(
                    egui::Slider::new(
                        &mut profile.deadzone.outer_percent,
                        profile.deadzone.inner_percent..=100.0,
                    )
                    .suffix("%")
                    .text("Outer")
                    .trailing_fill(true)
                    .clamping(egui::SliderClamping::Always),
                );
                if r_outer.drag_stopped() || r_outer.lost_focus() {
                    send_live_update = true;
                }
            });

            ui.add_space(10.0);

            ui.label(RichText::new("Haptics (Live)").strong());
            ui.indent("rumble_indent", |ui| {
                ui.add_space(5.0);
                let r_rumble = ui.add(
                    egui::Slider::new(&mut profile.rumble_strength, 0.0..=200.0)
                        .integer()
                        .suffix("%")
                        .text("Rumble Strength")
                        .trailing_fill(true)
                        .clamping(egui::SliderClamping::Always),
                );
                if r_rumble.drag_stopped() || r_rumble.lost_focus() {
                    send_live_update = true;
                }
            });

            ui.add_space(10.0);

            ui.label(RichText::new("Anti Double-Click (Live)").strong());
            ui.indent("anti_double_click_indent", |ui| {
                ui.add_space(5.0);

                let r_enabled = ui.checkbox(
                    &mut profile.anti_double_click.enabled,
                    "Filter repeated presses",
                );
                if r_enabled.changed() {
                    send_live_update = true;
                }

                ui.add_enabled_ui(profile.anti_double_click.enabled, |ui| {
                    ui.add_space(5.0);

                    if profile.anti_double_click.window_ms < ANTI_DOUBLE_CLICK_WINDOW_MIN_MS {
                        profile.anti_double_click.window_ms = ANTI_DOUBLE_CLICK_WINDOW_MIN_MS;
                    }

                    let r_window = ui.add(
                        egui::Slider::new(
                            &mut profile.anti_double_click.window_ms,
                            ANTI_DOUBLE_CLICK_WINDOW_MIN_MS..=ANTI_DOUBLE_CLICK_WINDOW_MAX_MS,
                        )
                        .integer()
                        .suffix(" ms")
                        .text("Window")
                        .trailing_fill(true)
                        .clamping(egui::SliderClamping::Always),
                    );
                    if r_window.drag_stopped() || r_window.lost_focus() {
                        send_live_update = true;
                    }

                    ui.add_space(5.0);
                    ui.label(RichText::new("Buttons").strong());
                    ui.horizontal_wrapped(|ui| {
                        for (bit, label) in [
                            (BTN_DPAD_UP, "Up"),
                            (BTN_DPAD_DOWN, "Down"),
                            (BTN_DPAD_LEFT, "Left"),
                            (BTN_DPAD_RIGHT, "Right"),
                            (BTN_A, "A"),
                            (BTN_B, "B"),
                            (BTN_X, "X"),
                            (BTN_Y, "Y"),
                            (BTN_LEFT_SHOULDER, "LB"),
                            (BTN_RIGHT_SHOULDER, "RB"),
                            (BTN_LEFT_THUMB, "LS"),
                            (BTN_RIGHT_THUMB, "RS"),
                            (BTN_START, "Start"),
                            (BTN_BACK, "Back"),
                            (BTN_GUIDE, "Guide"),
                        ] {
                            if button_mask_toggle(ui, &mut profile.anti_double_click.buttons, bit, label)
                            {
                                send_live_update = true;
                            }
                        }
                    });
                });

                ui.add_space(4.0);
                ui.small(
                    RichText::new(
                        "Drops a second press of the same button if it arrives inside the window. Releases are never delayed. Keep the window below your tap speed so real mashing is not dropped.",
                    )
                    .weak(),
                );
            });

            ui.add_space(20.0);

            ui.label(
                RichText::new("NOTIFICATIONS")
                    .strong()
                    .size(12.0)
                    .color(ui.visuals().weak_text_color()),
            );
            ui.add_space(5.0);
            ui.group(|ui| {
                ui.checkbox(
                    &mut app.config.enable_connection_notifications,
                    "Enable Connection Alerts",
                );
                ui.add_space(5.0);
                ui.checkbox(
                    &mut app.config.enable_battery_notifications,
                    "Enable Battery Warnings",
                );

                ui.add_enabled_ui(app.config.enable_battery_notifications, |ui| {
                    ui.add_space(5.0);
                    ui.label("Controller Threshold:");
                    ui.add(
                        egui::Slider::new(
                            &mut app.config.battery_warn_threshold_controller,
                            5..=50,
                        )
                        .trailing_fill(true)
                        .suffix("%"),
                    );

                    ui.add_space(5.0);
                    ui.label("Phone Threshold:");
                    ui.add(
                        egui::Slider::new(&mut app.config.battery_warn_threshold_phone, 5..=50)
                            .trailing_fill(true)
                            .suffix("%"),
                    );
                });
            });

            ui.add_space(20.0);

            ui.label(
                RichText::new("APPLICATION")
                    .strong()
                    .size(12.0)
                    .color(ui.visuals().weak_text_color()),
            );
            ui.add_space(5.0);

            ui.indent("app_indent", |ui| {
                ui.checkbox(
                    &mut app.config.auto_start,
                    "Start broadcasting automatically on launch",
                );
                ui.add_space(10.0);
                ui.checkbox(
                    &mut app.config.check_updates,
                    "Check for updates on startup",
                );
                ui.add_space(10.0);
                ui.checkbox(&mut app.config.tray_on_close, "Minimizes to Tray on Close");
                ui.add_space(10.0);

                ui.horizontal(|ui| {
                    ui.label("Theme:");
                    egui::ComboBox::from_id_salt("theme_selector")
                        .selected_text(match app.config.theme {
                            ThemeMode::System => "System",
                            ThemeMode::Light => "Light",
                            ThemeMode::Dark => "Dark",
                        })
                        .show_ui(ui, |ui| {
                            ui.selectable_value(&mut app.config.theme, ThemeMode::System, "System");
                            ui.selectable_value(&mut app.config.theme, ThemeMode::Light, "Light");
                            ui.selectable_value(&mut app.config.theme, ThemeMode::Dark, "Dark");
                        });
                });
            });

            ui.add_space(20.0);
        });

    if send_live_update && is_running {
        app.trigger_live_update();
    }
}

pub fn draw_about_tab(app: &mut ControlayApp, ui: &mut Ui) {
    use crate::updater::{REPO_NAME, REPO_OWNER};

    ui.with_layout(egui::Layout::top_down(egui::Align::Center), |ui| {
        ui.add_space(20.0);

        ui.heading(RichText::new("Controlay").size(24.0));
        ui.label(format!("Version {}", env!("CARGO_PKG_VERSION")));

        ui.add_space(20.0);
        ui.label("Author: arounre");
        ui.add_space(25.0);

        ui.hyperlink_to(
            "GitHub Repository",
            format!("https://github.com/{REPO_OWNER}/{REPO_NAME}"),
        );

        ui.add_space(10.0);

        if ui.button("📄 View Licenses & Third-Party Info").clicked() {
            app.license_window.toggle();
        }
    });
}

fn button_mask_toggle(ui: &mut Ui, mask: &mut u16, bit: u16, label: &str) -> bool {
    let selected = *mask & bit != 0;
    let response = ui.add(egui::Button::new(label).selected(selected));
    if response.clicked() {
        *mask ^= bit;
        true
    } else {
        false
    }
}

fn draw_battery_icon(ui: &mut Ui, percentage: i8, label: &str, is_active: bool) {
    let width = 24.0;
    let height = 12.0;
    let tip_width = 2.0;
    let tip_height = 6.0;

    let (rect, response) = ui.allocate_exact_size(
        egui::vec2(width + tip_width + 5.0, height),
        egui::Sense::hover(),
    );

    response.on_hover_text(format!("{}: {}%", label, percentage));

    if ui.is_rect_visible(rect) {
        let painter = ui.painter();

        let fill_color = if !is_active {
            ui.visuals().weak_text_color()
        } else if percentage > 50 {
            egui::Color32::GREEN
        } else if percentage > 20 {
            egui::Color32::YELLOW
        } else {
            egui::Color32::RED
        };

        let stroke_color = if is_active {
            ui.visuals().text_color()
        } else {
            ui.visuals().weak_text_color()
        };

        let body_rect = egui::Rect::from_min_size(rect.min, egui::vec2(width, height));

        painter.rect_stroke(
            body_rect,
            2.0,
            egui::Stroke::new(1.0f32, stroke_color),
            egui::StrokeKind::Inside,
        );

        let fill_width = (width - 2.0) * (percentage as f32 / 100.0).clamp(0.0, 1.0);
        let fill_rect = egui::Rect::from_min_size(
            body_rect.min + egui::vec2(1.0, 1.0),
            egui::vec2(fill_width, height - 2.0),
        );
        painter.rect_filled(fill_rect, 2.0, fill_color);

        let tip_rect = egui::Rect::from_min_size(
            egui::pos2(body_rect.max.x, body_rect.center().y - tip_height / 2.0),
            egui::vec2(tip_width, tip_height),
        );
        painter.rect_filled(tip_rect, 1.0, stroke_color);
    }
}

pub fn draw_driver_alert(app: &mut ControlayApp, ctx: &egui::Context) {
    let mut open = app.show_driver_alert;
    let mut close_requested = false;

    egui::Window::new("Broadcast Failed")
        .open(&mut open)
        .collapsible(false)
        .resizable(false)
        .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
        .show(ctx, |ui| {
            ui.set_max_width(320.0);

            ui.vertical_centered(|ui| {
                ui.add_space(10.0);
                ui.label(RichText::new("🚫").size(48.0));
                ui.add_space(10.0);
                ui.heading(RichText::new("Driver Missing").strong().size(18.0));
                ui.add_space(10.0);
                ui.label("Controlay cannot start because the ViGEmBus driver was not found.");
                ui.add_space(5.0);
                ui.label("Please install the driver and try again.");
                ui.add_space(20.0);

                ui.horizontal(|ui| {
                    let total_width = 160.0 + ui.spacing().item_spacing.x + 80.0;
                    ui.add_space((ui.available_width() - total_width) / 2.0);

                    let btn_download = egui::Button::new(
                        RichText::new("⬇ Download")
                            .size(14.0)
                            .color(egui::Color32::WHITE),
                    )
                    .min_size(egui::vec2(160.0, 30.0))
                    .fill(egui::Color32::from_rgb(0, 100, 200));

                    if ui.add(btn_download).clicked() {
                        ui.ctx().open_url(egui::output::OpenUrl::new_tab(VIGEM_URL));
                        close_requested = true;
                    }

                    let btn_dismiss = egui::Button::new("Dismiss").min_size(egui::vec2(80.0, 30.0));

                    if ui.add(btn_dismiss).clicked() {
                        close_requested = true;
                    }
                });

                ui.add_space(5.0);
                ui.small(RichText::new("Opens GitHub Releases in your browser").weak());
                ui.add_space(20.0);
            });
        });

    if close_requested {
        open = false;
    }
    app.show_driver_alert = open;
}

pub fn draw_update_alert(app: &mut ControlayApp, ctx: &egui::Context) {
    let mut open = app.show_update_alert;
    let mut close_requested = false;

    if let Some(info) = &app.pending_update {
        egui::Window::new("Update Available")
            .open(&mut open)
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, egui::vec2(0.0, 0.0))
            .show(ctx, |ui| {
                ui.set_max_width(320.0);

                ui.vertical_centered(|ui| {
                    ui.add_space(10.0);
                    ui.heading(RichText::new("New Version Available").strong().size(18.0));
                    ui.add_space(5.0);
                    ui.label(format!("Version {} is now available.", info.version));
                    ui.label(
                        RichText::new(format!("Current: {}", env!("CARGO_PKG_VERSION")))
                            .weak()
                            .size(11.0),
                    );

                    ui.add_space(20.0);

                    ui.horizontal(|ui| {
                        let total_width = 130.0 + ui.spacing().item_spacing.x + 80.0;
                        ui.add_space((ui.available_width() - total_width) / 2.0);

                        let btn_download = egui::Button::new(
                            RichText::new("View on GitHub").color(egui::Color32::WHITE),
                        )
                        .min_size(egui::vec2(130.0, 30.0))
                        .fill(egui::Color32::from_rgb(0, 100, 200));

                        if ui.add(btn_download).clicked() {
                            ui.ctx().open_url(egui::output::OpenUrl::new_tab(&info.url));
                            close_requested = true;
                        }

                        let btn_dismiss =
                            egui::Button::new("Dismiss").min_size(egui::vec2(80.0, 30.0));

                        if ui.add(btn_dismiss).clicked() {
                            app.config.dismissed_update_version = Some(info.version.clone());
                            close_requested = true;
                        }
                    });

                    ui.add_space(10.0);
                });
            });
    }

    if close_requested {
        open = false;
    }
    app.show_update_alert = open;
}
