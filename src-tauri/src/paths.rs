//! Pfade und Dateinamen.

use std::path::{Path, PathBuf};

use windows::core::{w, HSTRING};
use windows::Win32::System::Registry::{RegSetKeyValueW, HKEY_CURRENT_USER, REG_SZ};

use crate::error::{CoreError, CoreResult};
use crate::settings::Settings;

pub const APP_DIR: &str = "0x-clipper";

/// `%APPDATA%\0x-clipper`
pub fn config_dir() -> CoreResult<PathBuf> {
    dirs::config_dir()
        .map(|d| d.join(APP_DIR))
        .ok_or_else(|| CoreError::Settings("%APPDATA% nicht gefunden".into()))
}

/// `%LOCALAPPDATA%\0x-clipper`
pub fn local_dir() -> CoreResult<PathBuf> {
    dirs::data_local_dir()
        .map(|d| d.join(APP_DIR))
        .ok_or_else(|| CoreError::Settings("%LOCALAPPDATA% nicht gefunden".into()))
}

pub fn logs_dir() -> CoreResult<PathBuf> {
    Ok(local_dir()?.join("logs"))
}

pub fn thumbs_dir() -> CoreResult<PathBuf> {
    Ok(local_dir()?.join("thumbs"))
}

/// Zielordner der Clips; Standard `%USERPROFILE%\Videos\0x-clipper`.
pub fn output_dir(settings: &Settings) -> PathBuf {
    let custom = settings.output.folder.trim();
    if !custom.is_empty() {
        return PathBuf::from(custom);
    }
    dirs::video_dir()
        .or_else(|| dirs::home_dir().map(|h| h.join("Videos")))
        .unwrap_or_else(|| PathBuf::from("."))
        .join(APP_DIR)
}

/// Der Deinstaller liest den Clip-Ordner aus der Registry, um ihn optional zu löschen.
pub fn publish_clips_dir_for_uninstaller(dir: &Path) {
    let value: Vec<u16> =
        dir.as_os_str().to_string_lossy().encode_utf16().chain(std::iter::once(0)).collect();
    // SAFETY: Alle Zeiger zeigen auf gültige, nullterminierte UTF-16-Puffer, die während
    // des Aufrufs leben; die Länge ist in Bytes inklusive Terminator angegeben.
    let res = unsafe {
        RegSetKeyValueW(
            HKEY_CURRENT_USER,
            &HSTRING::from(r"Software\0x-clipper"),
            w!("ClipsDir"),
            REG_SZ.0,
            Some(value.as_ptr().cast()),
            (value.len() * 2) as u32,
        )
    };
    if res.is_err() {
        tracing::debug!("ClipsDir konnte nicht in die Registry geschrieben werden: {res:?}");
    }
}

/// Art der Datei für den `{type}`-Platzhalter.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClipKind {
    Clip,
    Recording,
}

/// Ersetzt die Platzhalter im Dateinamensmuster und entfernt ungültige Zeichen.
pub fn render_filename(
    pattern: &str,
    game: &str,
    kind: ClipKind,
    now: chrono::DateTime<chrono::Local>,
) -> String {
    let kind = match kind {
        ClipKind::Clip => "Clip",
        ClipKind::Recording => "Aufnahme",
    };
    let name = pattern
        .replace("{game}", game)
        .replace("{date}", &now.format("%Y-%m-%d").to_string())
        .replace("{time}", &now.format("%H-%M-%S").to_string())
        .replace("{type}", kind);
    let cleaned = sanitize_component(&name);
    if cleaned.is_empty() {
        format!("{kind}_{}", now.format("%Y-%m-%d_%H-%M-%S"))
    } else {
        cleaned
    }
}

/// Entfernt Zeichen, die Windows in Dateinamen nicht erlaubt, und reservierte Namen.
pub fn sanitize_component(s: &str) -> String {
    let mut out: String = s
        .chars()
        .map(|c| match c {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            c if c.is_control() => '_',
            c => c,
        })
        .collect();
    while out.ends_with(['.', ' ']) {
        out.pop();
    }
    let upper = out.to_ascii_uppercase();
    let reserved = ["CON", "PRN", "AUX", "NUL"];
    let numbered = (upper.starts_with("COM") || upper.starts_with("LPT"))
        && upper.len() == 4
        && upper.as_bytes()[3].is_ascii_digit();
    if reserved.contains(&upper.as_str()) || numbered {
        out.insert(0, '_');
    }
    out.chars().take(150).collect()
}

/// Ziel für eine neue Datei: Ordner (+ Unterordner pro Spiel) und eindeutiger Name.
pub fn new_output_file(settings: &Settings, game: &str, kind: ClipKind) -> CoreResult<PathBuf> {
    let mut dir = output_dir(settings);
    if settings.output.subfolder_per_game {
        let sub = sanitize_component(game);
        if !sub.is_empty() {
            dir.push(sub);
        }
    }
    std::fs::create_dir_all(&dir)?;
    let base = render_filename(&settings.output.filename_pattern, game, kind, chrono::Local::now());
    let mut path = dir.join(format!("{base}.mp4"));
    let mut n = 2;
    while path.exists() {
        path = dir.join(format!("{base} ({n}).mp4"));
        n += 1;
    }
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn renders_pattern() {
        let t = chrono::Local.with_ymd_and_hms(2026, 3, 4, 5, 6, 7).unwrap();
        let n = render_filename("{game}_{date}_{time}", "Valorant", ClipKind::Clip, t);
        assert_eq!(n, "Valorant_2026-03-04_05-06-07");
        let n = render_filename("{type}-{game}", "a:b", ClipKind::Recording, t);
        assert_eq!(n, "Aufnahme-a_b");
    }

    #[test]
    fn sanitizes_reserved_and_trailing() {
        assert_eq!(sanitize_component("CON"), "_CON");
        assert_eq!(sanitize_component("com1"), "_com1");
        assert_eq!(sanitize_component("Spiel. "), "Spiel");
        assert_eq!(sanitize_component("a/b\\c"), "a_b_c");
    }

    #[test]
    fn empty_pattern_falls_back() {
        let t = chrono::Local.with_ymd_and_hms(2026, 1, 1, 0, 0, 0).unwrap();
        assert_eq!(render_filename("???", "", ClipKind::Clip, t), "___");
        assert!(render_filename(". .", "", ClipKind::Clip, t).starts_with("Clip_"));
    }
}
