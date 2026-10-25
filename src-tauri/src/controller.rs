//! Controller-State-Machine. Läuft auf einem eigenen Thread und besitzt die Pipeline.
//!
//! ```text
//! Idle ──enable──▶ Buffering ──record──▶ Recording
//!   ▲                 │  ▲                   │
//!   └──disable────────┘  └──────record───────┘
//! Buffering ──clip──▶ (Snapshot → MuxWorker) ──▶ Buffering
//! ```
//! Während `Recording` läuft der Replay-Buffer weiter (wenn aktiviert).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crossbeam_channel::{bounded, select, Receiver, Sender};
use parking_lot::RwLock;
use tauri::{AppHandle, Emitter};

use crate::audio::AudioControls;
use crate::capture::d3d::D3dDevice;
use crate::capture::game::{self, SharedGame};
use crate::encode::probe::{self, EncoderInfo};
use crate::error::{CoreError, CoreResult, ErrorPayload};
use crate::events;
use crate::library::Library;
use crate::mux::mp4::write_snapshot;
use crate::mux::recorder::{Recorder, RecorderConfig, RecorderEvent};
use crate::mux::MuxWorker;
use crate::overlay::{Overlay, ToastKind};
use crate::paths::{self, ClipKind};
use crate::pipeline::{buffer_limits, Pipeline, PipelineDeps};
use crate::settings::{self, CaptureSource, Settings};
use crate::sound::{Sound, SoundKind};
use crate::stats::{CoreState, Stats, Status};

pub enum Cmd {
    SetBuffer(bool),
    ToggleBuffer,
    SaveClip(Instant),
    ToggleRecording,
    ToggleMicMute,
    ApplySettings(Box<Settings>),
    GameChanged,
    Restart,
    Shutdown(Sender<()>),
}

/// Von Commands und Status-Emitter lesbarer Zustand (ohne den Controller-Thread zu blockieren).
pub struct Shared {
    pub settings: RwLock<Settings>,
    pub encoders: RwLock<Vec<EncoderInfo>>,
    pub stats: Arc<Stats>,
    pub controls: Arc<AudioControls>,
    pub game: SharedGame,
    pub library: Library,
    pub mux: MuxWorker,
    info: RwLock<RunInfo>,
}

#[derive(Default, Clone)]
struct RunInfo {
    state: Option<CoreState>,
    buffer_enabled: bool,
    recording_since: Option<Instant>,
    encoder: String,
    resolution: String,
    source: String,
    last_error: Option<String>,
}

#[derive(Clone)]
pub struct Core {
    pub tx: Sender<Cmd>,
    pub shared: Arc<Shared>,
}

impl Core {
    pub fn send(&self, cmd: Cmd) {
        if self.tx.try_send(cmd).is_err() {
            tracing::warn!("Controller-Warteschlange voll");
        }
    }

    pub fn status(&self) -> Status {
        let s = &self.shared;
        let info = s.info.read().clone();
        let settings = s.settings.read();
        let game = s.game.read().as_ref().map(|g| g.name.clone());
        Status {
            state: info.state.unwrap_or(CoreState::Idle),
            buffer_enabled: info.buffer_enabled,
            recording: info.recording_since.is_some(),
            recording_secs: info.recording_since.map(|t| t.elapsed().as_secs_f64()).unwrap_or(0.0),
            buffered_secs: s.stats.buffered_us.load(Ordering::Relaxed) as f64 / 1e6,
            buffer_target_secs: settings.replay.duration_secs,
            buffer_bytes: s.stats.buffer_bytes.load(Ordering::Relaxed),
            buffer_limit_bytes: u64::from(settings.replay.ram_limit_mb) * 1024 * 1024,
            bitrate_kbps: 0,
            fps: 0.0,
            dropped_frames: s.stats.dropped_frames.load(Ordering::Relaxed),
            encoder: info.encoder,
            resolution: info.resolution,
            source: info.source,
            game,
            mic_muted: s.controls.mic_muted.load(Ordering::Relaxed),
            process_ram_bytes: crate::sysinfo::private_bytes(),
            last_error: info.last_error,
        }
    }

    pub fn output_dir(&self) -> PathBuf {
        paths::output_dir(&self.shared.settings.read())
    }
}

