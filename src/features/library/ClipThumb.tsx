import { useEffect, useRef, useState } from "react";

import { Card } from "@/components/ui/Card";
import { Icon } from "@/components/ui/Icon";
import { useLang } from "@/i18n";
import { formatBytes, formatDate, formatDuration } from "@/lib/format";
import { commands, fileSrc, type ClipInfo } from "@/lib/ipc";
import { useLibrary } from "@/stores/library";

interface Props {
  clip: ClipInfo;
  onOpen: () => void;
  compact?: boolean;
  /** Position für gestaffeltes Einblenden. */
  index?: number;
}

/**
 * Thumbnail-Karte. Fehlende Thumbnails erzeugt der Core (WebP, gecacht). Hover startet
 * nach kurzer Verzögerung eine stumme Vorschau, die per opacity einblendet und beim
 * Verlassen wieder entladen wird (kein Decoder läuft im Hintergrund weiter).
 */
export function ClipThumb({ clip, onOpen, compact, index }: Props) {
  const lang = useLang();
  const [preview, setPreview] = useState(false);
  const [ready, setReady] = useState(false);
  const [thumbLoaded, setThumbLoaded] = useState(false);
  const timer = useRef<number | undefined>(undefined);
  const setThumb = useLibrary((s) => s.setThumb);

  useEffect(() => {
    if (clip.thumbnail) return;
    let alive = true;
    commands
      .getThumbnail(clip.path)
      .then((p) => alive && setThumb(clip.path, p))
      .catch(() => {});
    return () => {
      alive = false;
    };
  }, [clip.path, clip.thumbnail, setThumb]);

  useEffect(() => () => window.clearTimeout(timer.current), []);

  return (
    <Card
      interactive
      index={index}
      onMouseEnter={() => {
        window.clearTimeout(timer.current);
        timer.current = window.setTimeout(() => setPreview(true), 450);
      }}
      onMouseLeave={() => {
        window.clearTimeout(timer.current);
        setPreview(false);
        setReady(false);
      }}
      onClick={onOpen}
      className="group overflow-hidden"
    >
      <div className="relative aspect-video overflow-hidden rounded-t-[inherit] bg-surface-2">
        {!thumbLoaded && <div className="skeleton absolute inset-0" />}
        {clip.thumbnail && (
          <img
            src={fileSrc(clip.thumbnail)}
            alt=""
            loading="lazy"
            decoding="async"
            draggable={false}
            onLoad={() => setThumbLoaded(true)}
            className="fade absolute inset-0 size-full object-cover"
            style={{ opacity: thumbLoaded ? 1 : 0 }}
          />
        )}
        {preview && (
          <video
            src={fileSrc(clip.path)}
            muted
            autoPlay
            loop
            playsInline
            preload="metadata"
            onPlaying={() => setReady(true)}
            className="fade absolute inset-0 size-full object-cover"
            style={{ opacity: ready ? 1 : 0 }}
          />
        )}
        <div className="fade pointer-events-none absolute inset-0 grid place-items-center opacity-0 group-hover:opacity-100">
          <span className="grid size-10 place-items-center rounded-full bg-black/55 text-white">
            <Icon name="play" size={16} fill="currentColor" strokeWidth={0} />
          </span>
        </div>
        <span className="tabular absolute right-2 bottom-2 rounded-[6px] bg-black/70 px-1.5 py-0.5 text-[11px] font-medium">
          {formatDuration(clip.durationSecs)}
        </span>
      </div>
      <div className={compact ? "px-3 py-2.5" : "px-3.5 py-3"}>
        <div className="flex items-center gap-1.5 truncate text-[13px] font-medium">
          <Icon name="film" size={14} className="text-muted" />
          <span className="truncate">{clip.fileName.replace(/\.mp4$/i, "")}</span>
        </div>
        {!compact && (
          <div className="tabular mt-1 flex justify-between text-[11.5px] text-muted">
            <span className="truncate">{clip.game || formatDate(clip.modifiedMs, lang)}</span>
            <span>{formatBytes(clip.sizeBytes)}</span>
          </div>
        )}
      </div>
    </Card>
  );
}
