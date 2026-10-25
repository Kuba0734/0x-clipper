//! Laden und Speichern von `%APPDATA%\0x-clipper\settings.json`.

pub mod migrate;
pub mod schema;

use std::fs;
use std::path::{Path, PathBuf};

pub use schema::*;

use crate::error::{CoreError, CoreResult};
use crate::paths;

pub fn settings_path() -> CoreResult<PathBuf> {
    Ok(paths::config_dir()?.join("settings.json"))
}

/// Lädt die Einstellungen. Fehlende oder kaputte Dateien ergeben Defaults
/// (eine kaputte Datei wird als `.bak` gesichert, damit nichts verloren geht).
pub fn load() -> Settings {
    let path = match settings_path() {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!("Kein Konfigurationsordner: {e}");
            return Settings::default();
        }
    };
    match load_from(&path) {
        Ok(s) => s,
        Err(e) => {
            if path.exists() {
                tracing::warn!("settings.json unlesbar ({e}), sichere als .bak");
                let _ = fs::rename(&path, path.with_extension("json.bak"));
            }
            Settings::default()
        }
    }
}

pub fn load_from(path: &Path) -> CoreResult<Settings> {
    let text = fs::read_to_string(path)?;
    parse(&text)
}

pub fn parse(text: &str) -> CoreResult<Settings> {
    let raw: serde_json::Value = serde_json::from_str(text)?;
    let (value, from) = migrate::migrate(raw);
    let mut s: Settings = serde_json::from_value(value)?;
    if from != SETTINGS_VERSION {
        tracing::info!("Einstellungen von v{from} auf v{SETTINGS_VERSION} migriert");
    }
    s.sanitize();
    Ok(s)
}

/// Schreibt atomar (temporäre Datei + Rename), damit ein Absturz nie eine halbe Datei hinterlässt.
pub fn save(settings: &Settings) -> CoreResult<()> {
    let path = settings_path()?;
    save_to(settings, &path)?;
    paths::publish_clips_dir_for_uninstaller(&paths::output_dir(settings));
    Ok(())
}

pub fn save_to(settings: &Settings, path: &Path) -> CoreResult<()> {
    let dir = path.parent().ok_or_else(|| CoreError::Settings("Ungültiger Pfad".into()))?;
    fs::create_dir_all(dir)?;
    let tmp = path.with_extension("json.tmp");
    fs::write(&tmp, serde_json::to_vec_pretty(settings)?)?;
    fs::rename(&tmp, path)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_through_file() {
        let dir = std::env::temp_dir().join(format!("clipper-test-{}", std::process::id()));
        let path = dir.join("settings.json");
        let mut s = Settings::default();
        s.replay.duration_secs = 90;
        s.video.codec = Codec::Hevc;
        save_to(&s, &path).unwrap();
        let loaded = load_from(&path).unwrap();
        assert_eq!(loaded, s);
        let _ = fs::remove_dir_all(dir);
    }
}
