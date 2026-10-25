//! Audio-Pipeline: WASAPI-Quellen → Engine (Lautstärke, Stummschaltung, Mischen) → AAC.
//!
//! Spuren: getrennt (Stream 1 = System, Stream 2 = Mikrofon) oder eine gemischte Spur.
//! Zeitbasis ist die QPC-Uhr; Lücken werden mit Stille aufgefüllt, Vorlauf verworfen,
//! damit Audio und Video synchron bleiben.

pub mod aac;
pub mod wasapi;

use std::collections::VecDeque;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use crossbeam_channel::{bounded, RecvTimeoutError, Sender};

use crate::encode::CodecParams;
use crate::error::CoreResult;
use crate::router::PacketRouter;
use crate::settings::{AudioSettings, TrackMode};
use crate::util;
use aac::AacEncoder;
use wasapi::{AudioChunk, CaptureParams, SourceKind, CHANNELS, SAMPLE_RATE};

/// Live änderbare Steuerung (Hotkeys, Einstellungen) ohne Neustart der Pipeline.
#[derive(Debug, Default)]
pub struct AudioControls {
    pub mic_muted: AtomicBool,
    pub ptt_enabled: AtomicBool,
    pub ptt_down: AtomicBool,
    system_volume: AtomicU32,
    mic_volume: AtomicU32,
}

impl AudioControls {
    pub fn new() -> Self {
        let c = AudioControls::default();
        c.set_volumes(1.0, 1.0);
        c
    }

    pub fn set_volumes(&self, system: f32, mic: f32) {
        self.system_volume.store(system.to_bits(), Ordering::Relaxed);
        self.mic_volume.store(mic.to_bits(), Ordering::Relaxed);
    }

    fn gain(&self, s: SourceKind) -> f32 {
        match s {
            SourceKind::Loopback => f32::from_bits(self.system_volume.load(Ordering::Relaxed)),
            SourceKind::Mic => {
                let muted = self.mic_muted.load(Ordering::Relaxed);
                let ptt_blocked = self.ptt_enabled.load(Ordering::Relaxed)
                    && !self.ptt_down.load(Ordering::Relaxed);
                if muted || ptt_blocked {
                    0.0
                } else {
                    f32::from_bits(self.mic_volume.load(Ordering::Relaxed))
                }
            }
        }
    }
}

const US_PER_FRAME: f64 = 1_000_000.0 / SAMPLE_RATE as f64;
/// Lücken > 40 ms werden mit Stille gefüllt.
const GAP_US: i64 = 40_000;
/// Liegt eine Quelle > 100 ms vor der Uhr, wird verworfen.
const AHEAD_US: i64 = 100_000;

/// Sample-FIFO einer Quelle mit eigener Zeitachse.
struct Fifo {
    kind: SourceKind,
    buf: VecDeque<f32>,
    base_us: Option<i64>,
    /// Bereits entnommene Frames (Stereo-Paare).
    consumed: i64,
}

impl Fifo {
    fn new(kind: SourceKind) -> Self {
        Fifo { kind, buf: VecDeque::with_capacity(SAMPLE_RATE as usize * CHANNELS), base_us: None, consumed: 0 }
    }

    fn frames(&self) -> usize {
        self.buf.len() / CHANNELS
    }

    /// Zeit des nächsten zu entnehmenden Frames.
    fn head_us(&self) -> Option<i64> {
        self.base_us.map(|b| b + (self.consumed as f64 * US_PER_FRAME) as i64)
    }

    fn push(&mut self, chunk: &AudioChunk) {
        let mut data: &[f32] = &chunk.data;
        match self.base_us {
            None => self.base_us = Some(chunk.ts_us),
            Some(base) => {
                let expected = base + ((self.consumed + self.frames() as i64) as f64 * US_PER_FRAME) as i64;
                let diff = chunk.ts_us - expected;
                if diff > GAP_US {
                    let fill = ((diff.min(1_000_000) as f64) / US_PER_FRAME) as usize;
                    self.buf.extend(std::iter::repeat_n(0.0, fill * CHANNELS));
                } else if diff < -AHEAD_US {
                    let skip = ((-diff as f64 / US_PER_FRAME) as usize * CHANNELS).min(data.len());
                    data = &data[skip..];
                }
            }
        }
        // Puffer nie unbegrenzt wachsen lassen (2 s).
        let max = SAMPLE_RATE as usize * CHANNELS * 2;
        self.buf.extend(data.iter().copied());
        while self.buf.len() > max {
            self.buf.drain(..CHANNELS * 1024);
            self.consumed += 1024;
        }
    }

