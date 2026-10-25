//! Clip-Bibliothek: Dateien im Zielordner, Metadaten-Cache, Thumbnails, Papierkorb.

use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use parking_lot::Mutex;
use serde::Serialize;
use ts_rs::TS;

use crate::error::{CoreError, CoreResult};
use crate::mux::media::{self, MediaInfo};
use crate::paths;

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct ClipInfo {
    pub path: String,
    pub file_name: String,
    /// Name des Unterordners (Spiel) oder leer.
    pub game: String,
    #[ts(type = "number")]
    pub size_bytes: u64,
    #[ts(type = "number")]
    pub modified_ms: u64,
    pub duration_secs: f64,
    pub width: u32,
    pub height: u32,
    /// Pfad zum Thumbnail, falls bereits erzeugt.
    pub thumbnail: Option<String>,
}

#[derive(Default)]
pub struct Library {
    /// (Größe, mtime) → Metadaten; vermeidet erneutes Öffnen unveränderter Dateien.
    cache: Mutex<HashMap<PathBuf, (u64, u64, MediaInfo)>>,
}

fn file_key(meta: &std::fs::Metadata) -> (u64, u64) {
    let mtime = meta
        .modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    (meta.len(), mtime)
}

pub fn thumbnail_path(path: &Path, size: u64, mtime: u64) -> CoreResult<PathBuf> {
    let mut h = std::collections::hash_map::DefaultHasher::new();
    path.to_string_lossy().to_lowercase().hash(&mut h);
    size.hash(&mut h);
    mtime.hash(&mut h);
    Ok(paths::thumbs_dir()?.join(format!("{:016x}.webp", h.finish())))
}

impl Library {
    /// Alle MP4 im Zielordner (eine Ebene Unterordner), neueste zuerst.
    pub fn list(&self, root: &Path) -> Vec<ClipInfo> {
        let mut files = Vec::new();
        collect_mp4(root, 0, &mut files);
        let mut out: Vec<ClipInfo> = files.into_iter().filter_map(|p| self.info(root, &p).ok()).collect();
        out.sort_by_key(|c| std::cmp::Reverse(c.modified_ms));
        out
    }

    pub fn info(&self, root: &Path, path: &Path) -> CoreResult<ClipInfo> {
        let meta = std::fs::metadata(path)?;
        let (size, mtime) = file_key(&meta);
        let cached = self.cache.lock().get(path).filter(|c| c.0 == size && c.1 == mtime).map(|c| c.2);
        let info = match cached {
            Some(i) => i,
            None => {
                let i = media::probe(path).unwrap_or_default();
                self.cache.lock().insert(path.to_path_buf(), (size, mtime, i));
                i
            }
        };
        let thumb = thumbnail_path(path, size, mtime).ok().filter(|t| t.exists());
        let game = path
            .parent()
            .filter(|p| *p != root)
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_default();
        Ok(ClipInfo {
            path: path.to_string_lossy().into_owned(),
            file_name: path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            game,
            size_bytes: size,
            modified_ms: mtime,
            duration_secs: info.duration_secs,
            width: info.width,
            height: info.height,
            thumbnail: thumb.map(|t| t.to_string_lossy().into_owned()),
        })
    }

    /// Erzeugt das Thumbnail, falls nötig (läuft auf dem MuxWorker).
    pub fn ensure_thumbnail(path: &Path) -> CoreResult<PathBuf> {
        let meta = std::fs::metadata(path)?;
        let (size, mtime) = file_key(&meta);
        let out = thumbnail_path(path, size, mtime)?;
        if !out.exists() {
            media::make_thumbnail(path, &out)?;
        }
        Ok(out)
    }

    pub fn forget(&self, path: &Path) {
        self.cache.lock().remove(path);
    }
}

fn collect_mp4(dir: &Path, depth: u32, out: &mut Vec<PathBuf>) {
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    for e in rd.flatten() {
        let p = e.path();
        let Ok(ft) = e.file_type() else { continue };
        if ft.is_dir() && depth < 1 {
            collect_mp4(&p, depth + 1, out);
        } else if ft.is_file() && p.extension().is_some_and(|x| x.eq_ignore_ascii_case("mp4")) {
            out.push(p);
        }
    }
}

/// Nur Dateien unterhalb des Zielordners dürfen gelöscht oder verändert werden.
pub fn ensure_inside(root: &Path, path: &Path) -> CoreResult<PathBuf> {
    let root = dunce_canon(root)?;
    let p = dunce_canon(path)?;
    if p.starts_with(&root) {
        Ok(p)
    } else {
        Err(CoreError::Other("Datei liegt außerhalb des Clip-Ordners".into()))
    }
}

fn dunce_canon(p: &Path) -> CoreResult<PathBuf> {
    let c = std::fs::canonicalize(p)?;
    // `\\?\`-Präfix entfernen, damit starts_with zuverlässig vergleicht.
    let s = c.to_string_lossy();
    Ok(PathBuf::from(s.strip_prefix(r"\\?\").unwrap_or(&s).to_string()))
}

/// In den Papierkorb verschieben (nicht endgültig löschen).
pub fn move_to_trash(path: &Path) -> CoreResult<()> {
    trash::delete(path).map_err(|e| CoreError::Other(format!("Papierkorb: {e}")))
}

/// Gesamtgröße und automatische Bereinigung (älteste zuerst, in den Papierkorb).
pub fn cleanup(clips: &[ClipInfo], max_age_days: u32, max_total_gb: u32) -> Vec<String> {
    let now = std::time::SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let mut removed = Vec::new();
    let mut sorted: Vec<&ClipInfo> = clips.iter().collect();
    sorted.sort_by_key(|c| c.modified_ms);
    let mut total: u64 = sorted.iter().map(|c| c.size_bytes).sum();
    let limit = u64::from(max_total_gb) * 1024 * 1024 * 1024;
    for c in sorted {
        let too_old = max_age_days > 0 && now.saturating_sub(c.modified_ms) > u64::from(max_age_days) * 86_400_000;
        let too_big = max_total_gb > 0 && total > limit;
        if (too_old || too_big) && move_to_trash(Path::new(&c.path)).is_ok() {
            total = total.saturating_sub(c.size_bytes);
            removed.push(c.path.clone());
        }
    }
    removed
}

/// Freier Speicher auf dem Laufwerk des Zielordners.
pub fn free_space_bytes(dir: &Path) -> Option<u64> {
    use windows::core::HSTRING;
    use windows::Win32::Storage::FileSystem::GetDiskFreeSpaceExW;
    let mut free = 0u64;
    let h = HSTRING::from(dir.as_os_str());
    // SAFETY: Ausgabezeiger gültig, Pfad nullterminiert.
    unsafe { GetDiskFreeSpaceExW(&h, Some(&mut free), None, None) }.ok()?;
    Some(free)
}
