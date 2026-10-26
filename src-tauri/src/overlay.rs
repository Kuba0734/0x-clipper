//! Toast-Overlay als natives, click-through Layered Window
//! (`WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE`). Gezeichnet mit Direct2D
//! in eine DIB, angezeigt per `UpdateLayeredWindow` – kein WebView, stiehlt nie den Fokus.
//!
//! Animation (Design-System): seitlicher Einflug mit cubic-bezier(0.22, 1, 0.36, 1), Häkchen
//! wird als Pfad gezeichnet, ablaufende Fortschrittsleiste, dann Auto-Dismiss. Timer laufen
//! nur, solange ein Toast sichtbar ist.

use std::cell::RefCell;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::OnceLock;
use std::time::Instant;

use crossbeam_channel::{unbounded, Receiver, Sender};
use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Direct2D::Common::*;
use windows::Win32::Graphics::Direct2D::*;
use windows::Win32::Graphics::DirectWrite::*;
use windows::Win32::Graphics::Dxgi::Common::DXGI_FORMAT_B8G8R8A8_UNORM;
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{GetDpiForMonitor, MDT_EFFECTIVE_DPI};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows_numerics::Vector2;

use crate::settings::{Language, OverlayPosition, Settings};
use crate::util;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToastKind {
    ClipSaved,
    RecordingStarted,
    RecordingSaved,
    BufferOn,
    BufferOff,
    MicMuted,
    MicUnmuted,
    Error,
}

