//! Tauri-Commands. Dünn: nur Delegation an Core, Library und Hilfsmodule.

use std::path::{Path, PathBuf};
use std::time::Instant;

use tauri::{AppHandle, Emitter, Manager, State};
use tauri_plugin_autostart::ManagerExt;

use crate::audio::wasapi::{self, AudioDevices};
use crate::capture::{self, MonitorInfo, WindowInfo};
use crate::controller::{Cmd, Core};
use crate::encode::EncoderInfo;
use crate::error::{CoreError, CoreResult};
use crate::events;
use crate::hotkeys::{self, HotkeyConflict};
use crate::library::{self, ClipInfo, Library};
use crate::mux::media;
use crate::settings::{self, HotkeySettings, Settings};
use crate::stats::Status;

#[tauri::command]
pub fn get_settings(core: State<'_, Core>) -> Settings {
    core.shared.settings.read().clone()
}

/// Speichert neue Einstellungen und wendet sie an. Gibt die bereinigte Version zurück.
#[tauri::command]
pub fn update_settings(app: AppHandle, core: State<'_, Core>, mut settings: Settings) -> CoreResult<Settings> {
    settings.sanitize();
    let old = core.shared.settings.read().clone();
    if old.hotkeys != settings.hotkeys {
        let conflicts = hotkeys::register_all(&app, &settings.hotkeys);
        let _ = app.emit(events::HOTKEY_CONFLICTS, &conflicts);
    }
    if old.general.autostart != settings.general.autostart {
        let al = app.autolaunch();
        let r = if settings.general.autostart { al.enable() } else { al.disable() };
        if let Err(e) = r {
            tracing::warn!("Autostart: {e}");
        }
    }
    if old.output.folder != settings.output.folder {
        allow_clip_dir(&app, &crate::paths::output_dir(&settings));
    }
    // Sofort in den geteilten Zustand, damit ein direkt folgendes get_settings stimmt.
    *core.shared.settings.write() = settings.clone();
    core.send(Cmd::ApplySettings(Box::new(settings.clone())));
    if old.general.language != settings.general.language {
        crate::tray::refresh(&app);
    }
    Ok(settings)
}

#[tauri::command]
pub fn reset_settings(app: AppHandle, core: State<'_, Core>) -> CoreResult<Settings> {
    update_settings(app, core, Settings::default())
}

#[tauri::command]
pub fn export_settings(core: State<'_, Core>, path: PathBuf) -> CoreResult<()> {
    settings::save_to(&core.shared.settings.read(), &path)
}

#[tauri::command]
pub fn import_settings(app: AppHandle, core: State<'_, Core>, path: PathBuf) -> CoreResult<Settings> {
    let s = settings::load_from(&path)?;
    update_settings(app, core, s)
}

#[tauri::command]
pub fn get_status(core: State<'_, Core>) -> Status {
    core.status()
}

#[tauri::command]
pub fn get_encoders(core: State<'_, Core>) -> Vec<EncoderInfo> {
    core.shared.encoders.read().clone()
}

#[tauri::command]
pub fn list_monitors() -> Vec<MonitorInfo> {
    capture::list_monitors()
}

#[tauri::command]
pub async fn list_windows() -> Vec<WindowInfo> {
    capture::list_windows()
}

#[tauri::command]
pub async fn list_audio_devices() -> CoreResult<AudioDevices> {
    wasapi::list_devices()
}

#[tauri::command]
pub fn save_clip(core: State<'_, Core>) {
    core.send(Cmd::SaveClip(Instant::now()));
}

#[tauri::command]
pub fn toggle_recording(core: State<'_, Core>) {
    core.send(Cmd::ToggleRecording);
}

#[tauri::command]
pub fn set_buffer_enabled(core: State<'_, Core>, enabled: bool) {
    core.send(Cmd::SetBuffer(enabled));
}

#[tauri::command]
pub fn toggle_mic_mute(core: State<'_, Core>) {
    core.send(Cmd::ToggleMicMute);
}

#[tauri::command]
pub fn validate_hotkeys(hotkeys: HotkeySettings) -> Vec<HotkeyConflict> {
    hotkeys::validate(&hotkeys)
}

/// Konflikte der aktuell registrierten Hotkeys (inkl. „von anderer App belegt“).
#[tauri::command]
pub fn hotkey_conflicts(app: AppHandle, core: State<'_, Core>) -> Vec<HotkeyConflict> {
    let h = core.shared.settings.read().hotkeys.clone();
    hotkeys::register_all(&app, &h)
}

