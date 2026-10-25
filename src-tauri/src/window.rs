//! Hauptfenster-Lebenszyklus. Schließen zerstört das WebView vollständig (kein
//! Renderer-Prozess im Tray); „Öffnen“ legt es neu an. Status-Events laufen nur,
//! solange das Fenster existiert (max. 2× pro Sekunde).

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use tauri::{AppHandle, Emitter, Manager, WebviewUrl, WebviewWindowBuilder, WindowEvent};

use crate::controller::Core;
use crate::events;

pub const MAIN: &str = "main";

pub fn open_main(app: &AppHandle) {
    if let Some(w) = app.get_webview_window(MAIN) {
        let _ = w.unminimize();
        let _ = w.show();
        let _ = w.set_focus();
        return;
    }
    let built = WebviewWindowBuilder::new(app, MAIN, WebviewUrl::App("index.html".into()))
        .title("0x-clipper")
        .inner_size(1180.0, 760.0)
        .min_inner_size(960.0, 620.0)
        .decorations(false)
        .resizable(true)
        .center()
        .background_color(tauri::window::Color(0x0B, 0x0B, 0x0F, 0xFF))
        .build();
    match built {
        Ok(w) => {
            let stop = Arc::new(AtomicBool::new(false));
            start_status_emitter(app.clone(), stop.clone());
            w.on_window_event(move |ev| {
                if let WindowEvent::Destroyed = ev {
                    stop.store(true, Ordering::Relaxed);
                }
            });
        }
        Err(e) => tracing::error!("Fenster: {e}"),
    }
}

fn start_status_emitter(app: AppHandle, stop: Arc<AtomicBool>) {
    let r = crate::util::spawn("status-emitter", crate::util::Priority::BelowNormal, move || {
        let Some(core) = app.try_state::<Core>().map(|c| c.inner().clone()) else { return };
        let stats = core.shared.stats.clone();
        let mut last = Instant::now();
        let mut last_bytes = stats.video_bytes.load(Ordering::Relaxed);
        let mut last_frames = stats.encoded_frames.load(Ordering::Relaxed);
        while !stop.load(Ordering::Relaxed) {
            std::thread::sleep(Duration::from_millis(500));
            let dt = last.elapsed().as_secs_f64().max(0.001);
            last = Instant::now();
            let bytes = stats.video_bytes.load(Ordering::Relaxed);
            let frames = stats.encoded_frames.load(Ordering::Relaxed);
            let mut status = core.status();
            status.bitrate_kbps = ((bytes - last_bytes) as f64 * 8.0 / 1000.0 / dt) as u32;
            status.fps = ((frames - last_frames) as f64 / dt) as f32;
            last_bytes = bytes;
            last_frames = frames;
            let _ = app.emit_to(MAIN, events::STATUS, status);
        }
    });
    if let Err(e) = r {
        tracing::error!("Status-Emitter: {e}");
    }
}
