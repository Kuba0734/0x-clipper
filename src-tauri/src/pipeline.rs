//! Eine laufende Aufnahme-Session: D3D11-Gerät, Capture-Thread, Encoder-Thread, Audio.
//! Wird bei Änderungen an Quelle/Video/Audio komplett neu gestartet.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::Duration;

use crossbeam_channel::{bounded, select, Receiver, Sender};

use crate::audio::{self, AudioControls, AudioPipeline};
use crate::buffer::Limits;
use crate::capture::d3d::D3dDevice;
use crate::capture::dxgi::DxgiSource;
use crate::capture::wgc::{self, WgcSource, WgcTarget};
use crate::capture::{self, CaptureLoop, CapturedFrame, FrameSource};
use crate::encode::ffi::OwnedFrame;
use crate::encode::ffmpeg::{packet_info, VideoEncoder, VideoEncoderConfig};
use crate::encode::probe::{self, EncoderInfo};
use crate::encode::StreamSet;
use crate::error::{CoreError, CoreResult};
use crate::router::PacketRouter;
use crate::settings::{CaptureBackend, CaptureSource, Resolution, Settings};
use crate::stats::Stats;
use crate::util;

/// Frames zwischen Capture und Encoder (bounded). Mehr als 3 bedeutet nur Latenz.
const FRAME_QUEUE: usize = 3;
/// Rahmen in Umlauf (Capture ↔ Encoder), kleiner als der d3d11-Pool.
const FRAME_SHELLS: usize = 6;

pub enum EncoderCmd {
    ForceKeyframe,
}

pub struct Pipeline {
    pub router: Arc<PacketRouter>,
    pub streams: Arc<StreamSet>,
    pub encoder: EncoderInfo,
    pub resolution: (u32, u32),
    pub source_label: String,
    stop: Arc<AtomicBool>,
    capture: Option<JoinHandle<()>>,
    encode: Option<JoinHandle<()>>,
    audio: Option<AudioPipeline>,
    enc_cmd: Sender<EncoderCmd>,
}

pub struct PipelineDeps {
    pub stats: Arc<Stats>,
    pub controls: Arc<AudioControls>,
    pub encoders: Vec<EncoderInfo>,
    /// Fehler aus Hintergrund-Threads (Quelle geschlossen, Encoder-Fehler).
    pub errors: Sender<String>,
    /// Monitor des erkannten Spiels (für CaptureSource::AutoGame).
    pub game_monitor: Option<String>,
}

pub fn buffer_limits(s: &Settings) -> Limits {
    if s.replay.enabled {
        Limits {
            max_duration_us: i64::from(s.replay.duration_secs) * 1_000_000,
            max_bytes: s.replay.ram_limit_mb as usize * 1024 * 1024,
        }
    } else {
        // Ohne Buffer nur die aktuelle GOP halten (Pakete für eine laufende Aufnahme).
        Limits { max_duration_us: 0, max_bytes: 8 * 1024 * 1024 }
    }
}

fn target_size(res: Resolution, native: (u32, u32)) -> (u32, u32) {
    let (nw, nh) = native;
    let fit_height = |h: u32| {
        let h = h.min(nh.max(2));
        let w = (u64::from(nw) * u64::from(h) / u64::from(nh.max(1))) as u32;
        (w, h)
    };
    let (w, h) = match res {
        Resolution::Native => native,
        Resolution::P1440 => fit_height(1440),
        Resolution::P1080 => fit_height(1080),
        Resolution::P720 => fit_height(720),
        Resolution::Custom { width, height } => (width, height),
    };
    (w.max(2) & !1, h.max(2) & !1)
}

enum Backend {
    Wgc,
    Dxgi,
}

