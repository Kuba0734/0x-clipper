//! Dateibasierte Medienfunktionen: Metadaten, Thumbnails (WebP, 320 px), Trimmen per Remux.

use std::path::Path;

use ffmpeg_next as ffn;
use ffn::{codec, encoder, format, media, software::scaling, util::frame::video::Video, Dictionary, Rational};

use crate::error::{CoreError, CoreResult};

pub const THUMB_WIDTH: u32 = 320;

#[derive(Debug, Clone, Copy, Default)]
pub struct MediaInfo {
    pub duration_secs: f64,
    pub width: u32,
    pub height: u32,
}

/// Liest nur den Header (moov) – schnell genug für die Bibliotheksliste.
pub fn probe(path: &Path) -> CoreResult<MediaInfo> {
    let ictx = format::input(path)?;
    let duration = ictx.duration().max(0) as f64 / f64::from(ffn::ffi::AV_TIME_BASE);
    let (width, height) = ictx
        .streams()
        .best(media::Type::Video)
        .map(|s| {
            let p = s.parameters();
            // SAFETY: Parameter gehören zum offenen Kontext und leben in diesem Block.
            unsafe { ((*p.as_ptr()).width.max(0) as u32, (*p.as_ptr()).height.max(0) as u32) }
        })
        .unwrap_or((0, 0));
    Ok(MediaInfo { duration_secs: duration, width, height })
}

/// Dekodiert den ersten Frame und speichert ihn als WebP.
pub fn make_thumbnail(input: &Path, output: &Path) -> CoreResult<()> {
    let mut ictx = format::input(input)?;
    let stream = ictx
        .streams()
        .best(media::Type::Video)
        .ok_or_else(|| CoreError::Other("Kein Videostream".into()))?;
    let idx = stream.index();
    let mut ctx = codec::context::Context::from_parameters(stream.parameters())?;
    // Nur ein Frame: ein Thread reicht und hält RAM klein.
    ctx.set_threading(codec::threading::Config { kind: codec::threading::Type::Frame, count: 1 });
    let mut dec = ctx.decoder().video()?;
    let mut frame = Video::empty();
    let mut got = false;
    for (n, (s, pkt)) in ictx.packets().enumerate() {
        if n > 300 {
            break;
        }
        if s.index() != idx {
            continue;
        }
        dec.send_packet(&pkt)?;
        if dec.receive_frame(&mut frame).is_ok() {
            got = true;
            break;
        }
    }
    if !got {
        dec.send_eof()?;
        got = dec.receive_frame(&mut frame).is_ok();
    }
    if !got || frame.width() == 0 {
        return Err(CoreError::Other("Kein Frame dekodierbar".into()));
    }
    let w = THUMB_WIDTH.min(frame.width()) & !1;
    let h = ((u64::from(w) * u64::from(frame.height()) / u64::from(frame.width())) as u32).max(2) & !1;
    let mut scaler = scaling::Context::get(
        frame.format(),
        frame.width(),
        frame.height(),
        format::Pixel::YUV420P,
        w,
        h,
        scaling::Flags::BILINEAR,
    )?;
    let mut scaled = Video::empty();
    scaler.run(&frame, &mut scaled)?;

    let webp = encoder::find_by_name("libwebp").ok_or_else(|| CoreError::Other("libwebp fehlt".into()))?;
    let mut enc = codec::context::Context::new_with_codec(webp).encoder().video()?;
    enc.set_width(w);
    enc.set_height(h);
    enc.set_format(format::Pixel::YUV420P);
    enc.set_time_base(Rational::new(1, 1));
    let mut opts = Dictionary::new();
    opts.set("quality", "72");
    opts.set("preset", "picture");
    let mut enc = enc.open_with(opts)?;
    scaled.set_pts(Some(0));
    enc.send_frame(&scaled)?;
    enc.send_eof()?;
    let mut pkt = ffn::Packet::empty();
    enc.receive_packet(&mut pkt)?;
    let data = pkt.data().ok_or_else(|| CoreError::Other("Leeres WebP".into()))?;
    if let Some(dir) = output.parent() {
        std::fs::create_dir_all(dir)?;
    }
    let tmp = output.with_extension("webp.tmp");
    std::fs::write(&tmp, data)?;
    std::fs::rename(&tmp, output)?;
    Ok(())
}

/// Schneidet `[start, end]` ohne Re-Encode heraus. Der Start rastet auf den Keyframe
/// davor ein; Rückgabe ist der tatsächliche Start in Sekunden.
pub fn trim(input: &Path, output: &Path, start_secs: f64, end_secs: f64) -> CoreResult<f64> {
    if end_secs <= start_secs {
        return Err(CoreError::Other("Ende liegt vor dem Start".into()));
    }
    let mut ictx = format::input(input)?;
    let mut octx = format::output_as(output, "mp4")?;
    let n = ictx.nb_streams() as usize;
    let mut map: Vec<Option<usize>> = vec![None; n];
    let mut in_tb: Vec<Rational> = vec![Rational::new(1, 1); n];
    let mut video_idx = None;
    let mut out_idx = 0;
    for (i, ist) in ictx.streams().enumerate() {
        let medium = ist.parameters().medium();
        if medium != media::Type::Video && medium != media::Type::Audio {
            continue;
        }
        if medium == media::Type::Video && video_idx.is_none() {
            video_idx = Some(i);
        }
        map[i] = Some(out_idx);
        in_tb[i] = ist.time_base();
        out_idx += 1;
        let mut ost = octx.add_stream(encoder::find(codec::Id::None))?;
        ost.set_parameters(ist.parameters());
        // SAFETY: Parameter des gerade angelegten Ausgabestreams.
        unsafe { (*ost.parameters().as_mut_ptr()).codec_tag = 0 };
    }
    let video_idx = video_idx.ok_or_else(|| CoreError::Other("Kein Videostream".into()))?;
    octx.write_header()?;
    let out_tb: Vec<Rational> = octx.streams().map(|s| s.time_base()).collect();

    let to_av = |s: f64| (s * f64::from(ffn::ffi::AV_TIME_BASE)) as i64;
    // Rückwärts auf den Keyframe ≤ Start.
    ictx.seek(to_av(start_secs), ..to_av(start_secs))?;

    let secs = |ts: i64, tb: Rational| ts as f64 * f64::from(tb.numerator()) / f64::from(tb.denominator());
    let mut base: Option<f64> = None;
    for (s, mut pkt) in ictx.packets() {
        let i = s.index();
        let Some(o) = map[i] else { continue };
        let Some(ts) = pkt.pts().or(pkt.dts()) else { continue };
        let t = secs(ts, in_tb[i]);
        if base.is_none() {
            if i != video_idx || !pkt.is_key() {
                continue;
            }
            base = Some(t);
        }
        let b = base.unwrap_or(0.0);
        if i == video_idx && t > end_secs {
            break;
        }
        if t < b || t > end_secs {
            continue;
        }
        // Zeitstempel auf den neuen Start verschieben.
        let shift = (b * f64::from(in_tb[i].denominator()) / f64::from(in_tb[i].numerator())).round() as i64;
        pkt.set_pts(pkt.pts().map(|v| v - shift));
        pkt.set_dts(pkt.dts().map(|v| v - shift));
        pkt.rescale_ts(in_tb[i], out_tb[o]);
        pkt.set_position(-1);
        pkt.set_stream(o);
        pkt.write_interleaved(&mut octx)?;
    }
    octx.write_trailer()?;
    base.ok_or_else(|| CoreError::Other("Kein Keyframe im Bereich".into()))
}
