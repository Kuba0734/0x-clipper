import { useVirtualizer } from "@tanstack/react-virtual";
import { useDeferredValue, useEffect, useMemo, useRef, useState } from "react";

import { Button } from "@/components/ui/Button";
import { Icon } from "@/components/ui/Icon";
import { Skeleton } from "@/components/ui/Card";
import { Select } from "@/components/ui/Select";
import { useT } from "@/i18n";
import { commands, type ClipInfo } from "@/lib/ipc";
import { formatBytes } from "@/lib/format";
import { usePresence } from "@/lib/anim";
import { useLibrary } from "@/stores/library";
import { ClipThumb } from "./ClipThumb";
import { Player } from "./Player";

const MIN_CARD = 260;
const GAP = 16;

type Sort = "newest" | "largest";

export function Library() {
  const t = useT();
  const { clips, loaded, loading, load } = useLibrary();
  const [query, setQuery] = useState("");
  const [game, setGame] = useState("__all");
  const [sort, setSort] = useState<Sort>("newest");
  const [open, setOpen] = useState<ClipInfo | null>(null);
  // Der zuletzt geöffnete Clip bleibt während der Schließ-Animation sichtbar.
  const [shownClip, setShownClip] = useState<ClipInfo | null>(null);
  if (open && open !== shownClip) setShownClip(open);
  const player = usePresence(open !== null);
  const deferredQuery = useDeferredValue(query);
  // Gestaffelt nur beim ersten Erscheinen, nicht für Zeilen, die beim Scrollen nachrücken.
  const [firstPaint, setFirstPaint] = useState(true);
  useEffect(() => {
    const t = window.setTimeout(() => setFirstPaint(false), 800);
    return () => window.clearTimeout(t);
  }, []);

  useEffect(() => {
    void load();
  }, [load]);

  const games = useMemo(() => [...new Set(clips.map((c) => c.game).filter(Boolean))].sort(), [clips]);
  const filtered = useMemo(() => {
    const q = deferredQuery.trim().toLowerCase();
    const list = clips.filter(
      (c) => (game === "__all" || c.game === game) && (!q || c.fileName.toLowerCase().includes(q) || c.game.toLowerCase().includes(q)),
    );
    if (sort === "largest") return [...list].sort((a, b) => b.sizeBytes - a.sizeBytes);
    return list;
  }, [clips, deferredQuery, game, sort]);
  const totalSize = useMemo(() => clips.reduce((s, c) => s + c.sizeBytes, 0), [clips]);

  // Virtualisiertes Grid: Zeilen virtualisieren, Spalten aus der Containerbreite.
  const scrollRef = useRef<HTMLDivElement>(null);
  const [width, setWidth] = useState(900);
  useEffect(() => {
    const el = scrollRef.current;
    if (!el) return;
    const ro = new ResizeObserver(([e]) => e && setWidth(e.contentRect.width));
    ro.observe(el);
    return () => ro.disconnect();
  }, []);
  const cols = Math.max(1, Math.floor((width + GAP) / (MIN_CARD + GAP)));
  const cardW = (width - GAP * (cols - 1)) / cols;
  const rowH = (cardW * 9) / 16 + 64 + GAP;
  const rows = Math.ceil(filtered.length / cols);
  // eslint-disable-next-line react-hooks/incompatible-library -- TanStack Virtual verwaltet Messwerte selbst.
  const virt = useVirtualizer({
    count: rows,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => rowH,
    overscan: 2,
  });
  useEffect(() => virt.measure(), [rowH, virt]);

  return (
    <div className="flex h-full flex-col px-8 pt-7">
      <div className="mb-5 flex items-end justify-between gap-4">
        <div>
          <h1 className="text-[26px] font-semibold tracking-tight">{t("library.title")}</h1>
          <p className="tabular text-[13.5px] text-muted">
            {t("library.count", { n: clips.length, size: formatBytes(totalSize) })}
          </p>
        </div>
        <div className="flex items-center gap-2">
          <div className="relative">
            <Icon name="search" size={16} className="pointer-events-none absolute top-1/2 left-3 -translate-y-1/2 text-muted" />
            <input
              value={query}
              onChange={(e) => setQuery(e.target.value)}
              placeholder={t("library.search")}
              className="h-9 w-56 rounded-sm border border-border bg-surface pr-3 pl-9 text-[13px] outline-none placeholder:text-subtle focus-visible:border-border-strong"
            />
          </div>
          <Select
            className="w-40"
            value={game}
            label={t("dashboard.game")}
            onChange={setGame}
            options={[{ value: "__all", label: t("library.allGames") }, ...games.map((g) => ({ value: g, label: g }))]}
          />
          <Select
            className="w-40"
            value={sort}
            onChange={setSort}
            options={[
              { value: "newest", label: t("library.sortNewest") },
              { value: "largest", label: t("library.sortLargest") },
            ]}
          />
          <Button icon={<Icon name="folder" size={16} />} onClick={() => void commands.openClipsFolder()}>
            {t("library.openFolder")}
          </Button>
        </div>
      </div>

      <div ref={scrollRef} className="-mx-2 min-h-0 flex-1 overflow-y-auto px-2 pb-8">
        {!loaded && loading ? (
          <div className="grid gap-4" style={{ gridTemplateColumns: `repeat(${cols}, minmax(0, 1fr))` }}>
            {Array.from({ length: cols * 2 }, (_, i) => (
              <div key={i}>
                <Skeleton className="aspect-video w-full" />
                <Skeleton className="mt-2 h-4 w-2/3" />
              </div>
            ))}
          </div>
        ) : filtered.length === 0 ? (
          <div className="grid h-60 place-items-center text-[13.5px] text-muted">{t("library.empty")}</div>
        ) : (
          <div style={{ height: virt.getTotalSize(), position: "relative" }} className="anim-fade-in">
            {virt.getVirtualItems().map((row) => (
              <div
                key={row.key}
                className="absolute inset-x-0 top-0 grid"
                style={{
                  transform: `translateY(${row.start}px)`,
                  gridTemplateColumns: `repeat(${cols}, minmax(0, 1fr))`,
                  gap: GAP,
                }}
              >
                {filtered.slice(row.index * cols, row.index * cols + cols).map((c, col) => (
                  <ClipThumb
                    key={c.path}
                    clip={c}
                    index={firstPaint ? row.index * cols + col : undefined}
                    onOpen={() => setOpen(c)}
                  />
                ))}
              </div>
            ))}
          </div>
        )}
      </div>

      {player.mounted && shownClip && (
        <Player key={shownClip.path} clip={shownClip} closing={player.closing} onClose={() => setOpen(null)} />
      )}
    </div>
  );
}