impl Pipeline {
    pub fn start(settings: &Settings, deps: PipelineDeps) -> CoreResult<Pipeline> {
        let started = std::time::Instant::now();
        let cap = &settings.capture;
        let backend = match cap.backend {
            CaptureBackend::Dxgi => Backend::Dxgi,
            CaptureBackend::Wgc => Backend::Wgc,
            CaptureBackend::Auto if wgc::is_supported() => Backend::Wgc,
            CaptureBackend::Auto => Backend::Dxgi,
        };
        // Quelle auflösen.
        let (monitor, window, label) = match &cap.source {
            CaptureSource::Monitor { device } => {
                (Some(capture::find_monitor(device)), None, format!("Monitor {}", short_device(device)))
            }
            CaptureSource::AutoGame => {
                let dev = deps.game_monitor.clone().unwrap_or_default();
                (Some(capture::find_monitor(&dev)), None, format!("Auto · {}", short_device(&dev)))
            }
            CaptureSource::Window { exe, title } => {
                let h = capture::find_window(exe, title)
                    .ok_or_else(|| CoreError::Capture(format!("Fenster {exe} nicht gefunden")))?;
                (None, Some(h), format!("Fenster {title}"))
            }
        };
        let (dev, source): (Arc<D3dDevice>, Box<dyn FrameSource>) = match (backend, monitor, window) {
            (Backend::Dxgi, Some(m), _) => {
                let (dev, output) = D3dDevice::for_monitor(m)?;
                let src = DxgiSource::new(&dev, &output)?;
                (Arc::new(dev), Box::new(src))
            }
            (Backend::Dxgi, None, _) => {
                return Err(CoreError::Capture("DXGI kann keine Fenster aufnehmen".into()));
            }
            (Backend::Wgc, m, w) => {
                let dev = D3dDevice::high_performance()?;
                let target = match (m, w) {
                    (_, Some(h)) => WgcTarget::Window(h),
                    (Some(m), None) => WgcTarget::Monitor(m),
                    (None, None) => return Err(CoreError::Capture("Keine Quelle".into())),
                };
                let src = WgcSource::new(&dev, target, cap.capture_cursor, cap.hide_border, settings.video.fps)?;
                (Arc::new(dev), Box::new(src))
            }
        };

        let encoder = probe::choose(&deps.encoders, &settings.video.encoder, settings.video.codec)
            .ok_or_else(|| CoreError::Encoder("Kein Encoder verfügbar".into()))?;
        let resolution = target_size(settings.video.resolution, source.size());
        let v = &settings.video;
        let cfg = VideoEncoderConfig {
            encoder: encoder.id.clone(),
            width: resolution.0,
            height: resolution.1,
            fps: v.fps,
            rate_control: v.rate_control,
            bitrate_kbps: v.bitrate_kbps,
            cqp: v.cqp,
            keyint_secs: v.keyframe_interval_secs,
            preset: v.preset,
        };
        let enc = VideoEncoder::new(&dev, &cfg)?;
        let video_params = enc.params()?;
        let router = Arc::new(PacketRouter::new(buffer_limits(settings), deps.stats.clone()));
        let (audio, audio_params) =
            audio::start(&settings.audio, deps.controls.clone(), router.clone(), deps.errors.clone())?;
        let streams = Arc::new(StreamSet { video: video_params, audio: audio_params, fps: v.fps });

        let stop = Arc::new(AtomicBool::new(false));
        let (frame_tx, frame_rx) = bounded::<CapturedFrame>(FRAME_QUEUE);
        let (free_tx, free_rx) = bounded::<OwnedFrame>(FRAME_SHELLS);
        for _ in 0..FRAME_SHELLS {
            let _ = free_tx.try_send(OwnedFrame::alloc()?);
        }
        let (enc_cmd, enc_cmd_rx) = bounded::<EncoderCmd>(4);

        let capture_loop = CaptureLoop {
            fps: v.fps,
            out_size: resolution,
            frames: enc.frames(),
            tx: frame_tx,
            free_rx,
            stop: stop.clone(),
            stats: deps.stats.clone(),
        };
        let errors = deps.errors.clone();
        let cap_dev = dev.clone();
        let capture = util::spawn("capture", util::Priority::AboveNormal, move || {
            if let Err(e) = capture_loop.run(cap_dev, source) {
                tracing::error!("Capture: {e}");
                let _ = errors.try_send(e.to_string());
            }
        })?;
        let errors = deps.errors.clone();
        let r2 = router.clone();
        let stop2 = stop.clone();
        let encode = util::spawn("encoder", util::Priority::AboveNormal, move || {
            // Das Gerät muss leben, solange der Encoder läuft.
            let _dev = dev;
            if let Err(e) = encode_loop(enc, frame_rx, free_tx, enc_cmd_rx, r2, stop2) {
                tracing::error!("Encoder: {e}");
                let _ = errors.try_send(e.to_string());
            }
        })?;
        tracing::info!(
            "Pipeline gestartet in {} ms: {} {}x{} @{} ({label})",
            started.elapsed().as_millis(),
            encoder.id,
            resolution.0,
            resolution.1,
            v.fps
        );
        Ok(Pipeline {
            router,
            streams,
            encoder,
            resolution,
            source_label: label,
            stop,
            capture: Some(capture),
            encode: Some(encode),
            audio: Some(audio),
            enc_cmd,
        })
    }

    pub fn force_keyframe(&self) {
        let _ = self.enc_cmd.try_send(EncoderCmd::ForceKeyframe);
    }

    pub fn stop(mut self) {
        self.shutdown();
    }

    fn shutdown(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(a) = self.audio.take() {
            a.stop();
        }
        if let Some(c) = self.capture.take() {
            let _ = c.join();
        }
        if let Some(e) = self.encode.take() {
            let _ = e.join();
        }
    }
}

impl Drop for Pipeline {
    fn drop(&mut self) {
        self.shutdown();
    }
}

fn short_device(d: &str) -> String {
    let s = d.trim_start_matches(r"\\.\");
    if s.is_empty() { "primär".into() } else { s.to_string() }
}

fn encode_loop(
    mut enc: VideoEncoder,
    frames: Receiver<CapturedFrame>,
    free_tx: Sender<OwnedFrame>,
    cmds: Receiver<EncoderCmd>,
    router: Arc<PacketRouter>,
    stop: Arc<AtomicBool>,
) -> CoreResult<()> {
    let mut sink = |p: &crate::encode::ffi::OwnedPacket, frame_us: i64| {
        let (pts, dts, dur, key) = packet_info(p);
        let dur = if dur > 0 { dur } else { frame_us };
        router.route(0, pts, dts, dur, key, p.data());
    };
    while !stop.load(Ordering::Relaxed) {
        select! {
            recv(frames) -> msg => {
                let Ok(mut cf) = msg else { break };
                enc.encode(&mut cf.frame, cf.pts_us, &mut sink)?;
                let _ = free_tx.try_send(cf.frame);
            }
            recv(cmds) -> cmd => {
                if let Ok(EncoderCmd::ForceKeyframe) = cmd {
                    enc.request_keyframe();
                }
            }
            default(Duration::from_millis(250)) => {}
        }
    }
    enc.flush(&mut sink)
}
