# CLAUDE.md – 0x-clipper

0x-clipper ist ein ressourcenschonender Clip-Recorder **nur für Windows 10/11 (x64)**, ähnlich wie ShadowPlay oder Medal.
Er hält im Hintergrund einen Replay-Buffer und speichert per Hotkey die letzten N Sekunden oder startet und stoppt eine normale Aufnahme.
Die App wird als Setup (NSIS-Installer) ausgeliefert.

Oberste Regel: **Performance vor Features.** 0x-clipper läuft neben Spielen. Jede Millisekunde CPU und jedes MB RAM zählt.

---

## Tech-Stack

| Bereich | Technologie |
|---|---|
| App-Shell | **Tauri 2** (Rust-Backend, WebView2-Frontend) |
| Backend | **Rust** (stable, edition 2021) |
| Bildschirmaufnahme | **Windows Graphics Capture (WGC)** über `windows`-Crate, Fallback **DXGI Desktop Duplication** |
| Encoding | **FFmpeg** (`ffmpeg-next`, gebündelte DLLs) mit Hardware-Encodern: `h264_nvenc` / `hevc_nvenc` / `av1_nvenc`, `h264_amf` / `hevc_amf`, `h264_qsv` / `hevc_qsv`; Fallback `libx264` (preset `veryfast`) |
| Audio | **WASAPI** (System-Loopback + Mikrofon, getrennte Spuren), Encoding AAC |
| Hotkeys | `tauri-plugin-global-shortcut` (intern `RegisterHotKey`) |
| Frontend | **React 18 + TypeScript + Vite**, **Tailwind CSS**, **Motion** (framer-motion) für Animationen |
| State (UI) | **Zustand** |
| Einstellungen | JSON in `%APPDATA%\0x-clipper\settings.json`, versioniert mit Migrationen |
| Installer | Tauri-Bundler → **NSIS** (`.exe`-Setup), WebView2-Bootstrapper eingebettet |
| Logging | `tracing` + rotierende Logdatei in `%LOCALAPPDATA%\0x-clipper\logs` |

Keine anderen Plattformen. Kein `#[cfg(target_os = ...)]`-Gefummel für macOS/Linux, kein Electron.

---

## Architektur

```
┌────────────── UI (WebView2, React) ──────────────┐
│  Dashboard · Clip-Bibliothek · Einstellungen      │
│  Toast-Overlay (eigenes transparentes Fenster)    │
└──────────────▲───────────────────┬────────────────┘
               │ Events            │ Commands (invoke)
┌──────────────┴───────────────────▼────────────────┐
│                Rust Core (Tauri)                   │
│  HotkeyManager → Controller (State Machine)        │
│        │                                           │
│  CaptureThread (WGC, D3D11-Texturen)               │
│        │ GPU-Textur (zero-copy)                    │
│  EncoderThread (FFmpeg HW, d3d11va hwframes)       │
│        │ encodierte Pakete                         │
│  ReplayRingBuffer (Pakete + PTS, im RAM)           │
│        │ bei Hotkey: Snapshot                      │
│  MuxWorker (MP4 schreiben, Thumbnail, Event)       │
│  AudioThreads (WASAPI Loopback / Mic → AAC)        │
└────────────────────────────────────────────────────┘
```

### Kernprinzipien

1. **Ringpuffer speichert encodierte Pakete, nie Rohframes.**
   30 s 1080p60 bei 20 Mbit/s ≈ 75 MB statt mehrerer GB.