/// Pausiert globale Hotkeys, während der Nutzer im Keycap-Feld eine Taste aufnimmt.
#[tauri::command]
pub fn suspend_hotkeys(app: AppHandle, core: State<'_, Core>, suspended: bool) {
    use tauri_plugin_global_shortcut::GlobalShortcutExt;
    if suspended {
        let _ = app.global_shortcut().unregister_all();
    } else {
        let h = core.shared.settings.read().hotkeys.clone();
        let conflicts = hotkeys::register_all(&app, &h);
        let _ = app.emit(events::HOTKEY_CONFLICTS, &conflicts);
    }
}

#[tauri::command]
pub async fn list_clips(core: State<'_, Core>) -> CoreResult<Vec<ClipInfo>> {
    let root = core.output_dir();
    Ok(core.shared.library.list(&root))
}

/// Erzeugt (falls nötig) das Thumbnail auf dem MuxWorker und meldet den Pfad zurück.
#[tauri::command]
pub async fn get_thumbnail(core: State<'_, Core>, path: String) -> CoreResult<String> {
    let (tx, rx) = tauri::async_runtime::channel::<CoreResult<PathBuf>>(1);
    let p = PathBuf::from(path);
    let queued = core.shared.mux.submit(move || {
        let _ = tx.blocking_send(Library::ensure_thumbnail(&p));
    });
    if !queued {
        return Err(CoreError::Other("Warteschlange voll".into()));
    }
    let mut rx = rx;
    let r = rx.recv().await.ok_or(CoreError::NotRunning)??;
    Ok(r.to_string_lossy().into_owned())
}

#[tauri::command]
pub async fn trim_clip(
    core: State<'_, Core>,
    path: String,
    start_secs: f64,
    end_secs: f64,
    replace: bool,
) -> CoreResult<ClipInfo> {
    let root = core.output_dir();
    let src = library::ensure_inside(&root, Path::new(&path))?;
    let stem = src.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let dir = src.parent().map(Path::to_path_buf).unwrap_or_else(|| root.clone());
    let mut out = dir.join(format!("{stem}_trim.mp4"));
    let mut n = 2;
    while out.exists() {
        out = dir.join(format!("{stem}_trim{n}.mp4"));
        n += 1;
    }
    let (tx, mut rx) = tauri::async_runtime::channel::<CoreResult<()>>(1);
    let (s2, o2) = (src.clone(), out.clone());
    core.shared.mux.submit(move || {
        let r = media::trim(&s2, &o2, start_secs, end_secs).map(|_| ());
        let _ = tx.blocking_send(r);
    });
    rx.recv().await.ok_or(CoreError::NotRunning)??;
    let final_path = if replace {
        library::move_to_trash(&src)?;
        std::fs::rename(&out, &src)?;
        core.shared.library.forget(&src);
        src
    } else {
        out
    };
    let _ = Library::ensure_thumbnail(&final_path);
    core.shared.library.info(&root, &final_path)
}

#[tauri::command]
pub fn delete_clip(core: State<'_, Core>, path: String) -> CoreResult<()> {
    let root = core.output_dir();
    let p = library::ensure_inside(&root, Path::new(&path))?;
    library::move_to_trash(&p)?;
    core.shared.library.forget(&p);
    Ok(())
}

#[tauri::command]
pub fn reveal_clip(app: AppHandle, path: String) -> CoreResult<()> {
    use tauri_plugin_opener::OpenerExt;
    app.opener().reveal_item_in_dir(&path).map_err(|e| CoreError::Other(e.to_string()))
}

#[tauri::command]
pub fn open_clips_folder(app: AppHandle, core: State<'_, Core>) -> CoreResult<()> {
    use tauri_plugin_opener::OpenerExt;
    let dir = core.output_dir();
    std::fs::create_dir_all(&dir)?;
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(|e| CoreError::Other(e.to_string()))
}

#[tauri::command]
pub fn default_output_dir(core: State<'_, Core>) -> String {
    core.output_dir().to_string_lossy().into_owned()
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    crate::quit(&app);
}

/// Erlaubt dem Asset-Protokoll den Zugriff auf den Clip-Ordner (Player, Thumbnails).
pub fn allow_clip_dir(app: &AppHandle, dir: &Path) {
    let scope = app.asset_protocol_scope();
    let _ = scope.allow_directory(dir, true);
    if let Ok(t) = crate::paths::thumbs_dir() {
        let _ = scope.allow_directory(t, true);
    }
}
