use crate::core::AppEvent;
use anyhow::{Context, Result};
use serde::Deserialize;
use tokio::sync::broadcast;

pub const REPO_OWNER: &str = "arounre";
pub const REPO_NAME: &str = "controlay";

#[derive(Deserialize)]
struct Release {
    tag_name: String,
    html_url: String,
}

#[derive(Clone)]
pub struct UpdateInfo {
    pub version: String,
    pub url: String,
}

pub fn check_for_updates(event_tx: broadcast::Sender<AppEvent>) {
    let current_version = env!("CARGO_PKG_VERSION").to_string();
    match internal_check(&current_version) {
        Ok(Some(info)) => {
            let _ = event_tx.send(AppEvent::UpdateAvailable(info));
        }
        Ok(None) => {}
        Err(e) => {
            let _ = event_tx.send(AppEvent::Log(format!("Update check failed: {}", e)));
        }
    }
}

fn internal_check(current_ver: &str) -> Result<Option<UpdateInfo>> {
    let client = reqwest::blocking::Client::builder()
        .user_agent("Controlay-Updater")
        .build()?;

    let url = format!("https://api.github.com/repos/{REPO_OWNER}/{REPO_NAME}/releases/latest");

    let resp = client
        .get(&url)
        .send()
        .context("Failed to query GitHub API")?;

    // If we hit this, just silently fail instead of showing an error.
    if resp.status() == reqwest::StatusCode::FORBIDDEN {
        return Ok(None);
    }

    // Convert other non-200 status codes into an actual Err
    let resp = resp.error_for_status()?;
    let release: Release = resp.json()?;

    let remote_ver_str = release.tag_name.trim_start_matches('v');
    let remote = semver::Version::parse(remote_ver_str)?;
    let current = semver::Version::parse(current_ver)?;

    if remote > current {
        Ok(Some(UpdateInfo {
            version: remote_ver_str.to_string(),
            url: release.html_url,
        }))
    } else {
        Ok(None)
    }
}
