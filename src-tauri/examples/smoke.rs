//! Headless-Test des heißen Pfads ohne UI: Pipeline starten, puffern, Clip speichern,
//! Aufnahme, Trimmen, Thumbnail. Gibt Messwerte für das Performance-Budget aus.
//!
//! `cargo run --release --example smoke -- [Sekunden] [Encoder]`

use std::sync::atomic::Ordering;
use std::sync::Arc;
use std::time::{Duration, Instant};

use clipper_lib::audio::AudioControls;
use clipper_lib::capture::d3d::D3dDevice;
use clipper_lib::encode::probe;
use clipper_lib::mux::{media, mp4, recorder};
use clipper_lib::pipeline::{Pipeline, PipelineDeps};
use clipper_lib::settings::{CaptureSource, Settings};
use clipper_lib::stats::Stats;
use clipper_lib::{sysinfo, util};

fn main() {
    if let Err(e) = run() {
        eprintln!("FEHLER: {e}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let t0 = Instant::now();
    clipper_lib::encode::ffmpeg::init_logging();
    util::com_init();
    let args: Vec<String> = std::env::args().collect();
    let secs: u64 = args.get(1).and_then(|s| s.parse().ok()).unwrap_or(10);
    let encoder = args.get(2).cloned().unwrap_or_else(|| "auto".into());
    let out_dir = std::env::temp_dir().join("0x-clipper-smoke");
    std::fs::create_dir_all(&out_dir)?;

    let mb = || sysinfo::private_bytes() as f64 / 1048576.0;
    println!("RAM nach Prozessstart: {:.1} MB", mb());
    let tp = Instant::now();
    let encoders = match probe::cached() {
        Some(c) => c,
        None => probe::probe(&D3dDevice::high_performance()?),
    };
    println!("Encoder ({} ms): {:?}", tp.elapsed().as_millis(), encoders.iter().map(|e| &e.id).collect::<Vec<_>>());

    let mut s = Settings::default();
    s.capture.source = CaptureSource::Monitor { device: String::new() };
    s.video.encoder = encoder;
    s.audio.mic.enabled = false;
    let stats = Arc::new(Stats::default());
    let (err_tx, err_rx) = crossbeam_channel::bounded(16);
    let deps = PipelineDeps {
        stats: stats.clone(),
        controls: Arc::new(AudioControls::new()),
        encoders,
        errors: err_tx,
        game_monitor: None,
    };
    let ts = Instant::now();
    let pipeline = Pipeline::start(&s, deps)?;
    println!(
        "Pipeline: {} {}x{} gestartet in {} ms (seit Prozessstart {} ms)",
        pipeline.encoder.id,
        pipeline.resolution.0,
        pipeline.resolution.1,
        ts.elapsed().as_millis(),
        t0.elapsed().as_millis()
    );
    println!("RAM nach Pipeline-Start (Buffer leer): {:.1} MB", mb());
    // Warten, bis der erste Frame im Buffer liegt.
    let tf = Instant::now();
    while stats.encoded_frames.load(Ordering::Relaxed) == 0 && tf.elapsed() < Duration::from_secs(5) {
        std::thread::sleep(Duration::from_millis(5));
    }
    println!("Erster encodierter Frame nach {} ms seit Prozessstart", t0.elapsed().as_millis());

    // Messfenster: CPU und RAM während des Bufferns.
    std::thread::sleep(Duration::from_secs(2));
    let cpu0 = sysinfo::cpu_time_100ns();
    let w0 = Instant::now();
    let f0 = stats.encoded_frames.load(Ordering::Relaxed);
    let c0 = stats.captured_frames.load(Ordering::Relaxed);
    // Zeitreihe: wächst der Prozess über den Buffer hinaus (Leck)?
    for i in 0..secs.div_ceil(10) {
        std::thread::sleep(Duration::from_secs(10.min(secs - i * 10)));
        let buf = stats.buffer_bytes.load(Ordering::Relaxed) as f64 / 1048576.0;
        println!(
            "  t={:>3} s  privat {:.1} MB  Buffer {:.1} MB  ohne Buffer {:.1} MB  Frames {}",
            (i + 1) * 10,
            mb(),
            buf,
            mb() - buf,
            stats.encoded_frames.load(Ordering::Relaxed)
        );
    }
    let wall = w0.elapsed().as_secs_f64();
    let cpu = (sysinfo::cpu_time_100ns() - cpu0) as f64 / 1e7;
    let cores = std::thread::available_parallelism().map(|n| n.get()).unwrap_or(1) as f64;
    let enc_fps = (stats.encoded_frames.load(Ordering::Relaxed) - f0) as f64 / wall;
    let cap_fps = (stats.captured_frames.load(Ordering::Relaxed) - c0) as f64 / wall;
    println!(
        "Buffering {wall:.1} s: CPU {:.2} % (aller Kerne) | {:.1} % eines Kerns | encodiert {enc_fps:.1} fps | captured {cap_fps:.1} fps | verworfen {}",
        cpu / wall / cores * 100.0,
        cpu / wall * 100.0,
        stats.dropped_frames.load(Ordering::Relaxed)
    );
    println!(
        "RAM privat: {:.1} MB | Buffer: {:.1} MB / {:.1} s",
        sysinfo::private_bytes() as f64 / 1048576.0,
        stats.buffer_bytes.load(Ordering::Relaxed) as f64 / 1048576.0,
        stats.buffered_us.load(Ordering::Relaxed) as f64 / 1e6
    );
    while let Ok(e) = err_rx.try_recv() {
        println!("Pipeline-Fehler: {e}");
    }

    // Clip speichern (Hotkey → Datei).
    let th = Instant::now();
    let snap = pipeline.router.snapshot(30_000_000).ok_or("Buffer leer")?;
    let clip = out_dir.join("clip.mp4");
    let clip_secs = mp4::write_snapshot(&clip, &pipeline.streams, &snap)?;
    println!(
        "Clip: {clip_secs:.1} s, {:.1} MB in {} ms → {}",
        std::fs::metadata(&clip)?.len() as f64 / 1048576.0,
        th.elapsed().as_millis(),
        clip.display()
    );
    let info = media::probe(&clip)?;
    println!("Probe: {:.2} s {}x{}", info.duration_secs, info.width, info.height);
    let tt = Instant::now();
    media::make_thumbnail(&clip, &out_dir.join("clip.webp"))?;
    println!("Thumbnail in {} ms", tt.elapsed().as_millis());

    // Aufnahme parallel zum Buffer (3 s).
    let (ev_tx, ev_rx) = crossbeam_channel::bounded(8);
    let rec_path = out_dir.join("recording.mp4");
    let rp = rec_path.clone();
    let rec = recorder::Recorder::start(
        pipeline.streams.clone(),
        recorder::RecorderConfig { split_bytes: 0, max_duration_us: 0 },
        Box::new(move || Ok(rp.clone())),
        ev_tx,
    )?;
    pipeline.router.attach_recorder(Some(rec.tx.clone()));
    pipeline.force_keyframe();
    std::thread::sleep(Duration::from_secs(3));
    pipeline.router.attach_recorder(None);
    rec.finish();
    for ev in ev_rx.try_iter() {
        if let recorder::RecorderEvent::FileDone { path, secs } = ev {
            println!("Aufnahme: {secs:.1} s → {}", path.display());
        }
    }
    println!("Aufnahme-Probe: {:.2} s", media::probe(&rec_path)?.duration_secs);

    // Trimmen (keyframe-genau, ohne Re-Encode).
    let trimmed = out_dir.join("clip_trim.mp4");
    let start = media::trim(&clip, &trimmed, 2.5, 6.0)?;
    println!("Trim: Start auf Keyframe {start:.2} s, Dauer {:.2} s", media::probe(&trimmed)?.duration_secs);

    let tstop = Instant::now();
    pipeline.stop();
    println!("Pipeline gestoppt in {} ms", tstop.elapsed().as_millis());
    Ok(())
}