2. **Zero-Copy GPU-Pfad:** WGC-Textur → D3D11 → Hardware-Encoder, ohne CPU-Readback. CPU-Kopie nur im `libx264`-Fallback.
3. **Keyframe-Intervall = 1 s** (konfigurierbar 1–2 s), damit Clips sauber an einem Keyframe beginnen. Beim Speichern wird ab dem letzten Keyframe vor `jetzt - N` geschnitten.
4. **Speichern blockiert nie die Aufnahme.** Snapshot des Puffers (Arc-Klone der Pakete) → `MuxWorker` auf eigenem Thread → Remux nach MP4 (`-c copy`, kein Re-Encode).
5. **Feste Threads mit klaren Prioritäten:** Capture und Encoder `THREAD_PRIORITY_ABOVE_NORMAL`, Mux und Thumbnails `BELOW_NORMAL`. Kein Tokio auf dem heißen Pfad. Kommunikation über `crossbeam-channel` (bounded).
6. **Backpressure:** Wenn der Encoder nicht hinterherkommt, Frames verwerfen und zählen. Nie unbegrenzt puffern.
7. **UI ist optional.** Beim Schließen ins Tray: das WebView wird zerstört (nicht nur versteckt), damit kein Renderer-Prozess RAM frisst. Aufnahme läuft im Rust-Core weiter.
8. **Toast-Overlay** ist ein eigenes, kleines, transparentes, click-through Fenster (`WS_EX_TRANSPARENT | WS_EX_LAYERED | WS_EX_NOACTIVATE`). Es stiehlt nie den Fokus vom Spiel.

### Controller-State-Machine

```
Idle ──enable──▶ Buffering ──hotkey_record──▶ Recording
  ▲                 │  ▲                          │
  └──disable────────┘  └──────hotkey_record───────┘
Buffering ──hotkey_clip──▶ (Snapshot → MuxWorker) ──▶ Buffering
```
Während `Recording` läuft der Replay-Buffer weiter (wenn aktiviert), damit „Clip speichern“ auch dann funktioniert.

---

## Funktionsumfang (alles einstellbar)

**Replay-Buffer**
- An/Aus, Länge 5 s – 10 min (Standard 30 s)
- RAM-Limit mit Live-Anzeige des aktuellen Verbrauchs

**Aufnahme**
- Manuelle Aufnahme starten/stoppen per Hotkey
- Optional: max. Dauer, Dateigröße-Split

**Quelle**
- Ganzer Monitor (Auswahl), bestimmtes Fenster, automatische Erkennung des Vollbild-Spiels
- Cursor ein/aus, gelber WGC-Rahmen aus (wo das OS es erlaubt)

**Video**
- Auflösung (nativ, 1440p, 1080p, 720p, eigene), Skalierung auf der GPU
- FPS 30/60/120/144/240
- Encoder (automatisch erkannt, beste Option vorausgewählt), Codec H.264/HEVC/AV1
- Rate-Control CBR/VBR/CQP, Bitrate oder Qualitätsstufe
- Presets „Leistung / Ausgewogen / Qualität“ plus „Benutzerdefiniert“

**Audio**
- System-Audio, Mikrofon, jeweils Gerät wählbar
- Getrennte Spuren oder gemischt, Lautstärke pro Quelle
- Push-to-Talk-Taste fürs Mikrofon (optional)

**Hotkeys** (alle frei belegbar, Konflikte werden erkannt)
- Clip speichern (Standard `Alt+F10`)
- Aufnahme starten/stoppen (Standard `Alt+F9`)
- Replay-Buffer an/aus (Standard `Alt+Shift+F10`)
- Mikrofon stummschalten

**Ausgabe**
- Zielordner, Dateinamensmuster (`{game}_{date}_{time}`), Unterordner pro Spiel
- Speicherplatz-Warnung, automatische Bereinigung alter Clips (optional)

**Feedback**
- Toast-Overlay (Position, Dauer, an/aus), Sound (an/aus, Lautstärke)

**System**
- Autostart mit Windows (minimiert ins Tray), Sprache DE/EN, Einstellungen importieren/exportieren

**Bibliothek**
- Grid mit Thumbnails, Hover-Vorschau, Abspielen, einfaches Trimmen (Remux ohne Re-Encode, keyframe-genau), Im-Explorer-zeigen, Löschen (Papierkorb)

---

## Performance-Budget (verbindlich)

Gemessen mit 1080p60, NVENC, 30 s Buffer, Spiel im Vordergrund:

