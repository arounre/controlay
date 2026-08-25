// hides console from release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use anyhow::Result;
use native_dialog::{MessageDialogBuilder, MessageLevel};
use single_instance::SingleInstance;
use tray_icon::{
    TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

use crate::{
    config::AppConfig,
    core::{AppEvent, ServerCommand},
};

mod app;
mod config;
pub mod core;
mod debounce;
mod discovery;
mod licenses;
mod logic;
mod notifier;
mod protocol;
mod ui;
mod updater;

// icon metadata
include!(concat!(env!("OUT_DIR"), "/icon_meta.rs"));

// helper to show a native windows error dialog
fn show_fatal_error(title: &str, error: &anyhow::Error) {
    let _ = MessageDialogBuilder::default()
        .set_level(MessageLevel::Error)
        .set_title(title)
        .set_text(&format!("Application Error:\n{:#}", error))
        .alert()
        .show();
}

fn run_app() -> Result<()> {
    let instance = SingleInstance::new("controlay").ok();
    if instance.as_ref().is_some_and(|i| !i.is_single()) {
        let _ = MessageDialogBuilder::default()
            .set_level(MessageLevel::Info)
            .set_title("Already Running")
            .set_text("Another instance is already open.")
            .alert()
            .show();
        return Ok(());
    }

    let icon_rgba = include_bytes!(concat!(env!("OUT_DIR"), "/icon.rgba")).to_vec();

    let (event_tx, mut event_rx) = tokio::sync::broadcast::channel::<AppEvent>(100);
    let (cmd_tx, cmd_rx) = tokio::sync::mpsc::channel::<ServerCommand>(10);
    let (ui_tx, ui_rx) = std::sync::mpsc::channel();
    let (config_tx, config_rx) = tokio::sync::watch::channel(AppConfig::default());

    let is_visible = Arc::new(AtomicBool::new(true));
    let is_visible_tray = is_visible.clone();

    let (ctx_tx, ctx_rx) = tokio::sync::oneshot::channel::<egui::Context>();

    // Backend Runtime
    std::thread::spawn(move || {
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            // Spawn Background Notification Worker
            tokio::spawn(notifier::run_notification_service(
                event_tx.subscribe(),
                config_rx.clone(),
            ));

            // Update checker Worker
            tokio::task::spawn_blocking({
                let tx = event_tx.clone();
                move || updater::check_for_updates(tx)
            });

            // Start the Backend Manager
            tokio::spawn(logic::run_backend(cmd_rx, config_rx, event_tx.clone()));

            // Relay broadcast events back to ui thread
            if let Ok(ctx) = ctx_rx.await {
                loop {
                    match event_rx.recv().await {
                        Ok(event) => {
                            let _ = ui_tx.send(event);
                            ctx.request_repaint();
                        }
                        Err(tokio::sync::broadcast::error::RecvError::Lagged(_)) => continue,
                        Err(tokio::sync::broadcast::error::RecvError::Closed) => break,
                    }
                }
            }
        });
    });

    let native_options = eframe::NativeOptions {
        // wgpu is eframe's default. Stay on glow: smaller binary, and it avoids
        // the still-open Windows multi-monitor/DPI leak (emilk/egui#4674).
        renderer: eframe::Renderer::Glow,
        viewport: egui::ViewportBuilder::default()
            .with_inner_size(app::WINDOW_MIN_INNER)
            .with_min_inner_size(app::WINDOW_MIN_INNER)
            .with_icon(egui::IconData {
                rgba: icon_rgba.clone(),
                width: ICON_WIDTH,
                height: ICON_HEIGHT,
            }),
        ..Default::default()
    };

    eframe::run_native(
        "Controlay",
        native_options,
        Box::new(move |cc| {
            let saved_config: AppConfig = cc
                .storage
                .and_then(|s| eframe::get_value(s, eframe::APP_KEY))
                .unwrap_or_default();

            let _ = config_tx.send(saved_config.clone());

            let ctx = cc.egui_ctx.clone();
            let _ = ctx_tx.send(ctx.clone());

            let tray_img = tray_icon::Icon::from_rgba(icon_rgba, ICON_WIDTH, ICON_HEIGHT).unwrap();
            let menu = Menu::new();
            let show_item = MenuItem::new("Show/Hide Window", true, None);
            let quit_item = MenuItem::new("Quit Application", true, None);
            let _ = menu.append_items(&[&show_item, &PredefinedMenuItem::separator(), &quit_item]);

            let tray_icon = TrayIconBuilder::new()
                .with_menu(Box::new(menu))
                .with_tooltip("Controlay")
                .with_icon(tray_img)
                .build()
                .unwrap();

            let show_item_id = show_item.id().clone();
            let quit_item_id = quit_item.id().clone();

            TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
                if matches!(event, TrayIconEvent::DoubleClick { .. }) {
                    toggle_window(&ctx, &is_visible_tray);
                }
            }));

            let ctx_menu = cc.egui_ctx.clone();
            let is_visible_menu = is_visible.clone();
            MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
                if event.id() == &show_item_id {
                    toggle_window(&ctx_menu, &is_visible_menu);
                } else if event.id() == &quit_item_id {
                    std::process::exit(0);
                }
            }));

            Ok(Box::new(app::ControlayApp::new(
                cc,
                config_tx,
                cmd_tx,
                ui_rx,
                saved_config,
                is_visible,
                tray_icon,
            )))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Eframe crash: {}", e))?;

    Ok(())
}

fn toggle_window(ctx: &egui::Context, is_visible: &AtomicBool) {
    let current = is_visible.fetch_xor(true, Ordering::Relaxed);
    ctx.send_viewport_cmd(egui::ViewportCommand::Visible(!current));
    if !current {
        ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
    }
}

fn main() {
    std::panic::set_hook(Box::new(|info| {
        let location = info.location().unwrap_or(std::panic::Location::caller());

        let payload = if let Some(s) = info.payload().downcast_ref::<&str>() {
            format!("{}", s)
        } else if let Some(s) = info.payload().downcast_ref::<String>() {
            format!("{}", s)
        } else {
            "Unknown panic".to_string()
        };

        let msg = format!(
            "Controlay has crashed.\n\nError: {}\nLocation: {}:{}",
            payload,
            location.file(),
            location.line()
        );

        let _ = MessageDialogBuilder::default()
            .set_level(MessageLevel::Error)
            .set_title("Critical Error")
            .set_text(&msg)
            .alert()
            .show();
    }));

    if let Err(e) = run_app() {
        show_fatal_error("Controlay Startup Failed", &e);
        std::process::exit(1);
    }
}