impl ToastKind {
    fn title(self, lang: Language) -> &'static str {
        let de = lang == Language::De;
        match self {
            ToastKind::ClipSaved => if de { "Clip gespeichert" } else { "Clip saved" },
            ToastKind::RecordingStarted => if de { "Aufnahme läuft" } else { "Recording" },
            ToastKind::RecordingSaved => if de { "Aufnahme gespeichert" } else { "Recording saved" },
            ToastKind::BufferOn => if de { "Replay-Buffer an" } else { "Replay buffer on" },
            ToastKind::BufferOff => if de { "Replay-Buffer aus" } else { "Replay buffer off" },
            ToastKind::MicMuted => if de { "Mikrofon stumm" } else { "Microphone muted" },
            ToastKind::MicUnmuted => if de { "Mikrofon an" } else { "Microphone on" },
            ToastKind::Error => if de { "Fehler" } else { "Error" },
        }
    }

    fn icon(self) -> Icon {
        match self {
            ToastKind::ClipSaved | ToastKind::RecordingSaved | ToastKind::BufferOn | ToastKind::MicUnmuted => Icon::Check,
            ToastKind::RecordingStarted => Icon::Dot,
            ToastKind::BufferOff | ToastKind::MicMuted => Icon::Pause,
            ToastKind::Error => Icon::Cross,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Icon {
    Check,
    Dot,
    Pause,
    Cross,
}

struct Request {
    kind: ToastKind,
    title: String,
    detail: String,
    position: OverlayPosition,
    duration_ms: u32,
    reduce_motion: bool,
}

#[derive(Clone)]
pub struct Overlay {
    tx: Option<Sender<Request>>,
}

static HWND_RAW: AtomicIsize = AtomicIsize::new(0);
static RX: OnceLock<Receiver<Request>> = OnceLock::new();
const WM_TOAST: u32 = WM_APP + 1;
const TIMER_FRAME: usize = 1;

impl Overlay {
    pub fn start() -> Self {
        let (tx, rx) = unbounded::<Request>();
        if RX.set(rx).is_err() {
            return Overlay { tx: Some(tx) };
        }
        let r = util::spawn("overlay", util::Priority::BelowNormal, || {
            if let Err(e) = run() {
                tracing::error!("Overlay: {e}");
            }
        });
        if let Err(e) = r {
            tracing::error!("Overlay-Thread: {e}");
            return Overlay { tx: None };
        }
        Overlay { tx: Some(tx) }
    }

    pub fn show(&self, kind: ToastKind, detail: String, s: &Settings) {
        if !s.overlay.enabled {
            return;
        }
        let Some(tx) = &self.tx else { return };
        let req = Request {
            kind,
            title: kind.title(s.general.language).to_string(),
            detail,
            position: s.overlay.position,
            duration_ms: s.overlay.duration_ms,
            reduce_motion: s.general.reduce_motion,
        };
        if tx.send(req).is_ok() {
            let h = HWND(HWND_RAW.load(Ordering::Acquire) as *mut _);
            if !h.is_invalid() {
                // SAFETY: Nachricht an unser eigenes Fenster; kein Zeiger im Payload.
                unsafe {
                    let _ = PostMessageW(Some(h), WM_TOAST, WPARAM(0), LPARAM(0));
                }
            }
        }
    }
}

// Logische Größe in DIP.
const W: f32 = 340.0;
const H: f32 = 76.0;
const MARGIN: f32 = 24.0;

struct Renderer {
    factory: ID2D1Factory,
    rt: ID2D1DCRenderTarget,
    title_fmt: IDWriteTextFormat,
    detail_fmt: IDWriteTextFormat,
    stroke: ID2D1StrokeStyle,
    mem_dc: HDC,
    bitmap: HBITMAP,
    old: HGDIOBJ,
    scale: f32,
    px: (i32, i32),
}

impl Renderer {
    fn new(scale: f32) -> windows::core::Result<Self> {
        // SAFETY: Standard-D2D/DWrite/GDI-Initialisierung; Ressourcen werden in Drop freigegeben.
        unsafe {
            let factory: ID2D1Factory = D2D1CreateFactory(D2D1_FACTORY_TYPE_SINGLE_THREADED, None)?;
            let props = D2D1_RENDER_TARGET_PROPERTIES {
                r#type: D2D1_RENDER_TARGET_TYPE_DEFAULT,
                pixelFormat: D2D1_PIXEL_FORMAT {
                    format: DXGI_FORMAT_B8G8R8A8_UNORM,
                    alphaMode: D2D1_ALPHA_MODE_PREMULTIPLIED,
                },
                dpiX: 96.0 * scale,
                dpiY: 96.0 * scale,
                usage: D2D1_RENDER_TARGET_USAGE_NONE,
                minLevel: D2D1_FEATURE_LEVEL_DEFAULT,
            };
            let rt = factory.CreateDCRenderTarget(&props)?;
            rt.SetTextAntialiasMode(D2D1_TEXT_ANTIALIAS_MODE_GRAYSCALE);
            let dw: IDWriteFactory = DWriteCreateFactory(DWRITE_FACTORY_TYPE_SHARED)?;
            let title_fmt = dw.CreateTextFormat(
                w!("Segoe UI Variable Text"),
                None,
                DWRITE_FONT_WEIGHT_SEMI_BOLD,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                15.0,
                w!(""),
            )?;
            let detail_fmt = dw.CreateTextFormat(
                w!("Segoe UI Variable Text"),
                None,
                DWRITE_FONT_WEIGHT_NORMAL,
                DWRITE_FONT_STYLE_NORMAL,
                DWRITE_FONT_STRETCH_NORMAL,
                12.5,
                w!(""),
            )?;
            for f in [&title_fmt, &detail_fmt] {
                f.SetWordWrapping(DWRITE_WORD_WRAPPING_NO_WRAP)?;
                let trimming = DWRITE_TRIMMING { granularity: DWRITE_TRIMMING_GRANULARITY_CHARACTER, delimiter: 0, delimiterCount: 0 };
                f.SetTrimming(&trimming, None)?;
            }
            let stroke = factory.CreateStrokeStyle(
                &D2D1_STROKE_STYLE_PROPERTIES {
                    startCap: D2D1_CAP_STYLE_ROUND,
                    endCap: D2D1_CAP_STYLE_ROUND,
                    dashCap: D2D1_CAP_STYLE_ROUND,
                    lineJoin: D2D1_LINE_JOIN_ROUND,
                    miterLimit: 1.0,
                    dashStyle: D2D1_DASH_STYLE_SOLID,
                    dashOffset: 0.0,
                },
                None,
            )?;
            let px = ((W * scale).round() as i32, (H * scale).round() as i32);
            let screen = GetDC(None);
            let mem_dc = CreateCompatibleDC(Some(screen));
            ReleaseDC(None, screen);
            let bmi = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: px.0,
                    biHeight: -px.1,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB.0,
                    ..Default::default()
                },
                ..Default::default()
            };
            let mut bits = std::ptr::null_mut();
            let bitmap = CreateDIBSection(Some(mem_dc), &bmi, DIB_RGB_COLORS, &mut bits, None, 0)?;
            let old = SelectObject(mem_dc, bitmap.into());
            Ok(Renderer { factory, rt, title_fmt, detail_fmt, stroke, mem_dc, bitmap, old, scale, px })
        }
    }

    fn draw(&self, req: &Request, check_progress: f32, bar: f32) -> windows::core::Result<()> {
        let rt = &self.rt;
        let rect = RECT { left: 0, top: 0, right: self.px.0, bottom: self.px.1 };
        // SAFETY: Render-Target ist an unseren Memory-DC gebunden; alle Ressourcen leben im Block.
        unsafe {
            rt.BindDC(self.mem_dc, &rect)?;
            rt.BeginDraw();
            rt.Clear(Some(&color(0x000000, 0.0)));
            let card = D2D1_ROUNDED_RECT {
                rect: D2D_RECT_F { left: 0.5, top: 0.5, right: W - 0.5, bottom: H - 0.5 },
                radiusX: 16.0,
                radiusY: 16.0,
            };
            // Monochrom wie die App: dunkles Glas, 1px-Rand rgba(255,255,255,0.08).
            let bg = rt.CreateSolidColorBrush(&color(0x111113, 0.92), None)?;
            let border = rt.CreateSolidColorBrush(&color(0xFFFFFF, 0.08), None)?;
            rt.FillRoundedRectangle(&card, &bg);
            rt.DrawRoundedRectangle(&card, &border, 1.0, None);

            // Icon-Kreis: Weiß (Akzent) mit dunklem Symbol; Rot nur für Fehler und Aufnahme.
            let center = Vector2 { X: 38.0, Y: H / 2.0 };
            let circle = D2D1_ELLIPSE { point: center, radiusX: 18.0, radiusY: 18.0 };
            let danger = matches!(req.kind, ToastKind::Error | ToastKind::RecordingStarted);
            let (fill, glyph) = if danger { (0xFF453A, 0xFFFFFF) } else { (0xFFFFFF, 0x0A0A0B) };
            let fill_b = rt.CreateSolidColorBrush(&color(fill, 1.0), None)?;
            rt.FillEllipse(&circle, &fill_b);
            let white = rt.CreateSolidColorBrush(&color(glyph, 1.0), None)?;
            match req.kind.icon() {
                Icon::Check => {
                    // Häkchen als Pfad, gezeichnet bis `check_progress` (pathLength).
                    let pts = [
                        Vector2 { X: center.X - 7.5, Y: center.Y + 0.5 },
                        Vector2 { X: center.X - 2.5, Y: center.Y + 5.5 },
                        Vector2 { X: center.X + 8.0, Y: center.Y - 5.5 },
                    ];
                    draw_partial_polyline(rt, &pts, check_progress, &white, &self.stroke);
                }
                Icon::Dot => {
                    let d = D2D1_ELLIPSE { point: center, radiusX: 6.0, radiusY: 6.0 };
                    rt.FillEllipse(&d, &white);
                }
                Icon::Pause => {
                    for dx in [-4.0f32, 4.0] {
                        rt.DrawLine(
                            Vector2 { X: center.X + dx, Y: center.Y - 6.0 },
                            Vector2 { X: center.X + dx, Y: center.Y + 6.0 },
                            &white,
                            3.0,
                            &self.stroke,
                        );
                    }
                }
                Icon::Cross => {
                    let p = check_progress;
                    let a = [Vector2 { X: center.X - 6.0, Y: center.Y - 6.0 }, Vector2 { X: center.X + 6.0, Y: center.Y + 6.0 }];
                    let b = [Vector2 { X: center.X + 6.0, Y: center.Y - 6.0 }, Vector2 { X: center.X - 6.0, Y: center.Y + 6.0 }];
                    draw_partial_polyline(rt, &a, (p * 2.0).min(1.0), &white, &self.stroke);
                    draw_partial_polyline(rt, &b, (p * 2.0 - 1.0).max(0.0), &white, &self.stroke);
                }
            }

            let text = rt.CreateSolidColorBrush(&color(0xF4F4F5, 1.0), None)?;
            let muted = rt.CreateSolidColorBrush(&color(0x8E8E93, 1.0), None)?;
            let has_detail = !req.detail.is_empty();
            let title_top = if has_detail { 17.0 } else { (H - 20.0) / 2.0 };
            let title: Vec<u16> = req.title.encode_utf16().collect();
            rt.DrawText(
                &title,
                &self.title_fmt,
                &D2D_RECT_F { left: 68.0, top: title_top, right: W - 16.0, bottom: title_top + 22.0 },
                &text,
                D2D1_DRAW_TEXT_OPTIONS_CLIP,
                DWRITE_MEASURING_MODE_NATURAL,
            );
            if has_detail {
                let detail: Vec<u16> = req.detail.encode_utf16().collect();
                rt.DrawText(
                    &detail,
                    &self.detail_fmt,
                    &D2D_RECT_F { left: 68.0, top: 40.0, right: W - 16.0, bottom: 60.0 },
                    &muted,
                    D2D1_DRAW_TEXT_OPTIONS_CLIP,
                    DWRITE_MEASURING_MODE_NATURAL,
                );
            }
            // Ablaufende Fortschrittsleiste (Restzeit bis zum Ausblenden).
            if bar > 0.0 {
                let c = if req.kind == ToastKind::Error { 0xFF453A } else { 0xFFFFFF };
                let b = rt.CreateSolidColorBrush(&color(c, 0.85), None)?;
                let track = rt.CreateSolidColorBrush(&color(0xFFFFFF, 0.06), None)?;
                let (x0, x1, y) = (16.0, W - 16.0, H - 6.0);
                rt.FillRectangle(&D2D_RECT_F { left: x0, top: y, right: x1, bottom: y + 2.0 }, &track);
                rt.FillRectangle(&D2D_RECT_F { left: x0, top: y, right: x0 + (x1 - x0) * bar.min(1.0), bottom: y + 2.0 }, &b);
            }
            rt.EndDraw(None, None)?;
            // GDI puffert Zeichenbefehle: vor UpdateLayeredWindow sicher in die DIB schreiben.
            let _ = GdiFlush();
        }
        let _ = &self.factory;
        Ok(())
    }

    fn present(&self, hwnd: HWND, pos: (i32, i32), alpha: f32) {
        let blend = BLENDFUNCTION {
            BlendOp: AC_SRC_OVER as u8,
            BlendFlags: 0,
            SourceConstantAlpha: (alpha.clamp(0.0, 1.0) * 255.0) as u8,
            AlphaFormat: AC_SRC_ALPHA as u8,
        };
        let dst = POINT { x: pos.0, y: pos.1 };
        let size = SIZE { cx: self.px.0, cy: self.px.1 };
        let src = POINT::default();
        // SAFETY: Fenster ist ein Layered Window; mem_dc enthält die 32-bit-DIB.
        unsafe {
            let screen = GetDC(None);
            let _ = UpdateLayeredWindow(
                hwnd,
                Some(screen),
                Some(&dst),
                Some(&size),
                Some(self.mem_dc),
                Some(&src),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            );
            ReleaseDC(None, screen);
        }
    }
}

