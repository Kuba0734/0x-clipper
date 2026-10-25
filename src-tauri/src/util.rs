//! Kleine Win32-Helfer: Uhr, Thread-Prioritäten, Prozessinfos, COM.

use std::sync::OnceLock;

use windows::core::PWSTR;
use windows::Win32::Foundation::{CloseHandle, HWND};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_MULTITHREADED};
use windows::Win32::System::Performance::{QueryPerformanceCounter, QueryPerformanceFrequency};
use windows::Win32::System::Threading::{
    GetCurrentThread, OpenProcess, QueryFullProcessImageNameW, SetThreadPriority,
    PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION, THREAD_PRIORITY,
    THREAD_PRIORITY_ABOVE_NORMAL, THREAD_PRIORITY_BELOW_NORMAL, THREAD_PRIORITY_NORMAL,
};
use windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId;

fn qpc_freq() -> i64 {
    static FREQ: OnceLock<i64> = OnceLock::new();
    *FREQ.get_or_init(|| {
        let mut f = 0i64;
        // SAFETY: f ist ein gültiger Ausgabezeiger.
        unsafe {
            let _ = QueryPerformanceFrequency(&mut f);
        }
        f.max(1)
    })
}

/// Gemeinsame Uhr aller Streams: QPC in Mikrosekunden.
/// WGC (`SystemRelativeTime`) und WASAPI (`u64QPCPosition`) liefern dieselbe Basis in 100 ns.
pub fn now_us() -> i64 {
    let mut c = 0i64;
    // SAFETY: c ist ein gültiger Ausgabezeiger.
    unsafe {
        let _ = QueryPerformanceCounter(&mut c);
    }
    qpc_to_us(c)
}

pub fn qpc_to_us(ticks: i64) -> i64 {
    let f = qpc_freq();
    (ticks / f) * 1_000_000 + (ticks % f) * 1_000_000 / f
}

#[derive(Debug, Clone, Copy)]
pub enum Priority {
    AboveNormal,
    Normal,
    BelowNormal,
}

pub fn set_thread_priority(p: Priority) {
    let prio: THREAD_PRIORITY = match p {
        Priority::AboveNormal => THREAD_PRIORITY_ABOVE_NORMAL,
        Priority::Normal => THREAD_PRIORITY_NORMAL,
        Priority::BelowNormal => THREAD_PRIORITY_BELOW_NORMAL,
    };
    // SAFETY: Pseudo-Handle des aktuellen Threads.
    unsafe {
        let _ = SetThreadPriority(GetCurrentThread(), prio);
    }
}

/// Startet einen benannten Thread mit Priorität.
pub fn spawn<F, T>(name: &str, prio: Priority, f: F) -> std::io::Result<std::thread::JoinHandle<T>>
where
    F: FnOnce() -> T + Send + 'static,
    T: Send + 'static,
{
    std::thread::Builder::new().name(name.into()).spawn(move || {
        set_thread_priority(prio);
        f()
    })
}

/// COM für den aktuellen Thread (MTA). Mehrfachaufrufe sind harmlos.
pub fn com_init() {
    // SAFETY: Initialisiert nur den aufrufenden Thread.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
    }
}

pub fn wide_to_string(w: &[u16]) -> String {
    let len = w.iter().position(|&c| c == 0).unwrap_or(w.len());
    String::from_utf16_lossy(&w[..len])
}

pub fn window_pid(h: HWND) -> u32 {
    let mut pid = 0u32;
    // SAFETY: pid ist ein gültiger Ausgabezeiger.
    unsafe { GetWindowThreadProcessId(h, Some(&mut pid)) };
    pid
}

/// Dateiname der EXE eines Fensters, z. B. `game.exe`.
pub fn window_exe(h: HWND) -> Option<String> {
    process_exe_path(window_pid(h)).and_then(|p| {
        std::path::Path::new(&p).file_name().map(|n| n.to_string_lossy().into_owned())
    })
}

pub fn process_exe_path(pid: u32) -> Option<String> {
    if pid == 0 {
        return None;
    }
    // SAFETY: Handle wird geprüft und wieder geschlossen; Puffer ist ausreichend groß.
    unsafe {
        let h = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid).ok()?;
        let mut buf = [0u16; 1024];
        let mut len = buf.len() as u32;
        let r = QueryFullProcessImageNameW(h, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len);
        let _ = CloseHandle(h);
        r.ok()?;
        Some(String::from_utf16_lossy(&buf[..len as usize]))
    }
}

pub fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}
