//! Zeigt die nativen Toasts nacheinander an (manuelle Sichtprüfung des Overlays).
use std::time::Duration;

use clipper_lib::overlay::{Overlay, ToastKind};
use clipper_lib::settings::Settings;

fn main() {
    tracing_subscriber::fmt().init();
    let o = Overlay::start();
    let s = Settings::default();
    std::thread::sleep(Duration::from_millis(200));
    for (kind, detail) in [
        (ToastKind::ClipSaved, "30 s · Valorant"),
        (ToastKind::RecordingStarted, ""),
        (ToastKind::Error, "Kein Encoder verfügbar"),
    ] {
        o.show(kind, detail.into(), &s);
        std::thread::sleep(Duration::from_millis(3200));
    }
}