struct Controller {
    app: AppHandle,
    shared: Arc<Shared>,
    pipeline: Option<Pipeline>,
    recorder: Option<Recorder>,
    rec_events: (Sender<RecorderEvent>, Receiver<RecorderEvent>),
    errors: (Sender<String>, Receiver<String>),
    buffer_enabled: bool,
    overlay: Overlay,
    sound: Sound,
    self_tx: Sender<Cmd>,
    restart_pending: Arc<AtomicBool>,
    pipeline_monitor: Option<String>,
}

pub fn start(app: AppHandle, initial: Settings) -> CoreResult<Core> {
    let (tx, rx) = bounded::<Cmd>(64);
    let stats = Arc::new(Stats::default());
    let controls = Arc::new(AudioControls::new());
    let game: SharedGame = Arc::new(RwLock::new(None));
    let shared = Arc::new(Shared {
        settings: RwLock::new(initial.clone()),
        encoders: RwLock::new(Vec::new()),
        stats,
        controls,
        game: game.clone(),
        library: Library::default(),
        mux: MuxWorker::start()?,
        info: RwLock::new(RunInfo { buffer_enabled: initial.replay.enabled, ..Default::default() }),
    });
    // Spielerkennung meldet Wechsel direkt an den Controller.
    let (game_tx, game_rx) = bounded::<()>(1);
    game::start(game, game_tx);
    let gtx = tx.clone();
    crate::util::spawn("game-forward", crate::util::Priority::BelowNormal, move || {
        for () in game_rx.iter() {
            let _ = gtx.try_send(Cmd::GameChanged);
        }
    })?;

    let core = Core { tx: tx.clone(), shared: shared.clone() };
    let mut c = Controller {
        app,
        shared,
        pipeline: None,
        recorder: None,
        rec_events: bounded(16),
        errors: bounded(16),
        buffer_enabled: initial.replay.enabled,
        overlay: Overlay::start(),
        sound: Sound::new(initial.sound.volume),
        self_tx: tx,
        restart_pending: Arc::new(AtomicBool::new(false)),
        pipeline_monitor: None,
    };
    crate::util::spawn("controller", crate::util::Priority::Normal, move || {
        crate::util::com_init();
        c.init_encoders();
        if c.buffer_enabled {
            c.ensure_pipeline();
        }
        c.publish();
        c.run(rx);
    })?;
    Ok(core)
}

impl Controller {
    fn settings(&self) -> Settings {
        self.shared.settings.read().clone()
    }

    fn init_encoders(&mut self) {
        let t = Instant::now();
        let list = match D3dDevice::high_performance() {
            Ok(dev) => probe::cached(&dev).unwrap_or_else(|| probe::probe(&dev)),
            Err(e) => {
                tracing::error!("Kein D3D11-Gerät: {e}");
                Vec::new()
            }
        };
        tracing::info!("Encoder-Liste in {} ms", t.elapsed().as_millis());
        *self.shared.encoders.write() = list;
        let _ = self.app.emit(events::ENCODERS, self.shared.encoders.read().clone());
    }

    fn run(&mut self, rx: Receiver<Cmd>) {
        let rec_rx = self.rec_events.1.clone();
        let err_rx = self.errors.1.clone();
        loop {
            select! {
                recv(rx) -> cmd => match cmd {
                    Ok(Cmd::Shutdown(done)) => {
                        self.stop_recording(false);
                        self.stop_pipeline();
                        let _ = done.send(());
                        return;
                    }
                    Ok(cmd) => self.handle(cmd),
                    Err(_) => return,
                },
                recv(rec_rx) -> ev => if let Ok(ev) = ev { self.on_recorder(ev) },
                recv(err_rx) -> e => if let Ok(e) = e { self.on_pipeline_error(e) },
            }
            self.publish();
        }
    }

    fn handle(&mut self, cmd: Cmd) {
        match cmd {
            Cmd::SetBuffer(on) => self.set_buffer(on),
            Cmd::ToggleBuffer => self.set_buffer(!self.buffer_enabled),
            Cmd::SaveClip(t) => self.save_clip(t),
            Cmd::ToggleRecording => {
                if self.recorder.is_some() {
                    self.stop_recording(true);
                } else {
                    self.start_recording();
                }
            }
            Cmd::ToggleMicMute => {
                let c = &self.shared.controls;
                let muted = !c.mic_muted.load(Ordering::Relaxed);
                c.mic_muted.store(muted, Ordering::Relaxed);
                let s = self.settings();
                self.toast(if muted { ToastKind::MicMuted } else { ToastKind::MicUnmuted }, String::new(), &s);
            }
            Cmd::ApplySettings(new) => self.apply_settings(*new),
            Cmd::GameChanged => self.on_game_changed(),
            Cmd::Restart => {
                self.restart_pending.store(false, Ordering::Relaxed);
                if self.buffer_enabled || self.recorder.is_some() {
                    self.stop_pipeline();
                    self.ensure_pipeline();
                }
            }
            Cmd::Shutdown(_) => {}
        }
    }

