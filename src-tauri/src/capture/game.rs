//! Erkennung des Vollbild-Spiels im Vordergrund per `SetWinEventHook(EVENT_SYSTEM_FOREGROUND)`.
//! Out-of-context: Es wird nichts in fremde Prozesse injiziert (Anti-Cheat-sicher).

use std::sync::{Arc, OnceLock};

use crossbeam_channel::Sender;
use parking_lot::RwLock;
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::Graphics::Gdi::{MonitorFromWindow, MONITOR_DEFAULTTONEAREST};
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetClassNameW, GetForegroundWindow, GetMessageW, GetShellWindow,
    GetWindowRect, TranslateMessage, EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_MINIMIZEEND, MSG,
    WINEVENT_OUTOFCONTEXT, WINEVENT_SKIPOWNPROCESS,
};

use super::{monitor_info, window_title};
use crate::util;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameInfo {
    /// Anzeigename für Dateinamen und Unterordner.
    pub name: String,
    pub exe: String,
    /// Monitor, auf dem das Spiel läuft (GDI-Gerätename).
    pub monitor: String,
}

pub type SharedGame = Arc<RwLock<Option<GameInfo>>>;

struct HookState {
    game: SharedGame,
    notify: Sender<()>,
}

static STATE: OnceLock<HookState> = OnceLock::new();

/// Prozesse, die nie als Spiel gelten.
const IGNORED_EXE: &[&str] = &[
    "explorer.exe",
    "0x-clipper.exe",
    "searchhost.exe",
    "startmenuexperiencehost.exe",
    "shellexperiencehost.exe",
    "lockapp.exe",
    "applicationframehost.exe",
    "textinputhost.exe",
];

/// Startet den Hook-Thread. `notify` bekommt ein Signal, wenn sich das Spiel ändert.
pub fn start(game: SharedGame, notify: Sender<()>) {
    if STATE.set(HookState { game, notify }).is_err() {
        return;
    }
    let spawned = util::spawn("game-detect", util::Priority::BelowNormal, || {
        // SAFETY: Callback ist eine statische Funktion; Hook wird auf diesem Thread entfernt.
        unsafe {
            let hook = SetWinEventHook(
                EVENT_SYSTEM_FOREGROUND,
                EVENT_SYSTEM_MINIMIZEEND,
                None,
                Some(on_event),
                0,
                0,
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            );
            evaluate(GetForegroundWindow());
            let mut msg = MSG::default();
            // Blockiert, bis Nachrichten (WinEvents) eintreffen.
            while GetMessageW(&mut msg, None, 0, 0).as_bool() {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            let _ = UnhookWinEvent(hook);
        }
    });
    if let Err(e) = spawned {
        tracing::error!("Spielerkennung nicht gestartet: {e}");
    }
}

unsafe extern "system" fn on_event(
    _: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    _: i32,
    _: i32,
    _: u32,
    _: u32,
) {
    if event == EVENT_SYSTEM_FOREGROUND || event == EVENT_SYSTEM_MINIMIZEEND {
        evaluate(hwnd);
    }
}

fn evaluate(hwnd: HWND) {
    let Some(state) = STATE.get() else { return };
    let detected = detect(hwnd);
    let mut cur = state.game.write();
    // Nicht-Spiel-Fenster (z. B. Alt+Tab auf Discord) behalten das zuletzt erkannte Spiel,
    // solange es kein neues Spiel gibt.
    if let Some(g) = detected {
        if cur.as_ref() != Some(&g) {
            tracing::info!("Spiel erkannt: {} ({}) auf {}", g.name, g.exe, g.monitor);
            *cur = Some(g);
            drop(cur);
            let _ = state.notify.try_send(());
        }
    }
}

/// Ein Fenster gilt als Spiel, wenn es seinen Monitor vollständig bedeckt
/// und nicht zur Shell gehört.
fn detect(hwnd: HWND) -> Option<GameInfo> {
    if hwnd.is_invalid() {
        return None;
    }
    // SAFETY: Reine Abfragen.
    unsafe {
        if hwnd == GetShellWindow() {
            return None;
        }
        let mut class = [0u16; 64];
        let n = GetClassNameW(hwnd, &mut class);
        let class = String::from_utf16_lossy(&class[..n.max(0) as usize]);
        if matches!(class.as_str(), "Progman" | "WorkerW" | "Shell_TrayWnd") {
            return None;
        }
        let exe = util::window_exe(hwnd)?;
        if IGNORED_EXE.iter().any(|i| exe.eq_ignore_ascii_case(i)) {
            return None;
        }
        let mon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        let info = monitor_info(mon)?;
        let mut r = RECT::default();
        GetWindowRect(hwnd, &mut r).ok()?;
        let covers = r.left <= info.x
            && r.top <= info.y
            && r.right >= info.x + info.width as i32
            && r.bottom >= info.y + info.height as i32;
        if !covers {
            return None;
        }
        Some(GameInfo { name: display_name(&window_title(hwnd), &exe), exe, monitor: info.device })
    }
}

/// Kurzer Fenstertitel ist meist der Spielname; sonst EXE ohne Endung und Shipping-Suffixe.
pub fn display_name(title: &str, exe: &str) -> String {
    let title = title.trim();
    if !title.is_empty() && title.chars().count() <= 40 {
        return title.to_string();
    }
    let stem = exe.rsplit_once('.').map(|(s, _)| s).unwrap_or(exe);
    let stem = stem.split("-Win64").next().unwrap_or(stem);
    stem.trim_end_matches("-Shipping").to_string()
}

#[cfg(test)]
mod tests {
    use super::display_name;

    #[test]
    fn names() {
        assert_eq!(display_name("VALORANT  ", "VALORANT-Win64-Shipping.exe"), "VALORANT");
        assert_eq!(display_name("", "VALORANT-Win64-Shipping.exe"), "VALORANT");
        assert_eq!(display_name("", "cs2.exe"), "cs2");
        let long = "x".repeat(60);
        assert_eq!(display_name(&long, "Game-Shipping.exe"), "Game");
    }
}