| Metrik | Ziel |
|---|---|
| CPU-Last Core (Buffering) | < 2 % |
| RAM Core (ohne Buffer) | < 60 MB |
| RAM UI offen | < 150 MB gesamt |
| UI geschlossen (Tray) | kein WebView-Prozess |
| Spiel-FPS-Verlust | < 3 % |
| Hotkey → Clip-Datei fertig | < 1,5 s |
| Kaltstart bis Buffering | < 1 s |

Jede Änderung am heißen Pfad (Capture, Encode, Ringpuffer) braucht einen Benchmark oder eine Begründung im PR.

### Regeln für den heißen Pfad
- Keine Allokationen pro Frame. Puffer wiederverwenden, Pools nutzen.
- Kein `Mutex` zwischen Capture und Encoder, nur Channels.
- Kein Logging pro Frame (nur `trace!` hinter Feature-Flag).
- Keine `unwrap()` / `expect()` im Core. Fehler → `thiserror`-Typen → Event an die UI.
- `unsafe` nur für Win32/D3D11-FFI, jeweils mit `// SAFETY:`-Kommentar.

### Regeln für das Frontend
- Animationen **nur** über `transform` und `opacity` (GPU-komponiert). Nie `width`, `height`, `top`, `left`, `box-shadow` animieren.
- `prefers-reduced-motion` respektieren und zusätzlich in den Einstellungen „Animationen reduzieren“ anbieten.
- Listen in der Bibliothek virtualisieren (`@tanstack/react-virtual`).
- Thumbnails werden im Rust-Core erzeugt (ein Frame per FFmpeg, WebP, 320 px) und gecacht, nie im Browser.
- Statusdaten (RAM-Verbrauch, Bitrate) maximal 2× pro Sekunde an die UI senden, und nur wenn ein Fenster offen ist.
- Bundle-Größe im Blick: keine schweren UI-Bibliotheken, Icons per Tree-Shaking (`lucide-react`).

---

## Design-System

Modern, dunkel, ruhig, mit lebendigen, aber nicht aufdringlichen Animationen.

**Farben (CSS-Variablen in `src/styles/tokens.css`)**
```
--bg:            #0B0B0F   /* App-Hintergrund */
--surface:       #14141A   /* Karten */
--surface-2:     #1C1C24   /* Hover, Inputs */
--border:        #2A2A35
--text:          #EDEDF2
--text-muted:    #8B8B9A
--accent:        #7C5CFF   /* Violett */
--accent-2:      #22D3EE   /* Cyan für Verläufe */
--success:       #22C55E
--danger:        #EF4444   /* Aufnahme-Indikator */
--radius:        14px
```
- Akzent-Verlauf `linear-gradient(135deg, var(--accent), var(--accent-2))` für primäre Buttons, aktive Navigation und den Buffer-Ring.
- Leichtes Glas für Overlays: `backdrop-filter: blur(16px)` auf halbtransparentem `--surface`.
- Schrift: **Inter** (lokal gebündelt), Zahlen mit `font-variant-numeric: tabular-nums`.
- Eigene Titelleiste (`decorations: false`) mit Drag-Region und eigenen Fensterbuttons.

**Animationen (Motion)**
- Standard-Spring: `{ type: "spring", stiffness: 380, damping: 30 }`; Dauer kurzer Übergänge 150–250 ms.
- Seitenwechsel: Fade + 8 px Slide, `AnimatePresence mode="wait"`.
- Karten: Hover-Lift (`y: -2`, leichter Scale 1.01), gestaffeltes Einblenden (`staggerChildren: 0.03`).
- Buffer-Indikator: animierter Ring, der den Füllstand zeigt; pulsierender roter Punkt bei Aufnahme.
- „Clip gespeichert“: Toast fliegt mit Spring ein, Häkchen wird als SVG-Pfad gezeichnet (`pathLength`), dann Auto-Dismiss.
- Hotkey-Eingabe: Tasten erscheinen als „Keycaps“ mit kurzem Press-Effekt.
- Toggles und Slider mit `layout`-Animation für den Knopf.
- Skeleton-Shimmer beim Laden der Bibliothek.

