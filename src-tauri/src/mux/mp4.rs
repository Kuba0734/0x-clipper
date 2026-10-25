//! MP4-Writer für bereits encodierte Pakete (Remux, kein Re-Encode).

use std::path::{Path, PathBuf};
use std::ptr;

use ffmpeg_sys_next as ff;

use crate::buffer::Packet;
use crate::encode::ffi::{check, cstr, OwnedPacket, StreamSet, US_TB};
use crate::error::{CoreError, CoreResult};

pub struct Mp4Writer {
    ctx: *mut ff::AVFormatContext,
    tbs: Vec<ff::AVRational>,
    last_dts: Vec<i64>,
    pkt: OwnedPacket,
    base_us: i64,
    pub bytes: u64,
    pub path: PathBuf,
    finished: bool,
    first_us: Option<i64>,
    last_us: i64,
}

// SAFETY: Der Writer gehört genau einem Thread (MuxWorker bzw. Recorder).
unsafe impl Send for Mp4Writer {}

impl Mp4Writer {
    /// Legt die Datei an und schreibt den Header. `base_us` wird zu Zeit 0.
    pub fn create(path: &Path, streams: &StreamSet, base_us: i64) -> CoreResult<Self> {
        let path_c = cstr(&path.to_string_lossy())?;
        let fmt = cstr("mp4")?;
        let mut ctx: *mut ff::AVFormatContext = ptr::null_mut();
        // SAFETY: Ausgabezeiger und nullterminierte Strings sind gültig.
        check(
            unsafe { ff::avformat_alloc_output_context2(&mut ctx, ptr::null(), fmt.as_ptr(), path_c.as_ptr()) },
            "avformat_alloc_output_context2",
        )?;
        let mut me = Mp4Writer {
            ctx,
            tbs: Vec::new(),
            last_dts: Vec::new(),
            pkt: OwnedPacket::alloc()?,
            base_us,
            bytes: 0,
            path: path.to_path_buf(),
            finished: false,
            first_us: None,
            last_us: base_us,
        };
        // SAFETY: ctx ist gültig; Streams werden vom Kontext besessen.
        unsafe {
            for params in std::iter::once(&streams.video).chain(streams.audio.iter()) {
                let st = ff::avformat_new_stream(ctx, ptr::null());
                if st.is_null() {
                    return Err(CoreError::Ffmpeg("avformat_new_stream".into()));
                }
                check(ff::avcodec_parameters_copy((*st).codecpar, params.as_ptr()), "codecpar")?;
                (*(*st).codecpar).codec_tag = 0;
                (*st).time_base = if params.is_video() {
                    ff::AVRational { num: 1, den: 90_000 }
                } else {
                    ff::AVRational { num: 1, den: (*params.as_ptr()).sample_rate.max(1) }
                };
                if params.is_video() {
                    (*st).avg_frame_rate = ff::AVRational { num: streams.fps as i32, den: 1 };
                }
            }
            check(ff::avio_open(&mut (*ctx).pb, path_c.as_ptr(), ff::AVIO_FLAG_WRITE), "Datei öffnen")?;
            check(ff::avformat_write_header(ctx, ptr::null_mut()), "MP4-Header")?;
            // Der Muxer darf die Zeitbasen anpassen – danach auslesen.
            let n = (*ctx).nb_streams as usize;
            for i in 0..n {
                me.tbs.push((**(*ctx).streams.add(i)).time_base);
            }
            me.last_dts = vec![i64::MIN; n];
        }
        Ok(me)
    }

    pub fn write(&mut self, p: &Packet) -> CoreResult<()> {
        let idx = p.stream as usize;
        let Some(&tb) = self.tbs.get(idx) else { return Ok(()) };
        let rescale = |v: i64| {
            // SAFETY: Reine Arithmetik.
            unsafe { ff::av_rescale_q(v, US_TB, tb) }
        };
        let mut dts = rescale(p.dts - self.base_us);
        let mut pts = rescale(p.pts - self.base_us);
        let last = self.last_dts[idx];
        if dts <= last {
            dts = last + 1;
        }
        if pts < dts {
            pts = dts;
        }
        self.last_dts[idx] = dts;
        self.first_us.get_or_insert(p.pts);
        self.last_us = self.last_us.max(p.pts + p.duration);
        // SAFETY: Das Paket zeigt nicht-besitzend auf p.data; av_write_frame übernimmt
        // keine Ownership und kopiert bei Bedarf. p lebt über den Aufruf hinaus.
        unsafe {
            let k = &mut *self.pkt.0;
            k.data = p.data.as_ptr() as *mut u8;
            k.size = p.data.len() as i32;
            k.stream_index = idx as i32;
            k.pts = pts;
            k.dts = dts;
            k.duration = rescale(p.duration);
            k.flags = if p.key { ff::AV_PKT_FLAG_KEY } else { 0 };
            k.pos = -1;
            let r = ff::av_write_frame(self.ctx, self.pkt.0);
            k.data = ptr::null_mut();
            k.size = 0;
            check(r, "av_write_frame")?;
        }
        self.bytes += p.data.len() as u64;
        Ok(())
    }

    pub fn base_us(&self) -> i64 {
        self.base_us
    }

    /// Dauer des bisher Geschriebenen in Sekunden.
    pub fn duration_secs(&self) -> f64 {
        (self.last_us - self.first_us.unwrap_or(self.base_us)).max(0) as f64 / 1e6
    }

    pub fn finish(mut self) -> CoreResult<PathBuf> {
        self.finished = true;
        // SAFETY: Header wurde geschrieben; Trailer und Schließen genau einmal.
        unsafe {
            check(ff::av_write_trailer(self.ctx), "MP4-Trailer")?;
            ff::avio_closep(&mut (*self.ctx).pb);
        }
        Ok(self.path.clone())
    }
}

impl Drop for Mp4Writer {
    fn drop(&mut self) {
        // SAFETY: Eigener Kontext; pb wird geschlossen, falls finish() nicht lief.
        unsafe {
            if !self.finished && !(*self.ctx).pb.is_null() {
                ff::avio_closep(&mut (*self.ctx).pb);
            }
            ff::avformat_free_context(self.ctx);
        }
        if !self.finished {
            // Unvollständige Datei ohne moov ist unbrauchbar.
            let _ = std::fs::remove_file(&self.path);
        }
    }
}

/// Schreibt einen Snapshot des Replay-Buffers als MP4.
pub fn write_snapshot(path: &Path, streams: &StreamSet, snap: &crate::buffer::Snapshot) -> CoreResult<f64> {
    let mut w = Mp4Writer::create(path, streams, snap.start_pts)?;
    for p in &snap.packets {
        if (p.stream as usize) <= streams.audio.len() {
            w.write(p)?;
        }
    }
    let secs = w.duration_secs();
    w.finish()?;
    Ok(secs)
}