impl Drop for Renderer {
    fn drop(&mut self) {
        // SAFETY: Eigene GDI-Objekte, genau einmal freigegeben.
        unsafe {
            SelectObject(self.mem_dc, self.old);
            let _ = DeleteObject(self.bitmap.into());
            let _ = DeleteDC(self.mem_dc);
        }
    }
}

fn draw_partial_polyline(
    rt: &ID2D1DCRenderTarget,
    pts: &[Vector2],
    progress: f32,
    brush: &ID2D1SolidColorBrush,
    stroke: &ID2D1StrokeStyle,
) {
    if progress <= 0.0 || pts.len() < 2 {
        return;
    }
    let seg = |a: Vector2, b: Vector2| ((b.X - a.X).powi(2) + (b.Y - a.Y).powi(2)).sqrt();
    let total: f32 = pts.windows(2).map(|w| seg(w[0], w[1])).sum();
    let mut left = total * progress.min(1.0);
    for w in pts.windows(2) {
        let len = seg(w[0], w[1]);
        let t = (left / len).min(1.0);
        let end = Vector2 { X: w[0].X + (w[1].X - w[0].X) * t, Y: w[0].Y + (w[1].Y - w[0].Y) * t };
        // SAFETY: Zeichnen innerhalb von BeginDraw/EndDraw.
        unsafe { rt.DrawLine(w[0], end, brush, 3.0, stroke) };
        left -= len;
        if left <= 0.0 {
            break;
        }
    }
}

