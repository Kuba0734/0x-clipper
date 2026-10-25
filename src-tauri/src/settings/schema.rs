//! Versioniertes Settings-Schema. Einzige Quelle der Wahrheit für alle Optionen.
//! Neue Option = Feld hier + Default + Migration (migrate.rs) + UI-Control.

use serde::{Deserialize, Serialize};
use ts_rs::TS;

pub const SETTINGS_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct Settings {
    pub version: u32,
    pub general: GeneralSettings,
    pub replay: ReplaySettings,
    pub capture: CaptureSettings,
    pub video: VideoSettings,
    pub audio: AudioSettings,
    pub hotkeys: HotkeySettings,
    pub output: OutputSettings,
    pub recording: RecordingSettings,
    pub overlay: OverlaySettings,
    pub sound: SoundSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Settings {
            version: SETTINGS_VERSION,
            general: GeneralSettings::default(),
            replay: ReplaySettings::default(),
            capture: CaptureSettings::default(),
            video: VideoSettings::default(),
            audio: AudioSettings::default(),
            hotkeys: HotkeySettings::default(),
            output: OutputSettings::default(),
            recording: RecordingSettings::default(),
            overlay: OverlaySettings::default(),
            sound: SoundSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Language {
    De,
    En,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct GeneralSettings {
    pub language: Language,
    pub autostart: bool,
    /// Beim Autostart nur ins Tray starten.
    pub start_minimized: bool,
    pub reduce_motion: bool,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        GeneralSettings {
            language: Language::De,
            autostart: false,
            start_minimized: true,
            reduce_motion: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct ReplaySettings {
    pub enabled: bool,
    /// 5 s – 600 s.
    pub duration_secs: u32,
    /// Obergrenze für den Ringpuffer im RAM.
    pub ram_limit_mb: u32,
}

impl Default for ReplaySettings {
    fn default() -> Self {
        ReplaySettings { enabled: true, duration_secs: 30, ram_limit_mb: 512 }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", tag = "kind")]
#[ts(export)]
pub enum CaptureSource {
    /// Monitor anhand des GDI-Gerätenamens (z. B. `\\.\DISPLAY1`); leer = primärer Monitor.
    Monitor { device: String },
    /// Fenster anhand Prozessname und Titel (HWNDs sind nicht stabil).
    Window { exe: String, title: String },
    /// Monitor des Vollbild-Spiels im Vordergrund, sonst primärer Monitor.
    AutoGame,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum CaptureBackend {
    Auto,
    Wgc,
    Dxgi,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct CaptureSettings {
    pub source: CaptureSource,
    pub backend: CaptureBackend,
    pub capture_cursor: bool,
    /// Gelben WGC-Rahmen ausblenden (ab Windows 11, sofern erlaubt).
    pub hide_border: bool,
}

impl Default for CaptureSettings {
    fn default() -> Self {
        CaptureSettings {
            source: CaptureSource::AutoGame,
            backend: CaptureBackend::Auto,
            capture_cursor: true,
            hide_border: true,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum QualityPreset {
    Performance,
    Balanced,
    Quality,
    Custom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", tag = "kind")]
#[ts(export)]
pub enum Resolution {
    Native,
    P1440,
    P1080,
    P720,
    Custom { width: u32, height: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum Codec {
    H264,
    Hevc,
    Av1,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum RateControl {
    Cbr,
    Vbr,
    Cqp,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct VideoSettings {
    pub preset: QualityPreset,
    pub resolution: Resolution,
    pub fps: u32,
    /// FFmpeg-Encodername (z. B. `h264_nvenc`) oder `auto`.
    pub encoder: String,
    pub codec: Codec,
    pub rate_control: RateControl,
    pub bitrate_kbps: u32,
    /// Qualitätsstufe für CQP (0 = beste, 51 = schlechteste).
    pub cqp: u32,
    /// 1–2 s.
    pub keyframe_interval_secs: u32,
}

impl Default for VideoSettings {
    fn default() -> Self {
        VideoSettings {
            preset: QualityPreset::Balanced,
            resolution: Resolution::Native,
            fps: 60,
            encoder: "auto".into(),
            codec: Codec::H264,
            rate_control: RateControl::Cbr,
            bitrate_kbps: 20_000,
            cqp: 23,
            keyframe_interval_secs: 1,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum TrackMode {
    /// System und Mikrofon als eigene Spuren.
    Separate,
    /// Eine gemischte Spur.
    Mixed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct AudioSourceSettings {
    pub enabled: bool,
    /// Endpunkt-ID von WASAPI; leer = Standardgerät.
    pub device_id: String,
    /// Linear, 0.0 – 2.0.
    pub volume: f32,
}

impl Default for AudioSourceSettings {
    fn default() -> Self {
        AudioSourceSettings { enabled: true, device_id: String::new(), volume: 1.0 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct AudioSettings {
    pub system: AudioSourceSettings,
    pub mic: AudioSourceSettings,
    pub track_mode: TrackMode,
    pub push_to_talk: bool,
    pub bitrate_kbps: u32,
}

impl Default for AudioSettings {
    fn default() -> Self {
        AudioSettings {
            system: AudioSourceSettings::default(),
            mic: AudioSourceSettings { enabled: false, ..AudioSourceSettings::default() },
            track_mode: TrackMode::Separate,
            push_to_talk: false,
            bitrate_kbps: 160,
        }
    }
}

/// Hotkeys im Accelerator-Format (`Alt+F10`, `Ctrl+Shift+S`). Leer = nicht belegt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct HotkeySettings {
    pub save_clip: String,
    pub toggle_recording: String,
    pub toggle_buffer: String,
    pub toggle_mic_mute: String,
    pub push_to_talk: String,
}

impl Default for HotkeySettings {
    fn default() -> Self {
        HotkeySettings {
            save_clip: "Alt+F10".into(),
            toggle_recording: "Alt+F9".into(),
            toggle_buffer: "Alt+Shift+F10".into(),
            toggle_mic_mute: String::new(),
            push_to_talk: String::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct CleanupSettings {
    pub enabled: bool,
    /// 0 = kein Alterslimit.
    pub max_age_days: u32,
    /// 0 = kein Größenlimit.
    pub max_total_gb: u32,
}

impl Default for CleanupSettings {
    fn default() -> Self {
        CleanupSettings { enabled: false, max_age_days: 30, max_total_gb: 50 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct OutputSettings {
    /// Leer = `%USERPROFILE%\Videos\0x-clipper`.
    pub folder: String,
    /// Platzhalter: `{game}`, `{date}`, `{time}`, `{type}`.
    pub filename_pattern: String,
    pub subfolder_per_game: bool,
    /// Warnung, wenn weniger frei ist (GB). 0 = aus.
    pub low_space_warning_gb: u32,
    pub cleanup: CleanupSettings,
}

impl Default for OutputSettings {
    fn default() -> Self {
        OutputSettings {
            folder: String::new(),
            filename_pattern: "{game}_{date}_{time}".into(),
            subfolder_per_game: true,
            low_space_warning_gb: 10,
            cleanup: CleanupSettings::default(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct RecordingSettings {
    /// 0 = unbegrenzt.
    pub max_duration_mins: u32,
    /// Neue Datei ab dieser Größe (MB). 0 = kein Split.
    pub split_size_mb: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase")]
#[ts(export)]
pub enum OverlayPosition {
    TopLeft,
    TopRight,
    BottomLeft,
    BottomRight,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct OverlaySettings {
    pub enabled: bool,
    pub position: OverlayPosition,
    pub duration_ms: u32,
}

impl Default for OverlaySettings {
    fn default() -> Self {
        OverlaySettings { enabled: true, position: OverlayPosition::TopRight, duration_ms: 2500 }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, TS)]
#[serde(rename_all = "camelCase", default)]
#[ts(export)]
pub struct SoundSettings {
    pub enabled: bool,
    /// 0.0 – 1.0.
    pub volume: f32,
}

impl Default for SoundSettings {
    fn default() -> Self {
        SoundSettings { enabled: true, volume: 0.5 }
    }
}

pub const FPS_CHOICES: [u32; 5] = [30, 60, 120, 144, 240];

impl Settings {
    /// Klemmt alle Werte in gültige Bereiche. Wird nach dem Laden und vor dem Speichern aufgerufen.
    pub fn sanitize(&mut self) {
        self.version = SETTINGS_VERSION;
        self.replay.duration_secs = self.replay.duration_secs.clamp(5, 600);
        self.replay.ram_limit_mb = self.replay.ram_limit_mb.clamp(64, 16_384);
        if !FPS_CHOICES.contains(&self.video.fps) {
            self.video.fps = 60;
        }
        self.video.bitrate_kbps = self.video.bitrate_kbps.clamp(1_000, 150_000);
        self.video.cqp = self.video.cqp.min(51);
        self.video.keyframe_interval_secs = self.video.keyframe_interval_secs.clamp(1, 2);
        if let Resolution::Custom { width, height } = &mut self.video.resolution {
            // Encoder brauchen gerade Abmessungen.
            *width = (*width).clamp(320, 7680) & !1;
            *height = (*height).clamp(180, 4320) & !1;
        }
        if self.video.encoder.trim().is_empty() {
            self.video.encoder = "auto".into();
        }
        for a in [&mut self.audio.system, &mut self.audio.mic] {
            a.volume = if a.volume.is_finite() { a.volume.clamp(0.0, 2.0) } else { 1.0 };
        }
        self.audio.bitrate_kbps = self.audio.bitrate_kbps.clamp(64, 320);
        self.overlay.duration_ms = self.overlay.duration_ms.clamp(800, 10_000);
        self.sound.volume =
            if self.sound.volume.is_finite() { self.sound.volume.clamp(0.0, 1.0) } else { 0.5 };
        if self.output.filename_pattern.trim().is_empty() {
            self.output.filename_pattern = OutputSettings::default().filename_pattern;
        }
    }

    /// Wendet ein Qualitätspreset auf Bitrate und Rate-Control an.
    pub fn apply_preset(&mut self) {
        let (rc, kbps) = match self.video.preset {
            QualityPreset::Performance => (RateControl::Cbr, 12_000),
            QualityPreset::Balanced => (RateControl::Cbr, 20_000),
            QualityPreset::Quality => (RateControl::Vbr, 40_000),
            QualityPreset::Custom => return,
        };
        self.video.rate_control = rc;
        self.video.bitrate_kbps = kbps;
    }
}