    fn deps(&self) -> PipelineDeps {
        PipelineDeps {
            stats: self.shared.stats.clone(),
            controls: self.shared.controls.clone(),
            encoders: self.shared.encoders.read().clone(),
            errors: self.errors.0.clone(),
            game_monitor: self.shared.game.read().as_ref().map(|g| g.monitor.clone()),
        }
    }

    fn ensure_pipeline(&mut self) -> bool {
        if self.pipeline.is_some() {
            return true;
        }
        self.set_state(CoreState::Starting);
        let s = self.settings();
        let deps = self.deps();
        let monitor = deps.game_monitor.clone();
        match Pipeline::start(&s, deps) {
            Ok(p) => {
                {
                    let mut info = self.shared.info.write();
                    info.encoder = p.encoder.id.clone();
                    info.resolution = format!("{}×{}", p.resolution.0, p.resolution.1);
                    info.source = p.source_label.clone();
                    info.last_error = None;
                }
                self.pipeline_monitor = monitor;
                self.pipeline = Some(p);
                true
            }
            Err(e) => {
                self.report_error(&e);
                self.schedule_restart();
                false
            }
        }
    }

    fn stop_pipeline(&mut self) {
        if let Some(p) = self.pipeline.take() {
            p.stop();
        }
        self.shared.stats.buffered_us.store(0, Ordering::Relaxed);
        self.shared.stats.buffer_bytes.store(0, Ordering::Relaxed);
    }

    fn set_buffer(&mut self, on: bool) {
        self.buffer_enabled = on;
        {
            let mut s = self.shared.settings.write();
            s.replay.enabled = on;
            if let Err(e) = settings::save(&s) {
                tracing::warn!("Einstellungen nicht gespeichert: {e}");
            }
        }
        let s = self.settings();
        if on {
            if let Some(p) = &self.pipeline {
                p.router.set_limits(buffer_limits(&s));
            } else {
                self.ensure_pipeline();
            }
            self.toast(ToastKind::BufferOn, format!("{} s", s.replay.duration_secs), &s);
        } else {
            if self.recorder.is_none() {
                self.stop_pipeline();
            } else if let Some(p) = &self.pipeline {
                p.router.set_limits(buffer_limits(&s));
                p.router.clear();
            }
            self.toast(ToastKind::BufferOff, String::new(), &s);
        }
        let _ = self.app.emit(events::SETTINGS, &s);
        crate::tray::refresh(&self.app);
    }

    fn game_name(&self) -> String {
        self.shared.game.read().as_ref().map(|g| g.name.clone()).unwrap_or_else(|| "Desktop".into())
    }

    fn save_clip(&mut self, requested: Instant) {
        let s = self.settings();
        if !self.buffer_enabled {
            self.toast(ToastKind::Error, tr(&s, "Replay-Buffer ist aus", "Replay buffer is off"), &s);
            self.sound.play(SoundKind::Error, &s.sound);
            return;
        }
        let Some(p) = &self.pipeline else {
            self.report_error(&CoreError::NotRunning);
            return;
        };
        let Some(snap) = p.router.snapshot(i64::from(s.replay.duration_secs) * 1_000_000) else {
            self.toast(ToastKind::Error, tr(&s, "Buffer ist noch leer", "Buffer is still empty"), &s);
            return;
        };
        let streams = p.streams.clone();
        let game = self.game_name();
        let path = match paths::new_output_file(&s, &game, ClipKind::Clip) {
            Ok(p) => p,
            Err(e) => return self.report_error(&e),
        };
        let app = self.app.clone();
        let shared = self.shared.clone();
        let overlay = self.overlay.clone();
        let sound = self.sound.clone();
        let mux = self.shared.mux.clone();
        let s_job = s.clone();
        let accepted = mux.submit(move || {
            let s = s_job;
            let res = write_snapshot(&path, &streams, &snap);
            drop(snap);
            match res {
                Ok(secs) => {
                    tracing::info!(
                        "Clip gespeichert ({secs:.1} s) in {} ms: {}",
                        requested.elapsed().as_millis(),
                        path.display()
                    );
                    overlay.show(ToastKind::ClipSaved, format!("{secs:.0} s · {game}"), &s);
                    sound.play(SoundKind::Saved, &s.sound);
                    let _ = Library::ensure_thumbnail(&path);
                    let root = paths::output_dir(&s);
                    if let Ok(info) = shared.library.info(&root, &path) {
                        let _ = app.emit(events::CLIP_SAVED, info);
                    }
                    after_save_housekeeping(&app, &shared, &s);
                }
                Err(e) => {
                    tracing::error!("Clip speichern: {e}");
                    overlay.show(ToastKind::Error, e.to_string(), &s);
                    sound.play(SoundKind::Error, &s.sound);
                    let _ = app.emit(events::ERROR, ErrorPayload::from(&e));
                }
            }
        });
        if !accepted {
            self.toast(ToastKind::Error, tr(&s, "Speichern ausgelastet", "Saving is busy"), &s);
        }
    }

