use anyhow::Result;
use notify_rust::Notification;

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
