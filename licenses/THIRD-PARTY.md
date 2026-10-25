# Drittanbieter-Lizenzen / Third-party licenses

## FFmpeg (LGPL 2.1 oder später)

0x-clipper verwendet FFmpeg-Bibliotheken (avcodec, avformat, avutil, swresample, swscale)
als **dynamisch gelinkte DLLs**. Es handelt sich um einen LGPL-Build ohne GPL- oder
nonfree-Komponenten (insbesondere **ohne libx264**). Der Software-Fallback nutzt den
Media-Foundation-Encoder von Windows (`h264_mf`).

- Build: BtbN FFmpeg-Builds, `ffmpeg-n9.0-latest-win64-lgpl-shared`
- Quellcode: https://ffmpeg.org/download.html bzw. https://github.com/BtbN/FFmpeg-Builds
- Lizenztext: `licenses/FFmpeg-LGPL.txt`

Sie dürfen die DLLs durch eigene, kompatible FFmpeg-Builds ersetzen.

## Weitere Komponenten

- Tauri, tao, wry (Apache-2.0 / MIT)
- windows-rs (Apache-2.0 / MIT)
- React, Motion, Zustand, @tanstack/react-virtual, lucide-react (MIT; lucide: ISC)
- Inter (SIL Open Font License 1.1)
- Microsoft Edge WebView2 Runtime (Microsoft Software License)