    /// Entnimmt `n` Frames mit Gain und addiert sie (oder schreibt sie) in `out`.
    fn take_into(&mut self, n: usize, gain: f32, out: &mut [f32], add: bool) {
        let avail = self.frames().min(n);
        for (i, v) in self.buf.drain(..avail * CHANNELS).enumerate() {
            if add {
                out[i] += v * gain;
            } else {
                out[i] = v * gain;
            }
        }
        if !add {
            for o in &mut out[avail * CHANNELS..n * CHANNELS] {
                *o = 0.0;
            }
        }
        self.consumed += n as i64;
    }

    fn drop_frames(&mut self, n: usize) {
        let n = n.min(self.frames());
        self.buf.drain(..n * CHANNELS);
        self.consumed += n as i64;
    }
}

struct Track {
    enc: AacEncoder,
    stream: u8,
    base_us: Option<i64>,
    samples: i64,
}

impl Track {
    fn encode(&mut self, data: &[f32], head_us: i64, router: &PacketRouter) -> CoreResult<()> {
        let base = *self.base_us.get_or_insert(head_us);
        let stream = self.stream;
        let pts = self.samples;
        self.samples += self.enc.frame_size as i64;
        let to_us = |s: i64| base + (s as f64 * US_PER_FRAME) as i64;
        self.enc.encode(data, pts, &mut |p, d, dur, bytes| {
            router.route(stream, to_us(p), to_us(d), (dur as f64 * US_PER_FRAME) as i64, true, bytes);
        })
    }
}

pub struct AudioPipeline {
    stop: Arc<AtomicBool>,
    threads: Vec<JoinHandle<()>>,
}

impl AudioPipeline {
    pub fn stop(mut self) {
        self.stop.store(true, Ordering::Relaxed);
        for t in self.threads.drain(..) {
            let _ = t.join();
        }
    }
}

impl Drop for AudioPipeline {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
    }
}

