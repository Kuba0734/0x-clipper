import { m } from "motion/react";
import { FolderSearch, Scissors, Trash2, X } from "lucide-react";
import { useEffect, useRef, useState } from "react";

import { Button, IconButton } from "@/components/ui/Button";
import { useLang, useT } from "@/i18n";
import { commands, errorMessage, fileSrc, type ClipInfo } from "@/lib/ipc";
import { formatBytes, formatDate, formatDuration } from "@/lib/format";
import { spring } from "@/lib/motion";
import { useLibrary } from "@/stores/library";
import { useToasts } from "@/stores/toasts";

/** Zwei-Griff-Bereich für Start/Ende; Griffe per translateX, Auswahl per scaleX. */
function RangeBar({
  duration,
  start,
  end,
  current,
  onChange,
  onSeek,
}: {
  duration: number;
  start: number;
  end: number;
  current: number;
  onChange: (s: number, e: number) => void;
  onSeek: (t: number) => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const pct = (v: number) => (duration > 0 ? (v / duration) * 100 : 0);
  const drag = (which: "start" | "end") => (e: React.PointerEvent) => {
    e.preventDefault();
    const el = ref.current;
    if (!el) return;
    const rect = el.getBoundingClientRect();
    const move = (ev: PointerEvent) => {
      const t = Math.max(0, Math.min(duration, ((ev.clientX - rect.left) / rect.width) * duration));
      if (which === "start") onChange(Math.min(t, end - 0.5), end);
      else onChange(start, Math.max(t, start + 0.5));
    };
    const up = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up);
  };
  const seek = (e: React.MouseEvent) => {
    const rect = e.currentTarget.getBoundingClientRect();
    onSeek(((e.clientX - rect.left) / rect.width) * duration);
  };
  return (
    <div ref={ref} className="relative h-10 select-none" onClick={seek}>
      <div className="absolute inset-x-0 top-1/2 h-2 -translate-y-1/2 rounded-full bg-surface-2" />
      <div
        className="absolute top-1/2 left-0 h-2 w-full origin-left -translate-y-1/2 rounded-full bg-gradient-accent opacity-80"
        style={{ transform: `translateX(${pct(start)}%) scaleX(${(end - start) / Math.max(duration, 0.001)})` }}
      />
      <div className="pointer-events-none absolute inset-y-0 left-0 w-full" style={{ transform: `translateX(${pct(current)}%)` }}>
        <span className="absolute top-1 bottom-1 left-0 w-0.5 -translate-x-1/2 rounded bg-white" />
      </div>
      {(["start", "end"] as const).map((w) => (
        <div
          key={w}
          className="pointer-events-none absolute inset-y-0 left-0 w-full"
          style={{ transform: `translateX(${pct(w === "start" ? start : end)}%)` }}
        >
          <span
            onPointerDown={drag(w)}
            onClick={(e) => e.stopPropagation()}
            role="slider"
            aria-label={w}
            aria-valuenow={w === "start" ? start : end}
            className="pointer-events-auto absolute top-1/2 left-0 h-7 w-3 -translate-x-1/2 -translate-y-1/2 cursor-ew-resize rounded-[4px] bg-white shadow-md"
          />
        </div>
      ))}
    </div>
  );
}

