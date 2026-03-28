use anyhow::Context;
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

#[derive(Debug, Clone)]
pub struct AppPaths {
    pub models_dir: PathBuf,
    pub settings_path: PathBuf,
    pub models_state_path: PathBuf,
    pub history_path: PathBuf,
}

impl AppPaths {
    pub fn new(app: &AppHandle) -> anyhow::Result<Self> {
        let root = app
            .path()
            .app_data_dir()
            .context("failed to resolve app data directory")?;
        let models_dir = root.join("models");
        std::fs::create_dir_all(&models_dir).context("failed to create models directory")?;

        Ok(Self {
            models_dir,
            settings_path: root.join("settings.json"),
            models_state_path: root.join("models_state.json"),
            history_path: root.join("history.json"),
        })
    }
}
