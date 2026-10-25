//! 0x-clipper Core.

pub mod audio;
pub mod buffer;
pub mod capture;
pub mod commands;
pub mod controller;
pub mod encode;
pub mod error;
pub mod events;
pub mod hotkeys;
pub mod library;
pub mod mux;
pub mod overlay;
pub mod paths;
pub mod pipeline;
pub mod router;
pub mod settings;
pub mod sound;
pub mod stats;
pub mod sysinfo;
pub mod tray;
pub mod util;
pub mod window;

use std::sync::OnceLock;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Manager, RunEvent};

/// Name des Run-Eintrags in der Registry (auch vom Installer benutzt).
pub const AUTOSTART_NAME: &str = "0x-clipper";

static LOG_GUARD: OnceLock<tracing_appender::non_blocking::WorkerGuard> = OnceLock::new();
static STARTED: OnceLock<Instant> = OnceLock::new();

/// Startzeitpunkt des Prozesses (für die Messung „Kaltstart bis Buffering“).
pub fn started_at() -> Instant {
    *STARTED.get_or_init(Instant::now)
}

fn init_logging() {
    use tracing_subscriber::{fmt, prelude::*, EnvFilter};
    let filter = EnvFilter::try_from_env("CLIPPER_LOG").unwrap_or_else(|_| EnvFilter::new("info"));
    let file_layer = paths::logs_dir().ok().and_then(|dir| {
        std::fs::create_dir_all(&dir).ok()?;
        let appender = tracing_appender::rolling::Builder::new()
            .rotation(tracing_appender::rolling::Rotation::DAILY)
            .filename_prefix("0x-clipper")
            .filename_suffix("log")
            .max_log_files(7)
            .build(dir)
            .ok()?;
        let (nb, guard) = tracing_appender::non_blocking(appender);
        let _ = LOG_GUARD.set(guard);
        Some(fmt::layer().with_writer(nb).with_ansi(false))
    });
    let stderr = cfg!(debug_assertions).then(fmt::layer);
    let _ = tracing_subscriber::registry().with(filter).with(file_layer).with(stderr).try_init();
}

/// Beendet die App sauber: Aufnahme finalisieren, Pipeline stoppen, dann Prozess beenden.
pub fn quit(app: &AppHandle) {
    if let Some(core) = app.try_state::<controller::Core>() {
        let (tx, rx) = crossbeam_channel::bounded(1);
        core.send(controller::Cmd::Shutdown(tx));
        let _ = rx.recv_timeout(Duration::from_secs(5));
    }
    app.exit(0);
}

pub fn run() {
    started_at();
    init_logging();
    encode::ffmpeg::init_logging();
    let minimized_arg = std::env::args().any(|a| a == "--minimized");

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            window::open_main(app);
        }))
        .plugin(
            tauri_plugin_global_shortcut::Builder::new()
                .with_handler(hotkeys::on_shortcut)
                .build(),
        )
        .plugin(
            tauri_plugin_autostart::Builder::new()
                .app_name(AUTOSTART_NAME)
                .args(["--minimized"])
                .build(),
        )
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(hotkeys::HotkeyMap::default())
        .invoke_handler(tauri::generate_handler![
            commands::get_settings,
            commands::update_settings,
            commands::reset_settings,
            commands::export_settings,
            commands::import_settings,
            commands::get_status,
            commands::get_encoders,
            commands::list_monitors,
            commands::list_windows,
            commands::list_audio_devices,
            commands::save_clip,
            commands::toggle_recording,
            commands::set_buffer_enabled,
            commands::toggle_mic_mute,
            commands::validate_hotkeys,
            commands::hotkey_conflicts,
            commands::suspend_hotkeys,
            commands::list_clips,
            commands::get_thumbnail,
            commands::trim_clip,
            commands::delete_clip,
            commands::reveal_clip,
            commands::open_clips_folder,
            commands::default_output_dir,
            commands::quit_app,
        ])
        .setup(move |app| {
            let handle = app.handle().clone();
            let first_run = settings::settings_path().map(|p| !p.exists()).unwrap_or(false);
            let mut s = settings::load();
            if first_run {
                // Der Installer kann Autostart bereits gesetzt haben: übernehmen.
                use tauri_plugin_autostart::ManagerExt;
                s.general.autostart = handle.autolaunch().is_enabled().unwrap_or(false);
                let _ = settings::save(&s);
            }
            commands::allow_clip_dir(&handle, &paths::output_dir(&s));
            paths::publish_clips_dir_for_uninstaller(&paths::output_dir(&s));
            let core = controller::start(handle.clone(), s.clone())?;
            app.manage(core);
            let conflicts = hotkeys::register_all(&handle, &s.hotkeys);
            if !conflicts.is_empty() {
                tracing::warn!("Hotkey-Konflikte: {conflicts:?}");
            }
            tray::build(&handle, &s)?;
            // Autostart-Eintrag mit den Einstellungen abgleichen.
            {
                use tauri_plugin_autostart::ManagerExt;
                let al = handle.autolaunch();
                if al.is_enabled().unwrap_or(false) != s.general.autostart {
                    let _ = if s.general.autostart { al.enable() } else { al.disable() };
                }
            }
            if !(minimized_arg && s.general.start_minimized) {
                window::open_main(&handle);
            }
            tracing::info!("Setup fertig nach {} ms", started_at().elapsed().as_millis());
            Ok(())
        })
        .build(tauri::generate_context!());

    let app = match app {
        Ok(a) => a,
        Err(e) => {
            tracing::error!("Tauri-Start fehlgeschlagen: {e}");
            return;
        }
    };
    app.run(|_app, ev| {
        // Fenster schließen beendet nicht die App: Aufnahme läuft im Tray weiter.
        if let RunEvent::ExitRequested { code: None, api, .. } = ev {
            api.prevent_exit();
        }
    });
}
