use serde::Serialize;
use ts_rs::TS;

/// Zentrale Fehlertypen des Cores. Werden als Event an die UI weitergereicht.
#[derive(Debug, thiserror::Error)]
pub enum CoreError {
    #[error("Windows-API: {0}")]
    Windows(#[from] windows::core::Error),
    #[error("FFmpeg: {0}")]
    Ffmpeg(String),
    #[error("E/A: {0}")]
    Io(#[from] std::io::Error),
    #[error("JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("Capture: {0}")]
    Capture(String),
    #[error("Encoder: {0}")]
    Encoder(String),
    #[error("Audio: {0}")]
    Audio(String),
    #[error("Hotkey: {0}")]
    Hotkey(String),
    #[error("Einstellungen: {0}")]
    Settings(String),
    #[error("Kein Inhalt im Replay-Buffer")]
    BufferEmpty,
    #[error("Pipeline läuft nicht")]
    NotRunning,
    #[error("{0}")]
    Other(String),
}

impl From<ffmpeg_next::Error> for CoreError {
    fn from(e: ffmpeg_next::Error) -> Self {
        CoreError::Ffmpeg(e.to_string())
    }
}

impl From<tauri::Error> for CoreError {
    fn from(e: tauri::Error) -> Self {
        CoreError::Other(e.to_string())
    }
}

impl CoreError {
    pub fn code(&self) -> &'static str {
        match self {
            CoreError::Windows(_) => "windows",
            CoreError::Ffmpeg(_) => "ffmpeg",
            CoreError::Io(_) => "io",
            CoreError::Json(_) => "json",
            CoreError::Capture(_) => "capture",
            CoreError::Encoder(_) => "encoder",
            CoreError::Audio(_) => "audio",
            CoreError::Hotkey(_) => "hotkey",
            CoreError::Settings(_) => "settings",
            CoreError::BufferEmpty => "bufferEmpty",
            CoreError::NotRunning => "notRunning",
            CoreError::Other(_) => "other",
        }
    }
}

/// Serialisierte Fehlerform für Commands und Events.
#[derive(Debug, Clone, Serialize, TS)]
#[ts(export)]
pub struct ErrorPayload {
    pub code: String,
    pub message: String,
}

impl From<&CoreError> for ErrorPayload {
    fn from(e: &CoreError) -> Self {
        ErrorPayload { code: e.code().into(), message: e.to_string() }
    }
}

impl Serialize for CoreError {
    fn serialize<S: serde::Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        ErrorPayload::from(self).serialize(s)
    }
}

pub type CoreResult<T> = Result<T, CoreError>;
