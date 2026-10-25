//! Video-Encoder auf FFmpeg-Basis.
//!
//! Zero-Copy-Pfad: Der Capture-Thread holt Frames aus dem d3d11-Framepool dieses Encoders
//! (`frames_ref`), schreibt per Video Processor NV12 hinein und übergibt sie per Channel.
//! NVENC und AMF lesen die Texturen direkt; QSV über eine abgeleitete QSV-Frame-Map.
//! Nur der Software-Fallback (`h264_mf`) liest die Frames auf die CPU zurück.

use std::ptr;

use ffmpeg_sys_next as ff;
use windows::core::Interface;
use windows::Win32::Graphics::Direct3D11::{ID3D11Texture2D, D3D11_BIND_DECODER, D3D11_BIND_RENDER_TARGET};

use super::ffi::{self, check, cstr, set_opt, CodecParams, OwnedFrame, OwnedPacket, AVERROR_EAGAIN, US_TB};
use crate::capture::d3d::D3dDevice;
use crate::error::{CoreError, CoreResult};
use crate::settings::{QualityPreset, RateControl};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EncoderKind {
    Nvenc,
    Amf,
    Qsv,
    /// Media Foundation (Software-MFT): CPU-Fallback, LGPL-kompatibel.
    Mf,
}

impl EncoderKind {
    pub fn of(name: &str) -> EncoderKind {
        if name.ends_with("_nvenc") {
            EncoderKind::Nvenc
        } else if name.ends_with("_amf") {
            EncoderKind::Amf
        } else if name.ends_with("_qsv") {
            EncoderKind::Qsv
        } else {
            EncoderKind::Mf
        }
    }

