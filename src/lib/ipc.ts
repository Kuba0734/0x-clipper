// Typisierte Wrapper um Tauri-Commands und -Events. Typen kommen aus Rust (ts-rs).
import { convertFileSrc, invoke } from "@tauri-apps/api/core";
import { listen, type UnlistenFn } from "@tauri-apps/api/event";

import type { AudioDevices } from "./bindings/AudioDevices";
import type { ClipInfo } from "./bindings/ClipInfo";
import type { EncoderInfo } from "./bindings/EncoderInfo";
import type { ErrorPayload } from "./bindings/ErrorPayload";
import type { HotkeyConflict } from "./bindings/HotkeyConflict";
import type { HotkeySettings } from "./bindings/HotkeySettings";
import type { MonitorInfo } from "./bindings/MonitorInfo";
import type { Settings } from "./bindings/Settings";
import type { Status } from "./bindings/Status";
import type { WindowInfo } from "./bindings/WindowInfo";

export type {
  AudioDevices,
  ClipInfo,
  EncoderInfo,
  ErrorPayload,
  HotkeyConflict,
  HotkeySettings,
  MonitorInfo,
  Settings,
  Status,
  WindowInfo,
};

export const commands = {
  getSettings: () => invoke<Settings>("get_settings"),
  updateSettings: (settings: Settings) => invoke<Settings>("update_settings", { settings }),
  resetSettings: () => invoke<Settings>("reset_settings"),
  exportSettings: (path: string) => invoke<void>("export_settings", { path }),
  importSettings: (path: string) => invoke<Settings>("import_settings", { path }),
  getStatus: () => invoke<Status>("get_status"),
  getEncoders: () => invoke<EncoderInfo[]>("get_encoders"),
  listMonitors: () => invoke<MonitorInfo[]>("list_monitors"),
  listWindows: () => invoke<WindowInfo[]>("list_windows"),
  listAudioDevices: () => invoke<AudioDevices>("list_audio_devices"),
  saveClip: () => invoke<void>("save_clip"),
  toggleRecording: () => invoke<void>("toggle_recording"),
  setBufferEnabled: (enabled: boolean) => invoke<void>("set_buffer_enabled", { enabled }),
  toggleMicMute: () => invoke<void>("toggle_mic_mute"),
  validateHotkeys: (hotkeys: HotkeySettings) =>
    invoke<HotkeyConflict[]>("validate_hotkeys", { hotkeys }),
  hotkeyConflicts: () => invoke<HotkeyConflict[]>("hotkey_conflicts"),
  suspendHotkeys: (suspended: boolean) => invoke<void>("suspend_hotkeys", { suspended }),
  listClips: () => invoke<ClipInfo[]>("list_clips"),
  getThumbnail: (path: string) => invoke<string>("get_thumbnail", { path }),
  trimClip: (path: string, startSecs: number, endSecs: number, replace: boolean) =>
    invoke<ClipInfo>("trim_clip", { path, startSecs, endSecs, replace }),
  deleteClip: (path: string) => invoke<void>("delete_clip", { path }),
  revealClip: (path: string) => invoke<void>("reveal_clip", { path }),
  openClipsFolder: () => invoke<void>("open_clips_folder"),
  defaultOutputDir: () => invoke<string>("default_output_dir"),
  quitApp: () => invoke<void>("quit_app"),
};

/** Event-Namen spiegeln `src-tauri/src/events.rs`. */
export interface CoreEvents {
  status: Status;
  "clip-saved": ClipInfo;
  "core-error": ErrorPayload;
  "settings-changed": Settings;
  encoders: EncoderInfo[];
  "library-changed": null;
  "hotkey-conflicts": HotkeyConflict[];
}

export function on<K extends keyof CoreEvents>(
  event: K,
  handler: (payload: CoreEvents[K]) => void,
): Promise<UnlistenFn> {
  return listen<CoreEvents[K]>(event, (e) => handler(e.payload));
}

/** Lokale Datei (Clip, Thumbnail) für <video>/<img> über das Asset-Protokoll. */
export const fileSrc = (path: string) => convertFileSrc(path);

/** Fehler aus `invoke` (ErrorPayload oder String) in Text umwandeln. */
export function errorMessage(e: unknown): string {
  if (typeof e === "string") return e;
  if (e && typeof e === "object" && "message" in e) return String((e as ErrorPayload).message);
  return String(e);
}