fn color(rgb: u32, a: f32) -> D2D1_COLOR_F {
    D2D1_COLOR_F {
        r: ((rgb >> 16) & 0xFF) as f32 / 255.0,
        g: ((rgb >> 8) & 0xFF) as f32 / 255.0,
        b: (rgb & 0xFF) as f32 / 255.0,
        a,
    }
}

/// Standard-Easing des Design-Systems: cubic-bezier(0.22, 1, 0.36, 1).
fn ease(t: f32) -> f32 {
    const X1: f32 = 0.22;
    const Y1: f32 = 1.0;
    const X2: f32 = 0.36;
    const Y2: f32 = 1.0;
    let t = t.clamp(0.0, 1.0);
    let bez = |a: f32, b: f32, s: f32| {
        let u = 1.0 - s;
        3.0 * u * u * s * a + 3.0 * u * s * s * b + s * s * s
    };
    // x(s) = t per Newton-Verfahren lösen, dann y(s) auswerten.
    let mut s = t;
    for _ in 0..8 {
        let x = bez(X1, X2, s) - t;
        let u = 1.0 - s;
        let dx = 3.0 * u * u * X1 + 6.0 * u * s * (X2 - X1) + 3.0 * s * s * (1.0 - X2);
        if dx.abs() < 1e-6 {
            break;
        }
        s = (s - x / dx).clamp(0.0, 1.0);
    }
    bez(Y1, Y2, s)
}

