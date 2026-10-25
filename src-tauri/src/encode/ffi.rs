//! Dünne, besitzende Wrapper um FFmpeg-Zeiger. Alle `unsafe`-Blöcke sind lokal begründet.

use std::ffi::{CStr, CString};
use std::ptr;

use ffmpeg_sys_next as ff;

use crate::error::{CoreError, CoreResult};

/// `AVERROR(EAGAIN)` (MSVC: EAGAIN = 11).
pub const AVERROR_EAGAIN: i32 = -11;

pub fn err_string(code: i32) -> String {
    let mut buf = [0 as std::ffi::c_char; 128];
    // SAFETY: Puffer ist groß genug und gültig; av_strerror schreibt nullterminiert.
    unsafe {
        ff::av_strerror(code, buf.as_mut_ptr(), buf.len());
        CStr::from_ptr(buf.as_ptr()).to_string_lossy().into_owned()
    }
}

pub fn check(code: i32, what: &str) -> CoreResult<i32> {
    if code < 0 {
        Err(CoreError::Ffmpeg(format!("{what}: {}", err_string(code))))
    } else {
        Ok(code)
    }
}

pub fn cstr(s: &str) -> CoreResult<CString> {
    CString::new(s).map_err(|_| CoreError::Ffmpeg(format!("Nullbyte in {s:?}")))
}

/// Setzt eine private Encoder-Option; unbekannte Optionen werden ignoriert (je nach Encoder).
///
/// # Safety
/// `obj` muss ein gültiges, AVClass-fähiges Objekt sein (z. B. ein AVCodecContext).
pub unsafe fn set_opt(obj: *mut std::ffi::c_void, key: &str, val: &str) {
    let (Ok(k), Ok(v)) = (CString::new(key), CString::new(val)) else { return };
    // SAFETY: laut Vertrag gültiges Objekt; Strings sind nullterminiert.
    let r = unsafe { ff::av_opt_set(obj, k.as_ptr(), v.as_ptr(), ff::AV_OPT_SEARCH_CHILDREN) };
    if r < 0 {
        tracing::debug!("Encoder-Option {key}={val} nicht gesetzt: {}", err_string(r));
    }
}

/// Schaltet FFmpegs eigenes stderr-Logging ab; Fehler kommen über Rückgabecodes
/// und landen in unserem Log (die Encoder-Erkennung erzeugt sonst viel Rauschen).
pub fn init_logging() {
    // SAFETY: Globale, threadsichere Einstellung ohne Zeiger.
    unsafe { ff::av_log_set_level(ff::AV_LOG_FATAL) };
}

pub struct OwnedFrame(pub *mut ff::AVFrame);

impl OwnedFrame {
    pub fn alloc() -> CoreResult<Self> {
        // SAFETY: av_frame_alloc hat keine Vorbedingungen.
        let f = unsafe { ff::av_frame_alloc() };
        if f.is_null() {
            return Err(CoreError::Ffmpeg("av_frame_alloc".into()));
        }
        Ok(OwnedFrame(f))
    }

    pub fn unref(&mut self) {
        // SAFETY: self.0 ist ein gültiger, von uns besessener AVFrame.
        unsafe { ff::av_frame_unref(self.0) };
    }
}

impl Drop for OwnedFrame {
    fn drop(&mut self) {
        // SAFETY: av_frame_free akzeptiert einen Zeiger auf unseren eigenen Zeiger.
        unsafe { ff::av_frame_free(&mut self.0) };
    }
}

// SAFETY: Ein AVFrame gehört genau einem Thread zur Zeit; wir übergeben ihn nur per Channel.
unsafe impl Send for OwnedFrame {}

pub struct OwnedPacket(pub *mut ff::AVPacket);

impl OwnedPacket {
    pub fn alloc() -> CoreResult<Self> {
        // SAFETY: Keine Vorbedingungen.
        let p = unsafe { ff::av_packet_alloc() };
        if p.is_null() {
            return Err(CoreError::Ffmpeg("av_packet_alloc".into()));
        }
        Ok(OwnedPacket(p))
    }