/// Startet Quellen und Engine. Gibt die Codec-Parameter der Spuren (Stream 1..) zurück.
pub fn start(
    cfg: &AudioSettings,
    controls: Arc<AudioControls>,
    router: Arc<PacketRouter>,
    errors: Sender<String>,
) -> CoreResult<(AudioPipeline, Vec<CodecParams>)> {
    let stop = Arc::new(AtomicBool::new(false));
    let mut sources: Vec<(SourceKind, String)> = Vec::new();
    if cfg.system.enabled {
        sources.push((SourceKind::Loopback, cfg.system.device_id.clone()));
    }
    if cfg.mic.enabled {
        sources.push((SourceKind::Mic, cfg.mic.device_id.clone()));
    }
    controls.set_volumes(cfg.system.volume, cfg.mic.volume);
    controls.ptt_enabled.store(cfg.push_to_talk, Ordering::Relaxed);
    if sources.is_empty() {
        return Ok((AudioPipeline { stop, threads: Vec::new() }, Vec::new()));
    }

    let mut tracks = Vec::new();
    let track_count = if cfg.track_mode == TrackMode::Mixed { 1 } else { sources.len() };
    for i in 0..track_count {
        tracks.push(Track { enc: AacEncoder::new(cfg.bitrate_kbps)?, stream: i as u8 + 1, base_us: None, samples: 0 });
    }
    let params = tracks.iter().map(|t| t.enc.params()).collect::<CoreResult<Vec<_>>>()?;

    let (tx, rx) = bounded::<AudioChunk>(256);
    let mut threads = Vec::new();
    let mut free_txs: Vec<(SourceKind, Sender<Vec<f32>>)> = Vec::new();
    for (kind, device_id) in &sources {
        let kind = *kind;
        let (free_tx, free_rx) = bounded::<Vec<f32>>(256);
        free_txs.push((kind, free_tx));
        let p = CaptureParams { kind, device_id: device_id.clone(), tx: tx.clone(), free_rx, stop: stop.clone() };
        let errors = errors.clone();
        threads.push(util::spawn("audio-capture", util::Priority::AboveNormal, move || {
            if let Err(e) = wasapi::run_capture(p) {
                tracing::error!("Audioquelle {kind:?}: {e}");
                let _ = errors.try_send(format!("Audio ({kind:?}): {e}"));
            }
        })?);
    }
    drop(tx);
    let mode = cfg.track_mode;
    let kinds: Vec<SourceKind> = sources.iter().map(|s| s.0).collect();
    let stop2 = stop.clone();
    threads.push(util::spawn("audio-engine", util::Priority::AboveNormal, move || {
        let mut engine = Engine::new(kinds, tracks, mode, controls, router);
        while !stop2.load(Ordering::Relaxed) {
            match rx.recv_timeout(Duration::from_millis(200)) {
                Ok(chunk) => {
                    if let Err(e) = engine.push(&chunk) {
                        tracing::error!("Audio-Encoder: {e}");
                    }
                    if let Some((_, f)) = free_txs.iter().find(|(k, _)| *k == chunk.source) {
                        let _ = f.try_send(chunk.data);
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => break,
            }
        }
    })?);
    Ok((AudioPipeline { stop, threads }, params))
}

struct Engine {
    fifos: Vec<Fifo>,
    tracks: Vec<Track>,
    mode: TrackMode,
    controls: Arc<AudioControls>,
    router: Arc<PacketRouter>,
    scratch: Vec<f32>,
}

impl Engine {
    fn new(
        kinds: Vec<SourceKind>,
        tracks: Vec<Track>,
        mode: TrackMode,
        controls: Arc<AudioControls>,
        router: Arc<PacketRouter>,
    ) -> Self {
        let n = tracks.first().map(|t| t.enc.frame_size).unwrap_or(1024);
        Engine {
            fifos: kinds.into_iter().map(Fifo::new).collect(),
            tracks,
            mode,
            controls,
            router,
            scratch: vec![0.0; n * CHANNELS],
        }
    }

    fn push(&mut self, chunk: &AudioChunk) -> CoreResult<()> {
        let Some(idx) = self.fifos.iter().position(|f| f.kind == chunk.source) else { return Ok(()) };
        self.fifos[idx].push(chunk);
        let n = self.scratch.len() / CHANNELS;
        match self.mode {
            TrackMode::Separate => {
                let f = &mut self.fifos[idx];
                let t = &mut self.tracks[idx];
                while f.frames() >= n {
                    let head = f.head_us().unwrap_or(0);
                    let gain = self.controls.gain(f.kind);
                    f.take_into(n, gain, &mut self.scratch, false);
                    t.encode(&self.scratch, head, &self.router)?;
                }
            }
            TrackMode::Mixed => {
                // Erste Quelle (System, falls aktiv) ist die Uhr; die zweite wird zugemischt.
                let (master, rest) = self.fifos.split_at_mut(1);
                let m = &mut master[0];
                let Some(t) = self.tracks.first_mut() else { return Ok(()) };
                while m.frames() >= n {
                    let head = m.head_us().unwrap_or(0);
                    m.take_into(n, self.controls.gain(m.kind), &mut self.scratch, false);
                    if let Some(s) = rest.first_mut() {
                        if let Some(sh) = s.head_us() {
                            // Nebenquelle zeitlich ausrichten.
                            let lag = head - sh;
                            if lag > 20_000 {
                                s.drop_frames((lag as f64 / US_PER_FRAME) as usize);
                            }
                            let sh = s.head_us().unwrap_or(sh);
                            if sh <= head + 20_000 {
                                s.take_into(n, self.controls.gain(s.kind), &mut self.scratch, true);
                            }
                        }
                    }
                    for v in &mut self.scratch {
                        *v = v.clamp(-1.0, 1.0);
                    }
                    t.encode(&self.scratch, head, &self.router)?;
                }
            }
        }
        Ok(())
    }
}