struct Active {
    req: Request,
    started: Instant,
    leaving: Option<Instant>,
    renderer: Renderer,
    anchor: (i32, i32),
    dir: f32,
    drawn_progress: f32,
    drawn_bar_px: f32,
    slow_timer: bool,
}

thread_local! {
    static ACTIVE: RefCell<Option<Active>> = const { RefCell::new(None) };
}

const IN_MS: f32 = 300.0;
const FRAME_FAST_MS: u32 = 16;
const FRAME_SLOW_MS: u32 = 33;
const OUT_MS: f32 = 220.0;

fn run() -> windows::core::Result<()> {
    // SAFETY: Klassenregistrierung und Fenstererstellung auf diesem Thread; die
    // Nachrichtenschleife blockiert (kein Polling).
    unsafe {
        let hinst = GetModuleHandleW(None)?;
        let class = WNDCLASSEXW {
            cbSize: std::mem::size_of::<WNDCLASSEXW>() as u32,
            lpfnWndProc: Some(wndproc),
            hInstance: hinst.into(),
            lpszClassName: w!("0xClipperToast"),
            ..Default::default()
        };
        RegisterClassExW(&class);
        let hwnd = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOPMOST | WS_EX_TOOLWINDOW,
            w!("0xClipperToast"),
            PCWSTR::null(),
            WS_POPUP,
            0,
            0,
            1,
            1,
            None,
            None,
            Some(hinst.into()),
            None,
        )?;
        HWND_RAW.store(hwnd.0 as isize, Ordering::Release);
        // Anfragen, die vor dem Fenster kamen, abholen.
        let _ = PostMessageW(Some(hwnd), WM_TOAST, WPARAM(0), LPARAM(0));
        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            DispatchMessageW(&msg);
        }
    }
    Ok(())
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wp: WPARAM, lp: LPARAM) -> LRESULT {
    match msg {
        WM_TOAST => {
            if let Some(rx) = RX.get() {
                // Nur die neueste Anfrage zählt.
                if let Some(req) = rx.try_iter().last() {
                    begin(hwnd, req);
                }
            }
            LRESULT(0)
        }
        WM_TIMER => {
            tick(hwnd);
            LRESULT(0)
        }
        // SAFETY: Standardbehandlung.
        _ => unsafe { DefWindowProcW(hwnd, msg, wp, lp) },
    }
}

