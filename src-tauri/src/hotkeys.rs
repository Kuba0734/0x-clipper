//! Globale Hotkeys über `tauri-plugin-global-shortcut` (intern `RegisterHotKey`, kein Polling).
//! Konflikte: doppelte Belegung, ungültiges Format, von Windows reserviert, von anderer App belegt.

use std::collections::HashMap;
use std::time::Instant;

use parking_lot::Mutex;
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutEvent, ShortcutState};
use ts_rs::TS;

use crate::controller::{Cmd, Core};
use crate::settings::HotkeySettings;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum HotkeyAction {
    SaveClip,
    ToggleRecording,
    ToggleBuffer,
    ToggleMicMute,
    PushToTalk,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum ConflictKind {
    Invalid,
    Duplicate,
    Reserved,
    InUse,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct HotkeyConflict {
    pub action: HotkeyAction,
    pub accelerator: String,
    pub kind: ConflictKind,
    /// Bei `Duplicate`: die andere Aktion.
    pub other: Option<HotkeyAction>,
}

pub fn entries(h: &HotkeySettings) -> [(HotkeyAction, &str); 5] {
    [
        (HotkeyAction::SaveClip, h.save_clip.as_str()),
        (HotkeyAction::ToggleRecording, h.toggle_recording.as_str()),
        (HotkeyAction::ToggleBuffer, h.toggle_buffer.as_str()),
        (HotkeyAction::ToggleMicMute, h.toggle_mic_mute.as_str()),
        (HotkeyAction::PushToTalk, h.push_to_talk.as_str()),
    ]
}

/// Von Windows oder der Shell belegt; Registrierung würde scheitern oder stören.
const RESERVED: &[&str] = &["alt+tab", "alt+f4", "ctrl+alt+delete", "ctrl+shift+escape", "alt+escape", "ctrl+escape"];

fn normalize(accel: &str) -> String {
    let mut parts: Vec<String> = accel.split('+').map(|p| p.trim().to_ascii_lowercase()).collect();
    let key = parts.pop().unwrap_or_default();
    let rank = |m: &str| match m {
        "ctrl" | "control" | "commandorcontrol" => 0,
        "alt" | "option" => 1,
        "shift" => 2,
        _ => 3,
    };
    parts.iter_mut().for_each(|m| {
        if m == "control" || m == "commandorcontrol" {
            *m = "ctrl".into();
        }
    });
    parts.sort_by_key(|m| rank(m));
    parts.dedup();
    parts.push(key);
    parts.join("+")
}

/// Statische Prüfung ohne Registrierung (für die UI beim Tippen).
pub fn validate(h: &HotkeySettings) -> Vec<HotkeyConflict> {
    let mut out = Vec::new();
    let mut seen: HashMap<String, HotkeyAction> = HashMap::new();
    for (action, accel) in entries(h) {
        if accel.trim().is_empty() {
            continue;
        }
        let norm = normalize(accel);
        let conflict = |kind, other| HotkeyConflict { action, accelerator: accel.to_string(), kind, other };
        if accel.parse::<Shortcut>().is_err() {
            out.push(conflict(ConflictKind::Invalid, None));
            continue;
        }
        if RESERVED.contains(&norm.as_str()) || norm.contains("super") || norm.contains("meta") {
            out.push(conflict(ConflictKind::Reserved, None));
            continue;
        }
        if let Some(&other) = seen.get(&norm) {
            out.push(conflict(ConflictKind::Duplicate, Some(other)));
            continue;
        }
        seen.insert(norm, action);
    }
    out
}

/// Zuordnung registrierter Shortcuts zu Aktionen.
#[derive(Default)]
pub struct HotkeyMap(pub Mutex<HashMap<u32, HotkeyAction>>);

/// Meldet alle Hotkeys neu an. Rückgabe: alle Konflikte (statisch + „von anderer App belegt“).
pub fn register_all<R: Runtime>(app: &AppHandle<R>, h: &HotkeySettings) -> Vec<HotkeyConflict> {
    let gs = app.global_shortcut();
    let _ = gs.unregister_all();
    let mut conflicts = validate(h);
    let mut map = HashMap::new();
    for (action, accel) in entries(h) {
        if accel.trim().is_empty() || conflicts.iter().any(|c| c.action == action) {
            continue;
        }
        let Ok(sc) = accel.parse::<Shortcut>() else { continue };
        match gs.register(sc) {
            Ok(()) => {
                map.insert(sc.id(), action);
            }
            Err(e) => {
                tracing::warn!("Hotkey {accel} nicht registrierbar: {e}");
                conflicts.push(HotkeyConflict {
                    action,
                    accelerator: accel.to_string(),
                    kind: ConflictKind::InUse,
                    other: None,
                });
            }
        }
    }
    if let Some(state) = app.try_state::<HotkeyMap>() {
        *state.0.lock() = map;
    }
    conflicts
}

/// Handler des Plugins: übersetzt Tastendrücke in Controller-Befehle.
pub fn on_shortcut<R: Runtime>(app: &AppHandle<R>, sc: &Shortcut, ev: ShortcutEvent) {
    let action = app.try_state::<HotkeyMap>().and_then(|m| m.0.lock().get(&sc.id()).copied());
    let Some(action) = action else { return };
    let Some(core) = app.try_state::<Core>() else { return };
    let pressed = ev.state() == ShortcutState::Pressed;
    match action {
        HotkeyAction::PushToTalk => {
            core.shared.controls.ptt_down.store(pressed, std::sync::atomic::Ordering::Relaxed);
        }
        _ if !pressed => {}
        HotkeyAction::SaveClip => core.send(Cmd::SaveClip(Instant::now())),
        HotkeyAction::ToggleRecording => core.send(Cmd::ToggleRecording),
        HotkeyAction::ToggleBuffer => core.send(Cmd::ToggleBuffer),
        HotkeyAction::ToggleMicMute => core.send(Cmd::ToggleMicMute),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn hk(a: &str, b: &str) -> HotkeySettings {
        HotkeySettings {
            save_clip: a.into(),
            toggle_recording: b.into(),
            toggle_buffer: String::new(),
            toggle_mic_mute: String::new(),
            push_to_talk: String::new(),
        }
    }

    #[test]
    fn defaults_have_no_conflicts() {
        assert!(validate(&HotkeySettings::default()).is_empty());
    }

    #[test]
    fn detects_duplicates_regardless_of_order_and_case() {
        let c = validate(&hk("Alt+Shift+F10", "shift+ALT+F10"));
        assert_eq!(c.len(), 1);
        assert_eq!(c[0].kind, ConflictKind::Duplicate);
        assert_eq!(c[0].other, Some(HotkeyAction::SaveClip));
    }

    #[test]
    fn detects_reserved_and_invalid() {
        let c = validate(&hk("Alt+F4", "Alt+Nope"));
        assert_eq!(c[0].kind, ConflictKind::Reserved);
        assert_eq!(c[1].kind, ConflictKind::Invalid);
    }
}
