//! Bildschirmaufnahme: WGC (bevorzugt) und DXGI Desktop Duplication (Fallback).
//!
//! Der Capture-Thread blockiert auf neuen Frames (Event von WGC bzw. AcquireNextFrame),
//! konvertiert sie auf der GPU in einen Frame des Encoder-Pools und übergibt ihn über einen
//! bounded Channel. Ist der Channel voll oder kein Frame frei, wird verworfen und gezählt.

pub mod d3d;
pub mod dxgi;
pub mod game;
pub mod wgc;

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;

use crossbeam_channel::{Receiver, Sender, TrySendError};
use serde::Serialize;
use ts_rs::TS;
use windows::core::BOOL;
use windows::Win32::Foundation::{HWND, LPARAM, RECT};
use windows::Win32::Graphics::Direct3D11::ID3D11Texture2D;
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
use windows::Win32::Graphics::Gdi::{
    EnumDisplayMonitors, GetMonitorInfoW, MonitorFromPoint, HDC, HMONITOR, MONITORINFO,
    MONITORINFOEXW, MONITOR_DEFAULTTOPRIMARY,
};
use windows::Win32::UI::WindowsAndMessaging::{
    EnumWindows, GetWindowLongW, GetWindowTextLengthW, GetWindowTextW, IsWindowVisible,
    GWL_EXSTYLE, MONITORINFOF_PRIMARY, WS_EX_TOOLWINDOW,
};

use crate::encode::ffi::OwnedFrame;
use crate::encode::ffmpeg::FramesRef;
use crate::error::{CoreError, CoreResult};
use crate::stats::Stats;
use crate::util;
use d3d::{D3dDevice, VideoConverter};

/// Ein von der Quelle gelieferter Frame (Textur gehört bis `release` der Quelle).
pub struct SourceFrame {
    pub texture: ID3D11Texture2D,
    pub pts_us: i64,
}

pub trait FrameSource: Send {
    /// Wartet höchstens `timeout` auf einen neuen Frame. `None` = Timeout.
    fn next_frame(&mut self, timeout: Duration) -> CoreResult<Option<SourceFrame>>;
    /// Gibt den zuletzt gelieferten Frame an die Quelle zurück.
    fn release(&mut self);
    fn size(&self) -> (u32, u32);
}

/// Encoder-fertiger Frame im d3d11-Pool.
pub struct CapturedFrame {
    pub frame: OwnedFrame,
    pub pts_us: i64,
}

pub struct CaptureLoop {
    pub fps: u32,
    pub out_size: (u32, u32),
    pub frames: FramesRef,
    pub tx: Sender<CapturedFrame>,
    pub free_rx: Receiver<OwnedFrame>,
    pub stop: Arc<AtomicBool>,
    pub stats: Arc<Stats>,
}

/// Ohne neue Bilder (statischer Inhalt) wird das letzte Bild so oft wiederholt,
/// damit Keyframes und Zeitachse weiterlaufen.
const REPEAT_AFTER: Duration = Duration::from_millis(100);