export function Player({ clip, onClose }: { clip: ClipInfo; onClose: () => void }) {
  const t = useT();
  const lang = useLang();
  const video = useRef<HTMLVideoElement>(null);
  const [duration, setDuration] = useState(clip.durationSecs);
  const [range, setRange] = useState<[number, number]>([0, clip.durationSecs]);
  const [current, setCurrent] = useState(0);
  const [busy, setBusy] = useState(false);
  const [confirmDelete, setConfirmDelete] = useState(false);
  const { upsert, remove } = useLibrary();
  const push = useToasts((s) => s.push);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);

  // Wiedergabe innerhalb des Trim-Bereichs halten.
  const onTime = () => {
    const v = video.current;
    if (!v) return;
    setCurrent(v.currentTime);
    if (v.currentTime > range[1]) v.currentTime = range[0];
  };

  const trim = async (replace: boolean) => {
    setBusy(true);
    try {
      const out = await commands.trimClip(clip.path, range[0], range[1], replace);
      upsert(out);
      push({ kind: "success", title: t("library.trimmed"), detail: out.fileName });
      onClose();
    } catch (e) {
      push({ kind: "error", title: errorMessage(e) });
    } finally {
      setBusy(false);
    }
  };

  const del = async () => {
    if (!confirmDelete) return setConfirmDelete(true);
    try {
      // Video freigeben, damit die Datei nicht gesperrt ist.
      video.current?.removeAttribute("src");
      video.current?.load();
      await commands.deleteClip(clip.path);
      remove(clip.path);
      push({ kind: "info", title: t("library.deleted") });
      onClose();
    } catch (e) {
      push({ kind: "error", title: errorMessage(e) });
    }
  };

  const trimmed = range[0] > 0.05 || range[1] < duration - 0.05;
  return (
    <m.div
      initial={{ opacity: 0 }}
      animate={{ opacity: 1 }}
      exit={{ opacity: 0 }}
      transition={{ duration: 0.18 }}
      className="fixed inset-0 top-10 z-40 grid place-items-center bg-black/60 p-8 backdrop-blur-sm"
      onClick={onClose}
    >
      <m.div
        initial={{ opacity: 0, scale: 0.96, y: 12 }}
        animate={{ opacity: 1, scale: 1, y: 0 }}
        exit={{ opacity: 0, scale: 0.97, y: 8 }}
        transition={spring}
        onClick={(e) => e.stopPropagation()}
        className="flex max-h-full w-full max-w-5xl flex-col overflow-hidden rounded-card border border-border bg-surface shadow-2xl"
        role="dialog"
        aria-label={clip.fileName}
      >
        <div className="flex items-center justify-between border-b border-border px-4 py-3">
          <div className="min-w-0">
            <div className="truncate text-[14px] font-semibold">{clip.fileName}</div>
            <div className="tabular text-[12px] text-muted">
              {formatDate(clip.modifiedMs, lang)} · {formatBytes(clip.sizeBytes)} · {clip.width}×{clip.height}
            </div>
          </div>
          <IconButton label={t("library.close")} onClick={onClose}>
            <X className="size-4" />
          </IconButton>
        </div>
        <video
          ref={video}
          src={fileSrc(clip.path)}
          controls
          autoPlay
          onLoadedMetadata={(e) => {
            const d = e.currentTarget.duration;
            if (Number.isFinite(d)) {
              setDuration(d);
              setRange([0, d]);
            }
          }}
          onTimeUpdate={onTime}
          className="max-h-[60vh] w-full bg-black"
        />
        <div className="space-y-3 px-4 py-4">
          <div className="flex items-center gap-3">
            <Scissors className="size-4 text-muted" />
            <div className="flex-1">
              <RangeBar
                duration={duration}
                start={range[0]}
                end={range[1]}
                current={current}
                onChange={(s, e) => {
                  setRange([s, e]);
                  if (video.current) video.current.currentTime = s;
                }}
                onSeek={(tm) => video.current && (video.current.currentTime = tm)}
              />
            </div>
            <span className="tabular w-36 text-right text-[12.5px] text-muted">
              {formatDuration(range[0])} – {formatDuration(range[1])}
            </span>
          </div>
          <p className="text-[12px] text-muted">{t("library.trimHint")}</p>
          <div className="flex items-center justify-between gap-2">
            <div className="flex gap-2">
              <Button size="sm" icon={<FolderSearch className="size-4" />} onClick={() => void commands.revealClip(clip.path)}>
                {t("library.reveal")}
              </Button>
              <Button size="sm" variant={confirmDelete ? "danger" : "ghost"} icon={<Trash2 className="size-4" />} onClick={() => void del()}>
                {confirmDelete ? t("library.deleteConfirm") : t("library.delete")}
              </Button>
            </div>
            <div className="flex gap-2">
              <Button size="sm" disabled={!trimmed || busy} onClick={() => void trim(true)}>
                {t("library.trimReplace")}
              </Button>
              <Button size="sm" variant="primary" disabled={!trimmed || busy} onClick={() => void trim(false)}>
                {t("library.trimSave")}
              </Button>
            </div>
          </div>
        </div>
      </m.div>
    </m.div>
  );
}
