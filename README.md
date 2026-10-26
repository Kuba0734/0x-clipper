# 0x-clipper

Ressourcenschonender Clip-Recorder für Windows 10/11 (x64): Replay-Buffer im Hintergrund,
Clip per Hotkey, normale Aufnahme, Bibliothek mit Trimmen. Details und Regeln: [CLAUDE.md](CLAUDE.md).

## Voraussetzungen

- Rust (stable, MSVC), Visual Studio Build Tools (C++), LLVM (für `ffmpeg-sys-next`/bindgen)
- Node.js + pnpm
- NSIS (nur für `pnpm tauri build`)
- FFmpeg 9.0 LGPL-Shared-Build: `powershell -File scripts/fetch-ffmpeg.ps1`

## Befehle

```bash
pnpm install
pnpm tauri dev
pnpm tauri build                 # NSIS-Setup unter src-tauri/target/release/bundle/nsis/
pnpm lint && pnpm typecheck
cd src-tauri && cargo clippy --all-targets -- -D warnings && cargo test && cargo bench
cd src-tauri && cargo run --release --example smoke -- 10   # Headless-Messung des heißen Pfads
```

TS-Typen (`src/lib/bindings`) entstehen bei `cargo test` aus den Rust-Typen (ts-rs).
Icons: `powershell -File scripts/make-icons.ps1`, danach `pnpm tauri icon src-tauri/icons/app-icon.png`.