fn begin(hwnd: HWND, req: Request) {
    // Monitor des Vordergrundfensters (= Spiel), DPI-skaliert.
    // SAFETY: Reine Abfragen.
    let (mon_rect, scale) = unsafe {
        let mon = MonitorFromWindow(GetForegroundWindow(), MONITOR_DEFAULTTOPRIMARY);
        let mut mi = MONITORINFO { cbSize: std::mem::size_of::<MONITORINFO>() as u32, ..Default::default() };
        let _ = GetMonitorInfoW(mon, &mut mi);
        let (mut dx, mut dy) = (96u32, 96u32);
        let _ = GetDpiForMonitor(mon, MDT_EFFECTIVE_DPI, &mut dx, &mut dy);
        (mi.rcWork, dx as f32 / 96.0)
    };
    let renderer = match Renderer::new(scale) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!("Overlay-Renderer: {e}");
            return;
        }
    };
    let (pw, ph) = renderer.px;
    let m = (MARGIN * scale) as i32;
    let (x, dir) = match req.position {
        OverlayPosition::TopLeft | OverlayPosition::BottomLeft => (mon_rect.left + m, -1.0),
        OverlayPosition::TopRight | OverlayPosition::BottomRight => (mon_rect.right - m - pw, 1.0),
    };
    let y = match req.position {
        OverlayPosition::TopLeft | OverlayPosition::TopRight => mon_rect.top + m,
        OverlayPosition::BottomLeft | OverlayPosition::BottomRight => mon_rect.bottom - m - ph,
    };
    ACTIVE.with(|a| {
        *a.borrow_mut() = Some(Active {
            req,
            started: Instant::now(),
            leaving: None,
            renderer,
            anchor: (x, y),
            dir,
            drawn_progress: -1.0,
            drawn_bar_px: -1.0,
            slow_timer: false,
        });
    });
    // SAFETY: Eigenes Fenster; SW_SHOWNOACTIVATE aktiviert nie.
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
        SetTimer(Some(hwnd), TIMER_FRAME, FRAME_FAST_MS, None);
    }
    tick(hwnd);
}