---

## Ordnerstruktur

```
0x-clipper/
├─ src/                      # React-Frontend
│  ├─ app/                   # Routing, Layout, Titelleiste
│  ├─ features/
│  │  ├─ dashboard/
│  │  ├─ library/
│  │  ├─ settings/
│  │  └─ overlay/            # Toast-Fenster
│  ├─ components/ui/         # Button, Toggle, Slider, Select, KeycapInput …
│  ├─ lib/ipc.ts             # typisierte Wrapper um invoke/listen
│  ├─ stores/                # Zustand
│  └─ styles/tokens.css
├─ src-tauri/
│  ├─ src/
│  │  ├─ main.rs
│  │  ├─ commands.rs         # Tauri-Commands (dünn, nur Delegation)
│  │  ├─ controller.rs       # State Machine
│  │  ├─ capture/            # wgc.rs, dxgi.rs
│  │  ├─ encode/             # ffmpeg.rs, probe.rs (Encoder-Erkennung)
│  │  ├─ audio/              # wasapi.rs
│  │  ├─ buffer/ring.rs      # ReplayRingBuffer
│  │  ├─ mux/                # mp4 writer, trim, thumbnails
│  │  ├─ hotkeys.rs
│  │  ├─ settings/           # schema.rs, migrate.rs
│  │  ├─ tray.rs
│  │  └─ overlay.rs
│  ├─ ffmpeg/                # gebündelte DLLs (LGPL-Build!)
│  ├─ benches/
│  └─ tauri.conf.json
└─ CLAUDE.md
```

---

## Befehle

```bash
pnpm install                  # Frontend-Abhängigkeiten
pnpm tauri dev                # Entwicklung
pnpm tauri build              # Release + NSIS-Setup unter src-tauri/target/release/bundle/nsis/
pnpm lint && pnpm typecheck   # Frontend-Checks
cargo clippy --all-targets -- -D warnings   # in src-tauri/
cargo test                    # in src-tauri/
cargo bench                   # Ringpuffer- und Mux-Benchmarks
```

Release-Profil in `Cargo.toml`:
```toml
[profile.release]
lto = "fat"
codegen-units = 1
opt-level = 3
panic = "abort"
strip = true
```

---

## Konventionen

- **Typisierte IPC:** Jeder Tauri-Command hat ein passendes TS-Interface in `src/lib/ipc.ts`. Rust-Typen mit `serde` + `specta`/`ts-rs` exportieren, nicht manuell doppeln.
- **Settings-Schema** ist die einzige Quelle der Wahrheit. Neue Option = Feld im Schema + Default + Migration + UI-Control.
- Commits: Conventional Commits (`feat:`, `fix:`, `perf:` …).
- UI-Texte über i18n-Keys (`de.json`, `en.json`), keine hartkodierten Strings.
- Neue UI-Komponenten zuerst in `components/ui`, mit Reduced-Motion-Variante.

## Installer

- NSIS, per-user-Installation standardmäßig (kein Admin nötig), Option für alle Benutzer.
- WebView2-Bootstrapper einbetten (`webviewInstallMode: embedBootstrapper`).
- Startmenü-Eintrag, optional Desktop-Verknüpfung, optional Autostart.
- Deinstallation entfernt Programmdateien, fragt nach, ob Einstellungen und Clips behalten werden sollen (Clips standardmäßig behalten).
- FFmpeg nur als **LGPL-Build** bündeln, Lizenztexte mit ausliefern.

## Nicht tun

- Kein Electron, kein .NET-Runtime-Zwang.
- Kein Re-Encode beim Speichern oder Trimmen.
- Keine Rohframes im RAM puffern.
- Kein Polling für Hotkeys oder Fensterwechsel (Events/Hooks nutzen).
- Keine Telemetrie ohne explizites Opt-in.
- Kein Injizieren in Spielprozesse (Anti-Cheat!). Nur WGC/DXGI.