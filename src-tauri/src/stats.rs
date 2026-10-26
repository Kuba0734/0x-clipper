//! Zähler des heißen Pfads (nur Atomics, kein Lock) und der UI-Status.

use std::sync::atomic::{AtomicI64, AtomicU64};

use serde::Serialize;
use ts_rs::TS;

#[derive(Debug, Default)]
pub struct Stats {
    pub captured_frames: AtomicU64,
    pub encoded_frames: AtomicU64,
    pub dropped_frames: AtomicU64,
    pub recorder_dropped: AtomicU64,
    pub video_bytes: AtomicU64,
    pub buffer_bytes: AtomicU64,
    pub buffered_us: AtomicI64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CoreState {
    Idle,
    Starting,
    Buffering,
    Recording,
    Error,
}

#[derive(Debug, Clone, Serialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub struct Status {
    pub state: CoreState,
    pub buffer_enabled: bool,
    pub recording: bool,
    /// Sekunden seit Aufnahmestart.
    pub recording_secs: f64,
    /// Gefüllte Sekunden im Replay-Buffer.
    pub buffered_secs: f64,
    pub buffer_target_secs: u32,
    #[ts(type = "number")]
    pub buffer_bytes: u64,
    #[ts(type = "number")]
    pub buffer_limit_bytes: u64,
    /// Video-Bitrate der letzten Messperiode.
    pub bitrate_kbps: u32,
    pub fps: f32,
    #[ts(type = "number")]
    pub dropped_frames: u64,
    pub encoder: String,
    pub resolution: String,
    pub source: String,
    pub game: Option<String>,
    pub mic_muted: bool,
    /// Arbeitsspeicher des gesamten Prozesses (Private Bytes).
    #[ts(type = "number")]
    pub process_ram_bytes: u64,
    pub last_error: Option<String>,
}