    fn start_recording(&mut self) {
        if !self.ensure_pipeline() {
            return;
        }
        let s = self.settings();
        let Some(p) = &self.pipeline else { return };
        let game = self.game_name();
        let s2 = s.clone();
        let cfg = RecorderConfig {
            split_bytes: u64::from(s.recording.split_size_mb) * 1024 * 1024,
            max_duration_us: i64::from(s.recording.max_duration_mins) * 60_000_000,
        };
        let next_path = Box::new(move || paths::new_output_file(&s2, &game, ClipKind::Recording));
        match Recorder::start(p.streams.clone(), cfg, next_path, self.rec_events.0.clone()) {
            Ok(r) => {
                p.router.attach_recorder(Some(r.tx.clone()));
                p.force_keyframe();
                self.recorder = Some(r);
                self.shared.info.write().recording_since = Some(Instant::now());
                self.toast(ToastKind::RecordingStarted, String::new(), &s);
                self.sound.play(SoundKind::RecordStart, &s.sound);
                crate::tray::refresh(&self.app);
            }
            Err(e) => self.report_error(&e),
        }
    }

    fn stop_recording(&mut self, notify: bool) {
        let Some(r) = self.recorder.take() else { return };
        if let Some(p) = &self.pipeline {
            p.router.attach_recorder(None);
        }
        // finish() wartet auf das Schreiben der restlichen Pakete (kurz, wenige ms).
        r.finish();
        self.shared.info.write().recording_since = None;
        if notify {
            let s = self.settings();
            self.sound.play(SoundKind::RecordStop, &s.sound);
        }
        if !self.buffer_enabled {
            self.stop_pipeline();
        }
        crate::tray::refresh(&self.app);
    }

    fn on_recorder(&mut self, ev: RecorderEvent) {
        let s = self.settings();
        match ev {
            RecorderEvent::FileDone { path, secs } => {
                self.toast(ToastKind::RecordingSaved, fmt_duration(secs), &s);
                let app = self.app.clone();
                let shared = self.shared.clone();
                self.shared.mux.submit(move || {
                    let _ = Library::ensure_thumbnail(&path);
                    if let Ok(info) = shared.library.info(&paths::output_dir(&s), &path) {
                        let _ = app.emit(events::CLIP_SAVED, info);
                    }
                });
            }
            RecorderEvent::LimitReached => self.stop_recording(true),
            RecorderEvent::Error(e) => {
                self.stop_recording(false);
                self.report_error(&CoreError::Other(e));
            }
        }
    }

    fn on_pipeline_error(&mut self, e: String) {
        // Eine laufende Aufnahme sauber abschließen, dann neu starten.
        self.stop_recording(false);
        self.stop_pipeline();
        self.report_error(&CoreError::Capture(e));
        self.schedule_restart();
    }

    fn schedule_restart(&self) {
        if self.restart_pending.swap(true, Ordering::Relaxed) {
            return;
        }
        let tx = self.self_tx.clone();
        let _ = crate::util::spawn("restart-timer", crate::util::Priority::BelowNormal, move || {
            std::thread::sleep(Duration::from_secs(3));
            let _ = tx.try_send(Cmd::Restart);
        });
    }

    fn on_game_changed(&mut self) {
        let s = self.settings();
        if s.capture.source != CaptureSource::AutoGame || self.pipeline.is_none() {
            return;
        }
        let monitor = self.shared.game.read().as_ref().map(|g| g.monitor.clone());
        // Nur neu starten, wenn das Spiel auf einem anderen Monitor läuft und nichts aufgenommen wird.
        if monitor != self.pipeline_monitor && self.recorder.is_none() {
            tracing::info!("Spiel auf anderem Monitor – Pipeline wird neu gestartet");
            self.stop_pipeline();
            self.ensure_pipeline();
        }
    }

