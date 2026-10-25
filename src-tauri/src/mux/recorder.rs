//! Manuelle Aufnahme: schreibt Pakete live in MP4, unabhängig vom Replay-Buffer.
//! Optional mit Split nach Dateigröße und maximaler Dauer.

use std::path::PathBuf;
use std::sync::Arc;
use std::thread::JoinHandle;

use crossbeam_channel::{bounded, Receiver, Sender};

use super::mp4::Mp4Writer;
use crate::buffer::{Packet, VIDEO_STREAM};
use crate::encode::StreamSet;
use crate::error::CoreResult;
use crate::util;

pub struct RecorderConfig {
    pub split_bytes: u64,
    pub max_duration_us: i64,
}

/// Rückmeldungen an den Controller.
pub enum RecorderEvent {
    FileDone { path: PathBuf, secs: f64 },
    /// Maximale Dauer erreicht – der Controller beendet die Aufnahme.
    LimitReached,
    Error(String),
}

pub struct Recorder {
    pub tx: Sender<Arc<Packet>>,
    join: JoinHandle<()>,
}

/// Kapazität für ~10 s Pakete, falls die Platte kurz hängt.
const QUEUE: usize = 4096;

impl Recorder {
    pub fn start(
        streams: Arc<StreamSet>,
        cfg: RecorderConfig,
        next_path: Box<dyn Fn() -> CoreResult<PathBuf> + Send>,
        events: Sender<RecorderEvent>,
    ) -> CoreResult<Self> {
        let (tx, rx) = bounded::<Arc<Packet>>(QUEUE);
        let join = util::spawn("recorder", util::Priority::BelowNormal, move || {
            if let Err(e) = run(rx, &streams, &cfg, &*next_path, &events) {
                tracing::error!("Aufnahme: {e}");
                let _ = events.send(RecorderEvent::Error(e.to_string()));
            }
        })?;
        Ok(Recorder { tx, join })
    }

    /// Beendet die Aufnahme: Sender schließen, restliche Pakete schreiben, Datei finalisieren.
    pub fn finish(self) {
        drop(self.tx);
        let _ = self.join.join();
    }
}

fn run(
    rx: Receiver<Arc<Packet>>,
    streams: &StreamSet,
    cfg: &RecorderConfig,
    next_path: &dyn Fn() -> CoreResult<PathBuf>,
    events: &Sender<RecorderEvent>,
) -> CoreResult<()> {
    let mut writer: Option<Mp4Writer> = None;
    let mut session_start: Option<i64> = None;
    let mut limit_sent = false;
    // Läuft, bis der Router den Sender abhängt (Kanal leer + getrennt).
    for p in rx.iter() {
        let is_key = p.stream == VIDEO_STREAM && p.key;
        if writer.is_none() {
            // Datei beginnt immer mit einem Video-Keyframe.
            if !is_key {
                continue;
            }
            session_start.get_or_insert(p.pts);
            writer = Some(Mp4Writer::create(&next_path()?, streams, p.pts)?);
        }
        let start = session_start.unwrap_or(p.pts);
        if cfg.max_duration_us > 0 && p.pts - start >= cfg.max_duration_us {
            if !limit_sent {
                limit_sent = true;
                let _ = events.send(RecorderEvent::LimitReached);
            }
            continue;
        }
        if is_key && cfg.split_bytes > 0 && writer.as_ref().is_some_and(|w| w.bytes >= cfg.split_bytes) {
            if let Some(w) = writer.take() {
                let secs = w.duration_secs();
                let path = w.finish()?;
                let _ = events.send(RecorderEvent::FileDone { path, secs });
            }
            writer = Some(Mp4Writer::create(&next_path()?, streams, p.pts)?);
        }
        if let Some(w) = writer.as_mut() {
            // Audio vor dem ersten Keyframe der Datei verwerfen.
            if p.stream != VIDEO_STREAM && p.pts < w.base_us() {
                continue;
            }
            w.write(&p)?;
        }
    }
    if let Some(w) = writer.take() {
        let secs = w.duration_secs();
        let path = w.finish()?;
        let _ = events.send(RecorderEvent::FileDone { path, secs });
    }
    Ok(())
}
