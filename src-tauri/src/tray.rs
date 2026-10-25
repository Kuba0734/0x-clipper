//! Tray-Icon mit Menü: Öffnen, Clip speichern, Aufnahme, Buffer an/aus, Beenden.

use std::time::Instant;

use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager};

use crate::controller::{tr, Cmd, Core};
use crate::error::CoreResult;
use crate::settings::Settings;

pub struct TrayItems {
    tray: TrayIcon,
    open: MenuItem<tauri::Wry>,
    clip: MenuItem<tauri::Wry>,
    record: MenuItem<tauri::Wry>,
    buffer: CheckMenuItem<tauri::Wry>,
    quit: MenuItem<tauri::Wry>,
}

const ICON: &[u8] = include_bytes!("../icons/tray.png");
const ICON_REC: &[u8] = include_bytes!("../icons/tray-rec.png");

pub fn build(app: &AppHandle, s: &Settings) -> CoreResult<()> {
    let open = MenuItem::with_id(app, "open", tr(s, "Öffnen", "Open"), true, None::<&str>)?;
    let clip = MenuItem::with_id(app, "clip", tr(s, "Clip speichern", "Save clip"), true, None::<&str>)?;
    let record = MenuItem::with_id(app, "record", tr(s, "Aufnahme starten", "Start recording"), true, None::<&str>)?;
    let buffer = CheckMenuItem::with_id(
        app,
        "buffer",
        tr(s, "Replay-Buffer", "Replay buffer"),
        true,
        s.replay.enabled,
        None::<&str>,
    )?;
    let quit = MenuItem::with_id(app, "quit", tr(s, "Beenden", "Quit"), true, None::<&str>)?;
    let sep = PredefinedMenuItem::separator(app)?;
    let sep2 = PredefinedMenuItem::separator(app)?;
    let menu = Menu::with_items(app, &[&open, &sep, &clip, &record, &buffer, &sep2, &quit])?;
    let tray = TrayIconBuilder::with_id("main")
        .icon(Image::from_bytes(ICON)?)
        .tooltip("0x-clipper")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, ev| {
            let core = app.try_state::<Core>();
            match ev.id().as_ref() {
                "open" => crate::window::open_main(app),
                "clip" => {
                    if let Some(c) = core {
                        c.send(Cmd::SaveClip(Instant::now()));
                    }
                }
                "record" => {
                    if let Some(c) = core {
                        c.send(Cmd::ToggleRecording);
                    }
                }
                "buffer" => {
                    if let Some(c) = core {
                        c.send(Cmd::ToggleBuffer);
                    }
                }
                "quit" => crate::quit(app),
                _ => {}
            }
        })
        .on_tray_icon_event(|tray, ev| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = ev {
                crate::window::open_main(tray.app_handle());
            }
        })
        .build(app)?;
    app.manage(TrayItems { tray, open, clip, record, buffer, quit });
    refresh(app);
    Ok(())
}

/// Aktualisiert Texte, Häkchen, Tooltip und Icon (rot während Aufnahme).
pub fn refresh(app: &AppHandle) {
    let (Some(items), Some(core)) = (app.try_state::<TrayItems>(), app.try_state::<Core>()) else { return };
    let s = core.shared.settings.read().clone();
    let st = core.status();
    let _ = items.open.set_text(tr(&s, "Öffnen", "Open"));
    let _ = items.clip.set_text(tr(&s, "Clip speichern", "Save clip"));
    let _ = items.quit.set_text(tr(&s, "Beenden", "Quit"));
    let _ = items.record.set_text(if st.recording {
        tr(&s, "Aufnahme stoppen", "Stop recording")
    } else {
        tr(&s, "Aufnahme starten", "Start recording")
    });
    let _ = items.buffer.set_text(tr(&s, "Replay-Buffer", "Replay buffer"));
    let _ = items.buffer.set_checked(s.replay.enabled);
    let _ = items.clip.set_enabled(s.replay.enabled);
    let tip = if st.recording {
        tr(&s, "0x-clipper – Aufnahme läuft", "0x-clipper – recording")
    } else if s.replay.enabled {
        format!("0x-clipper – {} {} s", tr(&s, "Buffer", "Buffer"), s.replay.duration_secs)
    } else {
        tr(&s, "0x-clipper – Buffer aus", "0x-clipper – buffer off")
    };
    let _ = items.tray.set_tooltip(Some(tip));
    if let Ok(img) = Image::from_bytes(if st.recording { ICON_REC } else { ICON }) {
        let _ = items.tray.set_icon(Some(img));
    }
}