fn tick(hwnd: HWND) {
    ACTIVE.with(|cell| {
        let mut slot = cell.borrow_mut();
        let Some(a) = slot.as_mut() else { return };
        let t = a.started.elapsed().as_secs_f32() * 1000.0;
        let reduce = a.req.reduce_motion;
        let total = a.req.duration_ms as f32;
        if a.leaving.is_none() && t >= total {
            a.leaving = Some(Instant::now());
            set_frame_timer(hwnd, FRAME_FAST_MS);
        }
        // Einflug: seitlich + Fade mit dem Standard-Easing; Häkchen wird danach gezeichnet.
        let (mut offset, mut alpha, check) = if reduce {
            (0.0, 1.0, 1.0)
        } else {
            let p = ease((t / IN_MS).min(1.0));
            ((1.0 - p) * 48.0 * a.dir, p, ease(((t - 120.0) / 260.0).clamp(0.0, 1.0)))
        };
        if let Some(l) = a.leaving {
            let p = if reduce { 1.0 } else { (l.elapsed().as_secs_f32() * 1000.0 / OUT_MS).min(1.0) };
            alpha *= 1.0 - ease(p);
            offset += ease(p) * 48.0 * a.dir;
            if p >= 1.0 {
                // SAFETY: eigenes Fenster.
                unsafe {
                    let _ = KillTimer(Some(hwnd), TIMER_FRAME);
                    let _ = ShowWindow(hwnd, SW_HIDE);
                }
                *slot = None;
                return;
            }
        }
        // Ablaufende Leiste (bei reduzierter Bewegung aus).
        let bar = if reduce { 0.0 } else { (1.0 - t / total).clamp(0.0, 1.0) };
        let bar_px = (bar * (W - 32.0) * a.renderer.scale).round();
        if (check - a.drawn_progress).abs() > 0.001 || bar_px != a.drawn_bar_px {
            if let Err(e) = a.renderer.draw(&a.req, check, bar) {
                tracing::warn!("Overlay zeichnen: {e}");
            }
            a.drawn_progress = check;
            a.drawn_bar_px = bar_px;
        }
        let x = a.anchor.0 + (offset * a.renderer.scale) as i32;
        a.renderer.present(hwnd, (x, a.anchor.1), alpha);

        // Timer-Takt: 60 fps für Ein-/Ausflug, 30 fps für die Leiste, sonst nur ein
        // einmaliger Weck-Timer bis zum Ausblenden.
        if a.leaving.is_none() && t >= IN_MS.max(400.0) {
            if reduce {
                // SAFETY: eigenes Fenster.
                unsafe {
                    let _ = KillTimer(Some(hwnd), TIMER_FRAME);
                    SetTimer(Some(hwnd), TIMER_FRAME, ((total - t).max(1.0)) as u32, None);
                }
            } else if !a.slow_timer {
                set_frame_timer(hwnd, FRAME_SLOW_MS);
                a.slow_timer = true;
            }
        }
    });
}

fn set_frame_timer(hwnd: HWND, ms: u32) {
    // SAFETY: eigenes Fenster; SetTimer mit gleicher ID ersetzt den alten Timer.
    unsafe {
        SetTimer(Some(hwnd), TIMER_FRAME, ms, None);
    }
}

#[cfg(test)]
mod tests {
    use super::ease;

    #[test]
    fn ease_matches_css_curve() {
        assert!(ease(0.0).abs() < 1e-4);
        assert!((ease(1.0) - 1.0).abs() < 1e-4);
        // cubic-bezier(0.22, 1, 0.36, 1) ist stark ausrollend: bei 30 % schon > 75 %.
        assert!(ease(0.3) > 0.75, "{}", ease(0.3));
        let mut last = 0.0;
        for i in 0..=100 {
            let v = ease(i as f32 / 100.0);
            assert!(v + 1e-4 >= last);
            last = v;
        }
    }
}
