use eframe::egui;
use egui_commonmark::{CommonMarkCache, CommonMarkViewer};

pub struct LicenseWindow {
    open: bool,
    active_tab: LicenseTab,
    app_license: String,
    third_party_license_text: String,

    markdown_cache: CommonMarkCache,
}

#[derive(PartialEq)]
enum LicenseTab {
    Application,
    ThirdParty,
}

impl Default for LicenseWindow {
    fn default() -> Self {
        // Main License
        let app_license = include_str!("../LICENSE").to_string();

        // Third Party
        let tp_license = include_str!("../thirdparty.md").to_string();

        Self {
            open: false,
            active_tab: LicenseTab::Application,
            app_license,
            third_party_license_text: tp_license,
            markdown_cache: CommonMarkCache::default(),
        }
    }
}

impl LicenseWindow {
    pub fn toggle(&mut self) {
        self.open = !self.open;
    }

    pub fn show(&mut self, ctx: &egui::Context) {
        let mut open = self.open;

        egui::Window::new("Licenses & Legal")
            .open(&mut open)
            .default_width(500.0)
            .default_height(400.0)
            .vscroll(false)
            .resizable(true)
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    ui.selectable_value(
                        &mut self.active_tab,
                        LicenseTab::Application,
                        "Controlay License",
                    );
                    ui.selectable_value(
                        &mut self.active_tab,
                        LicenseTab::ThirdParty,
                        "Third-Party / Open Source",
                    );
                });
                ui.separator();

                egui::ScrollArea::vertical()
                    .id_salt("license_scroll")
                    .show(ui, |ui| match self.active_tab {
                        LicenseTab::Application => {
                            ui.add(
                                egui::TextEdit::multiline(&mut self.app_license.as_str())
                                    .font(egui::TextStyle::Monospace)
                                    .desired_width(f32::INFINITY)
                                    .lock_focus(true),
                            );
                        }
                        LicenseTab::ThirdParty => {
                            CommonMarkViewer::new().show(
                                ui,
                                &mut self.markdown_cache,
                                &self.third_party_license_text,
                            );
                        }
                    });
            });

        self.open = open;
    }
}