    /// Daten des Pakets als Slice (gültig bis zum nächsten unref).
    pub fn data(&self) -> &[u8] {
        // SAFETY: data/size beschreiben den Paketpuffer, solange das Paket nicht verändert wird.
        unsafe {
            let p = &*self.0;
            if p.data.is_null() || p.size <= 0 {
                &[]
            } else {
                std::slice::from_raw_parts(p.data, p.size as usize)
            }
        }
    }

    pub fn unref(&mut self) {
        // SAFETY: Gültiges Paket.
        unsafe { ff::av_packet_unref(self.0) };
    }
}

impl Drop for OwnedPacket {
    fn drop(&mut self) {
        // SAFETY: Gibt unser eigenes Paket frei.
        unsafe { ff::av_packet_free(&mut self.0) };
    }
}

// SAFETY: Wie OwnedFrame – nie gleichzeitig von mehreren Threads benutzt.
unsafe impl Send for OwnedPacket {}

/// Unveränderliche Codec-Parameter eines Streams (für den Muxer).
#[derive(Debug)]
pub struct CodecParams {
    ptr: *mut ff::AVCodecParameters,
}

impl CodecParams {
    /// Kopiert die Parameter aus einem geöffneten Encoder.
    ///
    /// # Safety
    /// `ctx` muss ein gültiger, geöffneter AVCodecContext sein.
    pub unsafe fn from_context(ctx: *const ff::AVCodecContext) -> CoreResult<Self> {
        // SAFETY: laut Vertrag gültiger Kontext; Rückgabe wird geprüft.
        unsafe {
            let ptr = ff::avcodec_parameters_alloc();
            if ptr.is_null() {
                return Err(CoreError::Ffmpeg("avcodec_parameters_alloc".into()));
            }
            let me = CodecParams { ptr };
            check(ff::avcodec_parameters_from_context(ptr, ctx), "avcodec_parameters_from_context")?;
            Ok(me)
        }
    }

    /// # Safety
    /// `src` muss gültige AVCodecParameters sein.
    pub unsafe fn from_raw_copy(src: *const ff::AVCodecParameters) -> CoreResult<Self> {
        // SAFETY: laut Vertrag gültige Quelle.
        unsafe {
            let ptr = ff::avcodec_parameters_alloc();
            if ptr.is_null() {
                return Err(CoreError::Ffmpeg("avcodec_parameters_alloc".into()));
            }
            let me = CodecParams { ptr };
            check(ff::avcodec_parameters_copy(ptr, src), "avcodec_parameters_copy")?;
            Ok(me)
        }
    }

    pub fn as_ptr(&self) -> *const ff::AVCodecParameters {
        self.ptr
    }

    pub fn codec_id(&self) -> ff::AVCodecID {
        // SAFETY: ptr ist gültig, solange self lebt.
        unsafe { (*self.ptr).codec_id }
    }

    pub fn is_video(&self) -> bool {
        // SAFETY: wie oben.
        unsafe { (*self.ptr).codec_type == ff::AVMediaType::AVMEDIA_TYPE_VIDEO }
    }

    pub fn dimensions(&self) -> (u32, u32) {
        // SAFETY: wie oben.
        unsafe { ((*self.ptr).width.max(0) as u32, (*self.ptr).height.max(0) as u32) }
    }
}

impl Drop for CodecParams {
    fn drop(&mut self) {
        // SAFETY: Gibt unsere eigene Kopie frei.
        unsafe { ff::avcodec_parameters_free(&mut self.ptr) };
    }
}

// SAFETY: Nach der Erstellung nur noch lesend benutzt.
unsafe impl Send for CodecParams {}
// SAFETY: Nur lesender Zugriff von mehreren Threads.
unsafe impl Sync for CodecParams {}

/// Stream-Beschreibung einer Pipeline: Video (Stream 0) + Audiospuren (1..).
#[derive(Debug)]
pub struct StreamSet {
    pub video: CodecParams,
    pub audio: Vec<CodecParams>,
    /// Zeitbasis aller Pakete: Mikrosekunden.
    pub fps: u32,
}

pub const US_TB: ff::AVRational = ff::AVRational { num: 1, den: 1_000_000 };

pub fn null_dict() -> *mut ff::AVDictionary {
    ptr::null_mut()
}
