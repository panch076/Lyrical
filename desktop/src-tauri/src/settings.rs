use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// "acrylic" (Windows only) | "frosted" | "clear" | "tinted"
    pub glass: String,
    pub show_artwork: bool,
    pub surrounding_lines: bool,
    pub sweep: bool,
    pub font_scale: f64,
    /// Seconds; positive shows lyrics earlier.
    pub sync_offset: f64,
    pub click_through: bool,
    pub floating_visible: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            glass: if cfg!(windows) { "acrylic" } else { "frosted" }.into(),
            show_artwork: true,
            surrounding_lines: true,
            sweep: true,
            font_scale: 1.0,
            sync_offset: 0.0,
            click_through: false,
            floating_visible: true,
        }
    }
}

impl Settings {
    pub fn load(path: &PathBuf) -> Self {
        let mut settings: Settings = std::fs::read_to_string(path)
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default();
        if settings.glass == "acrylic" && !cfg!(windows) {
            settings.glass = "frosted".into();
        }
        settings.font_scale = settings.font_scale.clamp(0.7, 1.8);
        settings.sync_offset = settings.sync_offset.clamp(-5.0, 5.0);
        settings
    }

    pub fn save(&self, path: &PathBuf) {
        if let Some(parent) = path.parent() {
            let _ = std::fs::create_dir_all(parent);
        }
        if let Ok(text) = serde_json::to_string_pretty(self) {
            let _ = std::fs::write(path, text);
        }
    }

    /// Applies a partial JSON object of changed fields.
    pub fn merged(&self, patch: serde_json::Value) -> Self {
        let mut value = serde_json::to_value(self).unwrap_or_default();
        if let (Some(target), Some(changes)) = (value.as_object_mut(), patch.as_object()) {
            for (key, change) in changes {
                target.insert(key.clone(), change.clone());
            }
        }
        let mut merged: Settings = serde_json::from_value(value).unwrap_or_else(|_| self.clone());
        merged.font_scale = merged.font_scale.clamp(0.7, 1.8);
        merged.sync_offset = merged.sync_offset.clamp(-5.0, 5.0);
        merged
    }

    /// Smallest floating window that still fits the header and lyric rows.
    pub fn floating_min_size(&self) -> (f64, f64) {
        let row = (32.0 * self.font_scale).round();
        let rows = if self.surrounding_lines { 3.0 } else { 1.0 };
        let content: f64 = 16.0 + 6.0 + row * rows;
        let height = content.max(if self.show_artwork { 62.0 } else { 0.0 }) + (16.0 + 8.0) * 2.0;
        (if self.show_artwork { 460.0 } else { 380.0 }, height.ceil())
    }
}
