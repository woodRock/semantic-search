use serde::{Deserialize, Serialize};
use std::path::Path;
use std::fs;

#[derive(Serialize, Deserialize, Default)]
pub struct Settings {
    pub ignored_paths: Vec<String>,
}

impl Settings {
    pub fn load(app_data_dir: &Path) -> Self {
        let settings_path = app_data_dir.join("settings.json");
        if settings_path.exists() {
            let data = fs::read_to_string(settings_path).unwrap_or_default();
            serde_json::from_str(&data).unwrap_or_default()
        } else {
            Self::default()
        }
    }

    pub fn save(&self, app_data_dir: &Path) -> anyhow::Result<()> {
        let settings_path = app_data_dir.join("settings.json");
        let data = serde_json::to_string_pretty(self)?;
        fs::write(settings_path, data)?;
        Ok(())
    }
}
