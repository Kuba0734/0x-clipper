//! AAC-Encoder (FFmpeg nativ, LGPL), 48 kHz Stereo.

use std::ptr;

use ffmpeg_sys_next as ff;

use super::wasapi::{CHANNELS, SAMPLE_RATE};
use crate::encode::ffi::{check, CodecParams, OwnedFrame, OwnedPacket, AVERROR_EAGAIN};
use crate::error::{CoreError, CoreResult};

/// Empfänger encodierter Pakete: (pts, dts, duration) in Samples und die Daten.
pub type AacSink<'a> = dyn FnMut(i64, i64, i64, &[u8]) + 'a;

pub struct AacEncoder {
    ctx: *mut ff::AVCodecContext,
    frame: OwnedFrame,
    pkt: OwnedPacket,
    pub frame_size: usize,
}

// SAFETY: Wird nur vom Audio-Engine-Thread benutzt.
unsafe impl Send for AacEncoder {}

impl AacEncoder {
    pub fn new(bitrate_kbps: u32) -> CoreResult<Self> {
        // SAFETY: Alle Zeiger werden geprüft; Aufräumen in Drop.
        unsafe {
            let codec = ff::avcodec_find_encoder(ff::AVCodecID::AV_CODEC_ID_AAC);
            if codec.is_null() {
                return Err(CoreError::Audio("AAC-Encoder fehlt".into()));
            }
            let ctx = ff::avcodec_alloc_context3(codec);
            if ctx.is_null() {
                return Err(CoreError::Audio("avcodec_alloc_context3".into()));
            }
            let mut me =
                AacEncoder { ctx, frame: OwnedFrame::alloc()?, pkt: OwnedPacket::alloc()?, frame_size: 1024 };
            let c = &mut *ctx;
            c.sample_fmt = ff::AVSampleFormat::AV_SAMPLE_FMT_FLTP;
            c.sample_rate = SAMPLE_RATE as i32;
            ff::av_channel_layout_default(&mut c.ch_layout, CHANNELS as i32);
            c.bit_rate = bitrate_kbps as i64 * 1000;
            c.time_base = ff::AVRational { num: 1, den: SAMPLE_RATE as i32 };
            c.flags |= ff::AV_CODEC_FLAG_GLOBAL_HEADER as i32;
            check(ff::avcodec_open2(ctx, codec, ptr::null_mut()), "AAC öffnen")?;
            me.frame_size = if c.frame_size > 0 { c.frame_size as usize } else { 1024 };
            let f = &mut *me.frame.0;
            f.format = ff::AVSampleFormat::AV_SAMPLE_FMT_FLTP as i32;
            f.nb_samples = me.frame_size as i32;
            f.sample_rate = SAMPLE_RATE as i32;
            check(ff::av_channel_layout_copy(&mut f.ch_layout, &c.ch_layout), "ch_layout")?;
            check(ff::av_frame_get_buffer(me.frame.0, 0), "av_frame_get_buffer")?;
            Ok(me)
        }
    }

    pub fn params(&self) -> CoreResult<CodecParams> {
        // SAFETY: ctx ist geöffnet.
        unsafe { CodecParams::from_context(self.ctx) }
    }

    /// Encodiert genau `frame_size` interleavte Stereo-Frames. `pts` in Samples.
    /// Der Sink bekommt (pts, dts, duration) in Samples und die Paketdaten.
    pub fn encode(
        &mut self,
        interleaved: &[f32],
        pts: i64,
        sink: &mut AacSink,
    ) -> CoreResult<()> {
        let n = self.frame_size;
        if interleaved.len() < n * CHANNELS {
            return Err(CoreError::Audio("Zu wenige Samples".into()));
        }
        // SAFETY: Frame hat zwei Planes à n f32 (FLTP, Stereo); make_writable stellt sicher,
        // dass der Encoder keine Referenz mehr hält.
        unsafe {
            check(ff::av_frame_make_writable(self.frame.0), "make_writable")?;
            let f = &mut *self.frame.0;
            let l = std::slice::from_raw_parts_mut(f.data[0] as *mut f32, n);
            let r = std::slice::from_raw_parts_mut(f.data[1] as *mut f32, n);
            for (i, s) in interleaved.chunks_exact(CHANNELS).take(n).enumerate() {
                l[i] = s[0];
                r[i] = s[1];
            }
            f.pts = pts;
            let res = ff::avcodec_send_frame(self.ctx, self.frame.0);
            if res < 0 && res != AVERROR_EAGAIN {
                check(res, "AAC send_frame")?;
            }
        }
        self.drain(sink)
    }

    fn drain(&mut self, sink: &mut AacSink) -> CoreResult<()> {
        loop {
            // SAFETY: ctx/pkt gültig.
            let r = unsafe { ff::avcodec_receive_packet(self.ctx, self.pkt.0) };
            if r == AVERROR_EAGAIN || r == ff::AVERROR_EOF {
                return Ok(());
            }
            check(r, "AAC receive_packet")?;
            // SAFETY: Paket ist bis unref gültig.
            let (pts, dts, dur) = unsafe { ((*self.pkt.0).pts, (*self.pkt.0).dts, (*self.pkt.0).duration) };
            sink(pts, dts, dur, self.pkt.data());
            self.pkt.unref();
        }
    }
}

impl Drop for AacEncoder {
    fn drop(&mut self) {
        // SAFETY: Eigener Kontext.
        unsafe { ff::avcodec_free_context(&mut self.ctx) };
    }
}
