import { AnimatePresence, m } from "motion/react";
import { Film, Play } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { Card } from "@/components/ui/Card";
import { useLang } from "@/i18n";
import { commands, fileSrc, type ClipInfo } from "@/lib/ipc";
import { formatBytes, formatDate, formatDuration } from "@/lib/format";
import { useLibrary } from "@/stores/library";

interface Props {
  clip: ClipInfo;
  onOpen: () => void;
  compact?: boolean;
}

/** Thumbnail-Karte. Fehlende Thumbnails erzeugt der Core (WebP, gecacht); Hover startet
 *  nach kurzer Verzögerung eine stumme Vorschau, die beim Verlassen wieder entladen wird. */
export function ClipThumb({ clip, onOpen, compact }: Props) {
  const lang = useLang();
  const [preview, setPreview] = useState(false);
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
      onHoverStart={() => {
        window.clearTimeout(timer.current);
        timer.current = window.setTimeout(() => setPreview(true), 450);
      }}
      onHoverEnd={() => {
        window.clearTimeout(timer.current);
        setPreview(false);
      }}
      onClick={onOpen}
      className="group cursor-pointer overflow-hidden"
    >
      <div className="relative aspect-video overflow-hidden bg-surface-2">
        {clip.thumbnail ? (
          <img src={fileSrc(clip.thumbnail)} alt="" loading="lazy" decoding="async" className="size-full object-cover" draggable={false} />
        ) : (
          <div className="skeleton size-full" />
        )}
        <AnimatePresence>
          {preview && (
            <m.video
              key="pv"
              src={fileSrc(clip.path)}
              muted
              autoPlay
              loop
              playsInline
              preload="metadata"
              initial={{ opacity: 0 }}
              animate={{ opacity: 1 }}
              exit={{ opacity: 0 }}
              className="absolute inset-0 size-full object-cover"
            />
          )}
        </AnimatePresence>
        <div className="pointer-events-none absolute inset-0 grid place-items-center opacity-0 transition-opacity duration-200 group-hover:opacity-100">
          <span className="glass flex size-10 items-center justify-center rounded-full">
            <Play className="size-4 fill-white text-white" />
          </span>
        </div>
        <span className="tabular absolute right-2 bottom-2 rounded-[6px] bg-black/70 px-1.5 py-0.5 text-[11px] font-medium">
          {formatDuration(clip.durationSecs)}
        </span>
      </div>
      <div className={compact ? "px-3 py-2" : "px-3.5 py-3"}>
        <div className="flex items-center gap-1.5 truncate text-[13px] font-medium">
          <Film className="size-3.5 shrink-0 text-muted" />
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