impl CaptureLoop {
    pub fn run(self, dev: Arc<D3dDevice>, mut source: Box<dyn FrameSource>) -> CoreResult<()> {
        util::set_thread_priority(util::Priority::AboveNormal);
        let interval_us = 1_000_000 / self.fps.max(1) as i64;
        let mut converter: Option<VideoConverter> = None;
        let mut spare: Vec<OwnedFrame> = Vec::with_capacity(8);
        let mut last = OwnedFrame::alloc()?;
        let mut have_last = false;
        let mut next_due: Option<i64> = None;
        let mut last_pts = i64::MIN;

        while !self.stop.load(Ordering::Relaxed) {
            let got = source.next_frame(REPEAT_AFTER)?;
            let mut shell = match spare.pop().or_else(|| self.free_rx.try_recv().ok()) {
                Some(s) => s,
                None => {
                    // Encoder hält alle Frames: Backpressure → verwerfen.
                    if got.is_some() {
                        source.release();
                        self.stats.dropped_frames.fetch_add(1, Ordering::Relaxed);
                    }
                    continue;
                }
            };
            let pts = match got {
                Some(sf) => {
                    // Frameraten-Deckel: zu frühe Frames überspringen.
                    if next_due.is_some_and(|due| sf.pts_us < due - interval_us / 4) {
                        source.release();
                        spare.push(shell);
                        continue;
                    }
                    next_due = Some(match next_due {
                        Some(due) if sf.pts_us - due <= interval_us => due + interval_us,
                        _ => sf.pts_us + interval_us,
                    });
                    let in_size = d3d::texture_size(&sf.texture);
                    if converter.as_ref().map(|c| c.in_size) != Some(in_size) {
                        converter = Some(VideoConverter::new(&dev, in_size, self.out_size, self.fps)?);
                    }
                    let (tex, slice) = match self.frames.get(&mut shell) {
                        Ok(v) => v,
                        Err(e) => {
                            // Pool erschöpft (Encoder hält alle Texturen): verwerfen.
                            tracing::debug!("{e}");
                            source.release();
                            shell.unref();
                            spare.push(shell);
                            self.stats.dropped_frames.fetch_add(1, Ordering::Relaxed);
                            continue;
                        }
                    };
                    let conv = converter.as_mut().ok_or(CoreError::NotRunning)?;
                    let copy = self.frames.needs_copy();
                    let r = self.frames.with_lock(|| {
                        if copy {
                            conv.blt_copy(&dev, &sf.texture, &tex, slice)
                        } else {
                            conv.blt(&sf.texture, &tex, slice)
                        }
                    });
                    source.release();
                    r?;
                    last.unref();
                    // SAFETY: shell enthält einen gültigen Pool-Frame; last ist leer.
                    have_last = unsafe { ffmpeg_sys_next::av_frame_ref(last.0, shell.0) } >= 0;
                    sf.pts_us
                }
                None if have_last => {
                    // SAFETY: last hält eine gültige Referenz; shell ist leer.
                    if unsafe { ffmpeg_sys_next::av_frame_ref(shell.0, last.0) } < 0 {
                        spare.push(shell);
                        continue;
                    }
                    util::now_us()
                }
                None => {
                    spare.push(shell);
                    continue;
                }
            };
            let pts = if pts <= last_pts { last_pts + 1 } else { pts };
            last_pts = pts;
            match self.tx.try_send(CapturedFrame { frame: shell, pts_us: pts }) {
                Ok(()) => {
                    self.stats.captured_frames.fetch_add(1, Ordering::Relaxed);
                }
                Err(TrySendError::Full(mut cf)) => {
                    cf.frame.unref();
                    spare.push(cf.frame);
                    self.stats.dropped_frames.fetch_add(1, Ordering::Relaxed);
                }
                Err(TrySendError::Disconnected(_)) => break,
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct MonitorInfo {
    /// GDI-Gerätename, z. B. `\\.\DISPLAY1`.
    pub device: String,
    pub width: u32,
    pub height: u32,
    pub x: i32,
    pub y: i32,
    pub primary: bool,
}

pub fn list_monitors() -> Vec<MonitorInfo> {
    let mut out: Vec<MonitorInfo> = Vec::new();
    unsafe extern "system" fn cb(m: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
        // SAFETY: data zeigt auf unseren Vec, der während EnumDisplayMonitors lebt.
        let out = unsafe { &mut *(data.0 as *mut Vec<MonitorInfo>) };
        if let Some(info) = monitor_info(m) {
            out.push(info);
        }
        true.into()
    }
    // SAFETY: Callback und Datenzeiger sind während des synchronen Aufrufs gültig.
    unsafe {
        let _ = EnumDisplayMonitors(None, None, Some(cb), LPARAM(&mut out as *mut _ as isize));
    }
    out
}

pub fn monitor_info(m: HMONITOR) -> Option<MonitorInfo> {
    let mut mi = MONITORINFOEXW::default();
    mi.monitorInfo.cbSize = std::mem::size_of::<MONITORINFOEXW>() as u32;
    // SAFETY: mi ist korrekt dimensioniert (cbSize gesetzt).
    let ok = unsafe { GetMonitorInfoW(m, &mut mi as *mut _ as *mut MONITORINFO) }.as_bool();
    if !ok {
        return None;
    }
    let r = mi.monitorInfo.rcMonitor;
    Some(MonitorInfo {
        device: util::wide_to_string(&mi.szDevice),
        width: (r.right - r.left) as u32,
        height: (r.bottom - r.top) as u32,
        x: r.left,
        y: r.top,
        primary: mi.monitorInfo.dwFlags & MONITORINFOF_PRIMARY != 0,
    })
}

/// HMONITOR zu einem Gerätenamen; leer oder unbekannt = primärer Monitor.
pub fn find_monitor(device: &str) -> HMONITOR {
    let mut found: (String, HMONITOR) = (device.to_string(), HMONITOR::default());
    unsafe extern "system" fn cb(m: HMONITOR, _: HDC, _: *mut RECT, data: LPARAM) -> BOOL {
        // SAFETY: data zeigt auf das Tupel oben, das während der Enumeration lebt.
        let found = unsafe { &mut *(data.0 as *mut (String, HMONITOR)) };
        if monitor_info(m).is_some_and(|i| i.device == found.0) {
            found.1 = m;
            return false.into();
        }
        true.into()
    }
    if !device.is_empty() {
        // SAFETY: wie list_monitors.
        unsafe {
            let _ = EnumDisplayMonitors(None, None, Some(cb), LPARAM(&mut found as *mut _ as isize));
        }
    }
    if found.1.is_invalid() {
        // SAFETY: Keine Vorbedingungen.
        unsafe { MonitorFromPoint(Default::default(), MONITOR_DEFAULTTOPRIMARY) }
    } else {
        found.1
    }
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct WindowInfo {
    pub title: String,
    pub exe: String,
}

/// Sichtbare Top-Level-Fenster mit Titel (ohne Tool- und versteckte UWP-Fenster).
pub fn list_windows() -> Vec<WindowInfo> {
    let mut hwnds: Vec<HWND> = Vec::new();
    unsafe extern "system" fn cb(h: HWND, data: LPARAM) -> BOOL {
        // SAFETY: data zeigt auf unseren Vec.
        let v = unsafe { &mut *(data.0 as *mut Vec<HWND>) };
        v.push(h);
        true.into()
    }
    // SAFETY: Callback/Daten gültig während des synchronen Aufrufs.
    unsafe {
        let _ = EnumWindows(Some(cb), LPARAM(&mut hwnds as *mut _ as isize));
    }
    let own = std::process::id();
    let mut out: Vec<WindowInfo> = hwnds
        .into_iter()
        .filter(|&h| is_capturable_window(h))
        .filter(|&h| util::window_pid(h) != own)
        .filter_map(|h| {
            let title = window_title(h);
            let exe = util::window_exe(h)?;
            (!title.is_empty()).then_some(WindowInfo { title, exe })
        })
        .collect();
    out.sort_by(|a, b| a.exe.cmp(&b.exe).then(a.title.cmp(&b.title)));
    out.dedup_by(|a, b| a.exe == b.exe && a.title == b.title);
    out
}

pub fn window_title(h: HWND) -> String {
    // SAFETY: Puffer wird mit der gemeldeten Länge + 1 angelegt.
    unsafe {
        let len = GetWindowTextLengthW(h);
        if len <= 0 {
            return String::new();
        }
        let mut buf = vec![0u16; len as usize + 1];
        let n = GetWindowTextW(h, &mut buf);
        String::from_utf16_lossy(&buf[..n.max(0) as usize])
    }
}

fn is_capturable_window(h: HWND) -> bool {
    // SAFETY: Reine Abfragen auf einem HWND; ungültige HWNDs liefern Fehler/false.
    unsafe {
        if !IsWindowVisible(h).as_bool() {
            return false;
        }
        if GetWindowLongW(h, GWL_EXSTYLE) as u32 & WS_EX_TOOLWINDOW.0 != 0 {
            return false;
        }
        let mut cloaked = 0u32;
        let r = DwmGetWindowAttribute(
            h,
            DWMWA_CLOAKED,
            &mut cloaked as *mut u32 as *mut _,
            std::mem::size_of::<u32>() as u32,
        );
        !(r.is_ok() && cloaked != 0)
    }
}

/// Fenster zu Prozessname + Titel finden (Titel exakt, sonst erstes Fenster des Prozesses).
pub fn find_window(exe: &str, title: &str) -> Option<HWND> {
    let mut hwnds: Vec<HWND> = Vec::new();
    unsafe extern "system" fn cb(h: HWND, data: LPARAM) -> BOOL {
        // SAFETY: data zeigt auf unseren Vec.
        let v = unsafe { &mut *(data.0 as *mut Vec<HWND>) };
        v.push(h);
        true.into()
    }
    // SAFETY: wie list_windows.
    unsafe {
        let _ = EnumWindows(Some(cb), LPARAM(&mut hwnds as *mut _ as isize));
    }
    let candidates: Vec<HWND> = hwnds
        .into_iter()
        .filter(|&h| is_capturable_window(h))
        .filter(|&h| util::window_exe(h).is_some_and(|e| e.eq_ignore_ascii_case(exe)))
        .collect();
    candidates
        .iter()
        .copied()
        .find(|&h| window_title(h) == title)
        .or_else(|| candidates.first().copied())
}