    fn apply_settings(&mut self, mut new: Settings) {
        new.sanitize();
        let old = self.settings();
        if let Err(e) = settings::save(&new) {
            self.report_error(&e);
        }
        *self.shared.settings.write() = new.clone();
        let c = &self.shared.controls;
        c.set_volumes(new.audio.system.volume, new.audio.mic.volume);
        c.ptt_enabled.store(new.audio.push_to_talk, Ordering::Relaxed);
        self.sound.set_volume(new.sound.volume);

        let restart = old.capture != new.capture
            || old.video != new.video
            || old.audio.track_mode != new.audio.track_mode
            || old.audio.bitrate_kbps != new.audio.bitrate_kbps
            || old.audio.system.enabled != new.audio.system.enabled
            || old.audio.system.device_id != new.audio.system.device_id
            || old.audio.mic.enabled != new.audio.mic.enabled
            || old.audio.mic.device_id != new.audio.mic.device_id;
        if new.replay.enabled != self.buffer_enabled {
            self.set_buffer(new.replay.enabled);
        } else if let Some(p) = &self.pipeline {
            p.router.set_limits(buffer_limits(&new));
        }
        if restart && self.pipeline.is_some() {
            if self.recorder.is_some() {
                // Laufende Aufnahme nicht zerreißen: Neustart nach dem Stopp.
                tracing::info!("Video-/Audio-Änderung wird nach der Aufnahme übernommen");
            } else {
                self.stop_pipeline();
                self.ensure_pipeline();
            }
        }
        let _ = self.app.emit(events::SETTINGS, &new);
    }

    fn report_error(&self, e: &CoreError) {
        tracing::error!("{e}");
        let s = self.settings();
        {
            let mut info = self.shared.info.write();
            info.last_error = Some(e.to_string());
            info.state = Some(CoreState::Error);
        }
        self.overlay.show(ToastKind::Error, e.to_string(), &s);
        let _ = self.app.emit(events::ERROR, ErrorPayload::from(e));
    }

    fn toast(&self, kind: ToastKind, detail: String, s: &Settings) {
        self.overlay.show(kind, detail, s);
    }

    fn set_state(&self, st: CoreState) {
        self.shared.info.write().state = Some(st);
    }

    /// Leitet den Zustand aus Pipeline/Recorder ab.
    fn publish(&self) {
        let mut info = self.shared.info.write();
        info.buffer_enabled = self.buffer_enabled;
        let st = if self.recorder.is_some() {
            CoreState::Recording
        } else if self.pipeline.is_some() {
            CoreState::Buffering
        } else if info.state == Some(CoreState::Error) {
            CoreState::Error
        } else {
            CoreState::Idle
        };
        info.state = Some(st);
    }
}

fn after_save_housekeeping(app: &AppHandle, shared: &Shared, s: &Settings) {
    let root = paths::output_dir(s);
    if s.output.low_space_warning_gb > 0 {
        if let Some(free) = crate::library::free_space_bytes(&root) {
            if free < u64::from(s.output.low_space_warning_gb) * 1024 * 1024 * 1024 {
                let e = CoreError::Other(format!(
                    "{} ({:.1} GB)",
                    tr(s, "Wenig Speicherplatz", "Low disk space"),
                    free as f64 / 1e9
                ));
                let _ = app.emit(events::ERROR, ErrorPayload::from(&e));
            }
        }
    }
    if s.output.cleanup.enabled {
        let clips = shared.library.list(&root);
        let removed = crate::library::cleanup(&clips, s.output.cleanup.max_age_days, s.output.cleanup.max_total_gb);
        if !removed.is_empty() {
            tracing::info!("{} alte Clips in den Papierkorb verschoben", removed.len());
            let _ = app.emit(events::LIBRARY_CHANGED, ());
        }
    }
}

pub fn tr(s: &Settings, de: &str, en: &str) -> String {
    match s.general.language {
        settings::Language::De => de.into(),
        settings::Language::En => en.into(),
    }
}

pub fn fmt_duration(secs: f64) -> String {
    let t = secs.round() as u64;
    if t >= 3600 {
        format!("{}:{:02}:{:02}", t / 3600, (t / 60) % 60, t % 60)
    } else {
        format!("{}:{:02}", t / 60, t % 60)
    }
}
