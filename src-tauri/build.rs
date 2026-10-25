use std::path::PathBuf;

fn main() {
    println!("cargo:rerun-if-changed=ffmpeg/bin");
    copy_ffmpeg_dlls();
    tauri_build::build();
}

/// Legt die FFmpeg-DLLs neben die EXE (target/<profil>), damit `tauri dev`, Tests und
/// Benchmarks sie finden. Im Installer kommen sie über `bundle.resources` mit.
fn copy_ffmpeg_dlls() {
    let Ok(out) = std::env::var("OUT_DIR") else { return };
    // OUT_DIR = target/<profil>/build/<crate>/out
    let Some(profile_dir) = PathBuf::from(out).ancestors().nth(3).map(PathBuf::from) else { return };
    let bin = PathBuf::from("ffmpeg/bin");
    let Ok(entries) = std::fs::read_dir(&bin) else {
        println!("cargo:warning=ffmpeg/bin fehlt – scripts/fetch-ffmpeg.ps1 ausführen");
        return;
    };
    for e in entries.flatten() {
        let p = e.path();
        if p.extension().is_some_and(|x| x == "dll") {
            if let Some(name) = p.file_name() {
                for dir in [profile_dir.clone(), profile_dir.join("deps")] {
                    let dst = dir.join(name);
                    if !dst.exists() {
                        let _ = std::fs::copy(&p, &dst);
                    }
                }
            }
        }
    }
}
