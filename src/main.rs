// hides console from release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{ffi::c_void, sync::Mutex};

use anyhow::{Context, Result};
use native_dialog::{MessageDialogBuilder, MessageLevel};
use single_instance::SingleInstance;
use tray_icon::{
    TrayIconBuilder, TrayIconEvent,
    menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem},
};

use winit::raw_window_handle::{HasWindowHandle, Win32WindowHandle};

mod app;
mod config;
mod discovery;
mod licenses;
mod logic;
mod notifier;
mod protocol;
mod ui;
mod updater;

// icon metadata
include!(concat!(env!("OUT_DIR"), "/icon_meta.rs"));

static VISIBLE: Mutex<bool> = Mutex::new(true);

// helper to show a native windows error dialog
fn show_fatal_error(title: &str, error: &anyhow::Error) {
    let _ = MessageDialogBuilder::default()
        .set_level(MessageLevel::Error)
        .set_title(title)
        .set_text(&format!("Application Error:\n{:#}", error))
        .alert()
        .show();
}

pub fn flip_window_visibility(handle: Win32WindowHandle) {
    // Solution for scuffed tray support comes from: https://github.com/emilk/egui/discussions/737

    let mut visible_mutex = VISIBLE.lock().unwrap();

    if *visible_mutex {
        let window_handle = windows::Win32::Foundation::HWND(handle.hwnd.get() as *mut c_void);
        let hide = windows::Win32::UI::WindowsAndMessaging::SW_HIDE;
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::ShowWindow(window_handle, hide);
        }
        *visible_mutex = false;
    } else {
        let window_handle = windows::Win32::Foundation::HWND(handle.hwnd.get() as *mut c_void);
        let show = windows::Win32::UI::WindowsAndMessaging::SW_SHOWDEFAULT;
        unsafe {
            let _ = windows::Win32::UI::WindowsAndMessaging::ShowWindow(window_handle, show);
            let _ = windows::Win32::UI::WindowsAndMessaging::SetForegroundWindow(window_handle);
        }
        *visible_mutex = true;
    }
}

fn run_app() -> Result<()> {
    let instance = SingleInstance::new("controlay").ok();
    if let Some(inst) = &instance {
        if !inst.is_single() {
            let _ = MessageDialogBuilder::default()
                .set_level(MessageLevel::Info)
                .set_title("Already Running")
                .set_text("Another instance of this application is already open.")
                .alert()
                .show();

            return Ok(());
        }
    }

    let icon_rgba = include_bytes!(concat!(env!("OUT_DIR"), "/icon.rgba")).to_vec();

    // Tray Menu setup
    let tray_img = tray_icon::Icon::from_rgba(icon_rgba.clone(), ICON_WIDTH, ICON_HEIGHT)
        .context("Failed to load tray icon image")?;

    let menu = Menu::new();
    let show_item = MenuItem::new("Show/Hide Window", true, None);
    let quit_item = MenuItem::new("Quit Application", true, None);

    menu.append(&show_item).context("Failed to build menu")?;
    menu.append(&PredefinedMenuItem::separator())
        .context("Failed to build menu")?;
    menu.append(&quit_item).context("Failed to build menu")?;

    let _tray_icon = TrayIconBuilder::new()
        .with_menu(Box::new(menu))
        .with_tooltip("Controlay")
        .with_icon(tray_img)
        .build()
        .context("Failed to build System Tray icon")?;

    // EGUI icon setup
    let icon = egui::IconData {
        rgba: icon_rgba.clone(),
        width: ICON_WIDTH,
        height: ICON_HEIGHT,
    };

    let native_options = eframe::NativeOptions {
        renderer: eframe::Renderer::Glow, // default backend (wgpu) had memory leak issues
        viewport: egui::ViewportBuilder::default()
            .with_min_inner_size([450.0, 350.0])
            .with_icon(icon),
        ..Default::default()
    };

    // Notification setup
    let notifier = notifier::AppNotifier::new("com.arounre.controlay").ok();

    eframe::run_native(
        "Controlay",
        native_options,
        Box::new(move |cc| {
            let winit::raw_window_handle::RawWindowHandle::Win32(handle) =
                cc.window_handle().unwrap().as_raw()
            else {
                panic!("Unsupported platform");
            };

            let show_item_id = show_item.id().clone();
            let quit_item_id = quit_item.id().clone();

            TrayIconEvent::set_event_handler(Some(move |event: TrayIconEvent| {
                if !matches!(event, TrayIconEvent::DoubleClick { .. }) {
                    return;
                }

                flip_window_visibility(handle);
            }));

            MenuEvent::set_event_handler(Some(move |event: MenuEvent| {
                let event_id = event.id();

                if *event_id == show_item_id {
                    flip_window_visibility(handle);
                } else if *event_id == quit_item_id {
                    std::process::exit(0);
                }
            }));

            Ok(Box::new(app::ControlayApp::new(cc, handle, notifier)))
        }),
    )
    .map_err(|e| anyhow::anyhow!("Eframe crash: {}", e))?;

    Ok(())
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