    pub fn is_hardware(self) -> bool {
        self != EncoderKind::Mf
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct VideoEncoderConfig {
    pub encoder: String,
    pub width: u32,
    pub height: u32,
    pub fps: u32,
    pub rate_control: RateControl,
    pub bitrate_kbps: u32,
    pub cqp: u32,
    pub keyint_secs: u32,
    pub preset: QualityPreset,
}

/// Referenz auf den d3d11-Framepool, die an den Capture-Thread geht.
pub struct FramesRef(*mut ff::AVBufferRef, bool);

// SAFETY: AVBufferRef-Pools sind threadsicher (av_buffer_pool_get ist gesperrt).
unsafe impl Send for FramesRef {}

impl FramesRef {
    /// Holt einen freien Frame aus dem Pool. Liefert Textur und Array-Slice.
    pub fn get(&self, frame: &mut OwnedFrame) -> CoreResult<(ID3D11Texture2D, u32)> {
        // SAFETY: frame ist leer (unref) und self.0 ein gültiger Frames-Kontext.
        unsafe {
            check(ff::av_hwframe_get_buffer(self.0, frame.0, 0), "av_hwframe_get_buffer")?;
            let raw = (*frame.0).data[0] as *mut std::ffi::c_void;
            let slice = (*frame.0).data[1] as usize as u32;
            let tex = ID3D11Texture2D::from_raw_borrowed(&raw)
                .ok_or_else(|| CoreError::Encoder("Frame ohne Textur".into()))?
                .clone();
            Ok((tex, slice))
        }
    }

    /// Pool-Texturen sind kein Render-Target (QSV-Array): über Zwischentextur kopieren.
    pub fn needs_copy(&self) -> bool {
        self.1
    }

    /// Sperrt den D3D11-Context wie FFmpeg selbst (rekursiver Lock).
    pub fn with_lock<R>(&self, f: impl FnOnce() -> R) -> R {
        // SAFETY: device_ctx/hwctx sind nach av_hwframe_ctx_init gesetzt und leben mit dem Ref.
        unsafe {
            let fc = (*self.0).data as *mut ff::AVHWFramesContext;
            let dc = (*fc).device_ctx;
            let hw = (*dc).hwctx as *mut ff::AVD3D11VADeviceContext;
            if let (Some(lock), Some(unlock)) = ((*hw).lock, (*hw).unlock) {
                lock((*hw).lock_ctx);
                let r = f();
                unlock((*hw).lock_ctx);
                r
            } else {
                f()
            }
        }
    }
}

impl Drop for FramesRef {
    fn drop(&mut self) {
        // SAFETY: Gibt unsere Referenz frei.
        unsafe { ff::av_buffer_unref(&mut self.0) };
    }
}

pub struct VideoEncoder {
    ctx: *mut ff::AVCodecContext,
    device_ref: *mut ff::AVBufferRef,
    frames_ref: *mut ff::AVBufferRef,
    qsv_device: *mut ff::AVBufferRef,
    qsv_frames: *mut ff::AVBufferRef,
    sw_frame: Option<OwnedFrame>,
    mapped: Option<OwnedFrame>,
    pkt: OwnedPacket,
    pub kind: EncoderKind,
    pub name: String,
    keyint_us: i64,
    frame_us: i64,
    last_key_pts: Option<i64>,
    force_key: bool,
}

// SAFETY: Der Encoder wird genau von einem Thread (EncoderThread) benutzt; er wird nur
// beim Start dorthin verschoben.
unsafe impl Send for VideoEncoder {}

/// QSV braucht ein festes Texture-Array (NV12-Arrays sind nur mit BIND_DECODER erlaubt, nicht als
/// Render-Target). Für NVENC/AMF/MF: 0 = dynamischer Pool aus einzelnen NV12-Render-Targets.
const QSV_POOL_SIZE: i32 = 20;

impl VideoEncoder {
    pub fn new(dev: &D3dDevice, cfg: &VideoEncoderConfig) -> CoreResult<Self> {
        let kind = EncoderKind::of(&cfg.encoder);
        let name = cstr(&cfg.encoder)?;
        // SAFETY: name ist nullterminiert.
        let codec = unsafe { ff::avcodec_find_encoder_by_name(name.as_ptr()) };
        if codec.is_null() {
            return Err(CoreError::Encoder(format!("{} nicht vorhanden", cfg.encoder)));
        }
        let mut me = VideoEncoder {
            ctx: ptr::null_mut(),
            device_ref: ptr::null_mut(),
            frames_ref: ptr::null_mut(),
            qsv_device: ptr::null_mut(),
            qsv_frames: ptr::null_mut(),
            sw_frame: None,
            mapped: None,
            pkt: OwnedPacket::alloc()?,
            kind,
            name: cfg.encoder.clone(),
            keyint_us: cfg.keyint_secs as i64 * 1_000_000,
            frame_us: 1_000_000 / cfg.fps.max(1) as i64,
            last_key_pts: None,
            force_key: false,
        };
        // SAFETY: Alle Zeiger werden direkt nach der Erzeugung geprüft; me räumt in Drop auf.
        unsafe {
            me.init_hw(dev, cfg)?;
            me.ctx = ff::avcodec_alloc_context3(codec);
            if me.ctx.is_null() {
                return Err(CoreError::Encoder("avcodec_alloc_context3".into()));
            }
            let c = &mut *me.ctx;
            c.width = cfg.width as i32;
            c.height = cfg.height as i32;
            c.time_base = US_TB;
            c.framerate = ff::AVRational { num: cfg.fps as i32, den: 1 };
            c.gop_size = (cfg.fps * cfg.keyint_secs) as i32;
            c.max_b_frames = 0;
            c.flags |= ff::AV_CODEC_FLAG_GLOBAL_HEADER as i32;
            c.color_range = ff::AVColorRange::AVCOL_RANGE_MPEG;
            c.colorspace = ff::AVColorSpace::AVCOL_SPC_BT709;
            c.color_primaries = ff::AVColorPrimaries::AVCOL_PRI_BT709;
            c.color_trc = ff::AVColorTransferCharacteristic::AVCOL_TRC_BT709;
            match kind {
                EncoderKind::Nvenc | EncoderKind::Amf => {
                    c.pix_fmt = ff::AVPixelFormat::AV_PIX_FMT_D3D11;
                    c.hw_frames_ctx = ff::av_buffer_ref(me.frames_ref);
                }
                EncoderKind::Qsv => {
                    c.pix_fmt = ff::AVPixelFormat::AV_PIX_FMT_QSV;
                    c.hw_frames_ctx = ff::av_buffer_ref(me.qsv_frames);
                }
                EncoderKind::Mf => {
                    c.pix_fmt = ff::AVPixelFormat::AV_PIX_FMT_NV12;
                }
            }
            let bitrate = cfg.bitrate_kbps as i64 * 1000;
            match cfg.rate_control {
                RateControl::Cbr => {
                    c.bit_rate = bitrate;
                    c.rc_max_rate = bitrate;
                    c.rc_min_rate = bitrate;
                    c.rc_buffer_size = bitrate as i32;
                }
                RateControl::Vbr => {
                    c.bit_rate = bitrate;
                    c.rc_max_rate = bitrate * 3 / 2;
                    c.rc_buffer_size = (bitrate * 2) as i32;
                }
                RateControl::Cqp => {}
            }
            me.apply_options(cfg);
            check(ff::avcodec_open2(me.ctx, codec, ptr::null_mut()), &format!("{} öffnen", cfg.encoder))?;
            if kind == EncoderKind::Mf {
                let f = OwnedFrame::alloc()?;
                (*f.0).format = ff::AVPixelFormat::AV_PIX_FMT_NV12 as i32;
                (*f.0).width = cfg.width as i32;
                (*f.0).height = cfg.height as i32;
                check(ff::av_frame_get_buffer(f.0, 0), "av_frame_get_buffer")?;
                me.sw_frame = Some(f);
            }
            if kind == EncoderKind::Qsv {
                me.mapped = Some(OwnedFrame::alloc()?);
            }
        }
        Ok(me)
    }

    /// d3d11-Gerät und Framepool aufsetzen (für QSV zusätzlich abgeleitet).
    unsafe fn init_hw(&mut self, dev: &D3dDevice, cfg: &VideoEncoderConfig) -> CoreResult<()> {
        // SAFETY: Aufrufer garantiert Einzelaufruf; alle Zeiger werden geprüft.
        unsafe {
            self.device_ref = ff::av_hwdevice_ctx_alloc(ff::AVHWDeviceType::AV_HWDEVICE_TYPE_D3D11VA);
            if self.device_ref.is_null() {
                return Err(CoreError::Encoder("av_hwdevice_ctx_alloc".into()));
            }
            let dc = (*self.device_ref).data as *mut ff::AVHWDeviceContext;
            let hw = (*dc).hwctx as *mut ff::AVD3D11VADeviceContext;
            // FFmpeg übernimmt genau eine Referenz und gibt sie beim Freigeben wieder ab.
            (*hw).device = dev.device.clone().into_raw() as *mut ff::ID3D11Device;
            check(ff::av_hwdevice_ctx_init(self.device_ref), "av_hwdevice_ctx_init")?;

            self.frames_ref = ff::av_hwframe_ctx_alloc(self.device_ref);
            if self.frames_ref.is_null() {
                return Err(CoreError::Encoder("av_hwframe_ctx_alloc".into()));
            }
            let fc = (*self.frames_ref).data as *mut ff::AVHWFramesContext;
            (*fc).format = ff::AVPixelFormat::AV_PIX_FMT_D3D11;
            (*fc).sw_format = ff::AVPixelFormat::AV_PIX_FMT_NV12;
            (*fc).width = cfg.width as i32;
            (*fc).height = cfg.height as i32;
            (*fc).initial_pool_size = if self.kind == EncoderKind::Qsv { QSV_POOL_SIZE } else { 0 };
            let fhw = (*fc).hwctx as *mut ff::AVD3D11VAFramesContext;
            // Render-Target, damit der Video Processor direkt hineinschreiben kann.
            // Render-Target, damit der Video Processor direkt hineinschreiben kann. QSV: Array mit
            // BIND_DECODER, beschrieben per GPU-Kopie aus einer Zwischentextur.
            (*fhw).BindFlags = if self.kind == EncoderKind::Qsv {
                D3D11_BIND_DECODER.0 as u32
            } else {
                D3D11_BIND_RENDER_TARGET.0 as u32
            };
            check(ff::av_hwframe_ctx_init(self.frames_ref), "av_hwframe_ctx_init")?;

            if self.kind == EncoderKind::Qsv {
                check(
                    ff::av_hwdevice_ctx_create_derived(
                        &mut self.qsv_device,
                        ff::AVHWDeviceType::AV_HWDEVICE_TYPE_QSV,
                        self.device_ref,
                        0,
                    ),
                    "QSV-Gerät ableiten",
                )?;
                check(
                    ff::av_hwframe_ctx_create_derived(
                        &mut self.qsv_frames,
                        ff::AVPixelFormat::AV_PIX_FMT_QSV,
                        self.qsv_device,
                        self.frames_ref,
                        0,
                    ),
                    "QSV-Frames ableiten",
                )?;
            }
        }
        Ok(())
    }

    fn apply_options(&mut self, cfg: &VideoEncoderConfig) {
        let o = self.ctx as *mut std::ffi::c_void;
        // SAFETY: ctx ist ein gültiger, noch nicht geöffneter AVCodecContext.
        let set = |k: &str, v: &str| unsafe { set_opt(o, k, v) };
        let qp = cfg.cqp.to_string();
        match self.kind {
            EncoderKind::Nvenc => {
                let preset = match cfg.preset {
                    QualityPreset::Performance => "p2",
                    QualityPreset::Balanced | QualityPreset::Custom => "p4",
                    QualityPreset::Quality => "p6",
                };
                set("preset", preset);
                set("tune", "hq");
                // Pakete sofort ausgeben, kein Lookahead: niedrige Latenz, wenig VRAM.
                set("delay", "0");
                set("rc-lookahead", "0");
                set("forced-idr", "1");
                set("zerolatency", "1");
                match cfg.rate_control {
                    RateControl::Cbr => set("rc", "cbr"),
                    RateControl::Vbr => set("rc", "vbr"),
                    RateControl::Cqp => {
                        set("rc", "constqp");
                        set("qp", &qp);
                    }
                }
                if cfg.preset == QualityPreset::Quality {
                    set("spatial-aq", "1");
                }
            }
            EncoderKind::Amf => {
                let quality = match cfg.preset {
                    QualityPreset::Performance => "speed",
                    QualityPreset::Balanced | QualityPreset::Custom => "balanced",
                    QualityPreset::Quality => "quality",
                };
                set("usage", "lowlatency");
                set("quality", quality);
                set("forced_idr", "1");
                match cfg.rate_control {
                    RateControl::Cbr => set("rc", "cbr"),
                    RateControl::Vbr => set("rc", "vbr_peak"),
                    RateControl::Cqp => {
                        set("rc", "cqp");
                        set("qp_i", &qp);
                        set("qp_p", &qp);
                    }
                }
            }
            EncoderKind::Qsv => {
                let preset = match cfg.preset {
                    QualityPreset::Performance => "veryfast",
                    QualityPreset::Balanced | QualityPreset::Custom => "faster",
                    QualityPreset::Quality => "medium",
                };
                set("preset", preset);
                set("look_ahead", "0");
                set("async_depth", "2");
                if cfg.rate_control == RateControl::Cqp {
                    // SAFETY: ctx ist gültig und noch nicht geöffnet.
                    unsafe {
                        (*self.ctx).flags |= ff::AV_CODEC_FLAG_QSCALE as i32;
                        (*self.ctx).global_quality = cfg.cqp as i32 * ff::FF_QP2LAMBDA;
                    }
                }
            }
            EncoderKind::Mf => {
                set("hw_encoding", "0");
                set("scenario", "display_remoting");
                match cfg.rate_control {
                    RateControl::Cbr => set("rate_control", "cbr"),
                    RateControl::Vbr => set("rate_control", "pc_vbr"),
                    RateControl::Cqp => {
                        set("rate_control", "quality");
                        // MF-Qualität 0–100, invers zu QP.
                        let q = (100 - (cfg.cqp * 100 / 51)).clamp(1, 100);
                        // SAFETY: wie oben.
                        unsafe { (*self.ctx).global_quality = q as i32 };
                    }
                }
            }
        }
    }

    pub fn frames(&self) -> FramesRef {
        // SAFETY: frames_ref ist nach new() gültig; wir geben eine neue Referenz heraus.
        FramesRef(unsafe { ff::av_buffer_ref(self.frames_ref) }, self.kind == EncoderKind::Qsv)
    }

    pub fn params(&self) -> CoreResult<CodecParams> {
        // SAFETY: ctx ist geöffnet.
        unsafe { CodecParams::from_context(self.ctx) }
    }

    pub fn frame_duration_us(&self) -> i64 {
        self.frame_us
    }

    /// Erzwingt einen Keyframe beim nächsten Frame (z. B. Start einer Aufnahme).
    pub fn request_keyframe(&mut self) {
        self.force_key = true;
    }

    /// Encodiert einen d3d11-Frame. Der Frame wird danach geleert (zurück an den Pool).
    pub fn encode(
        &mut self,
        frame: &mut OwnedFrame,
        pts_us: i64,
        sink: &mut dyn FnMut(&OwnedPacket, i64),
    ) -> CoreResult<()> {
        // SAFETY: frame ist ein gültiger d3d11-Frame aus unserem Pool; ctx ist geöffnet.
        unsafe {
            let f = &mut *frame.0;
            f.pts = pts_us;
            let due = self.last_key_pts.is_none_or(|k| pts_us - k >= self.keyint_us - self.frame_us / 2);
            if self.force_key || due {
                f.pict_type = ff::AVPictureType::AV_PICTURE_TYPE_I;
                f.flags |= ff::AV_FRAME_FLAG_KEY;
                self.last_key_pts = Some(pts_us);
                self.force_key = false;
            }
            let send = match self.kind {
                EncoderKind::Nvenc | EncoderKind::Amf => frame.0,
                EncoderKind::Qsv => {
                    let m = self.mapped.as_mut().ok_or(CoreError::NotRunning)?;
                    m.unref();
                    (*m.0).format = ff::AVPixelFormat::AV_PIX_FMT_QSV as i32;
                    check(
                        ff::av_hwframe_map(m.0, frame.0, ff::AV_HWFRAME_MAP_READ as i32),
                        "av_hwframe_map",
                    )?;
                    check(ff::av_frame_copy_props(m.0, frame.0), "av_frame_copy_props")?;
                    m.0
                }
                EncoderKind::Mf => {
                    let s = self.sw_frame.as_mut().ok_or(CoreError::NotRunning)?;
                    check(ff::av_frame_make_writable(s.0), "av_frame_make_writable")?;
                    check(ff::av_hwframe_transfer_data(s.0, frame.0, 0), "Readback")?;
                    check(ff::av_frame_copy_props(s.0, frame.0), "av_frame_copy_props")?;
                    s.0
                }
            };
            let r = ff::avcodec_send_frame(self.ctx, send);
            frame.unref();
            if let Some(m) = self.mapped.as_mut() {
                m.unref();
            }
            if r < 0 && r != AVERROR_EAGAIN {
                check(r, "avcodec_send_frame")?;
            }
        }
        self.drain(sink)
    }

    fn drain(&mut self, sink: &mut dyn FnMut(&OwnedPacket, i64)) -> CoreResult<()> {
        loop {
            // SAFETY: ctx und pkt sind gültig.
            let r = unsafe { ff::avcodec_receive_packet(self.ctx, self.pkt.0) };
            if r == AVERROR_EAGAIN || r == ff::AVERROR_EOF {
                return Ok(());
            }
            check(r, "avcodec_receive_packet")?;
            sink(&self.pkt, self.frame_us);
            self.pkt.unref();
        }
    }

    /// Leert den Encoder am Ende einer Session.
    pub fn flush(&mut self, sink: &mut dyn FnMut(&OwnedPacket, i64)) -> CoreResult<()> {
        // SAFETY: NULL-Frame signalisiert EOF.
        let r = unsafe { ff::avcodec_send_frame(self.ctx, ptr::null()) };
        if r < 0 && r != ff::AVERROR_EOF {
            check(r, "flush")?;
        }
        self.drain(sink)
    }
}

impl Drop for VideoEncoder {
    fn drop(&mut self) {
        // SAFETY: Alle Zeiger gehören uns; NULL ist für die Free-Funktionen erlaubt.
        unsafe {
            self.sw_frame = None;
            self.mapped = None;
            ff::avcodec_free_context(&mut self.ctx);
            ff::av_buffer_unref(&mut self.qsv_frames);
            ff::av_buffer_unref(&mut self.qsv_device);
            ff::av_buffer_unref(&mut self.frames_ref);
            ff::av_buffer_unref(&mut self.device_ref);
        }
    }
}

/// Paket-Hilfen für den Sink.
pub fn packet_info(p: &OwnedPacket) -> (i64, i64, i64, bool) {
    // SAFETY: Paket ist gültig, solange der Sink läuft.
    unsafe {
        let r = &*p.0;
        (r.pts, r.dts, r.duration, r.flags & ff::AV_PKT_FLAG_KEY != 0)
    }
}

pub use ffi::init_logging;
