//! Event-Namen (Core → UI). Spiegeln `src/lib/ipc.ts`.

pub const STATUS: &str = "status";
pub const CLIP_SAVED: &str = "clip-saved";
pub const ERROR: &str = "core-error";
pub const SETTINGS: &str = "settings-changed";
pub const ENCODERS: &str = "encoders";
pub const LIBRARY_CHANGED: &str = "library-changed";
pub const HOTKEY_CONFLICTS: &str = "hotkey-conflicts";
