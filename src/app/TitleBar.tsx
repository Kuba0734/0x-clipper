import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useState } from "react";

import { Icon } from "@/components/ui/Icon";
import { useT } from "@/i18n";
import { StatusPill } from "./StatusPill";

/** Eigene Titelleiste (decorations: false) mit Drag-Region und Fensterbuttons. */
export function TitleBar() {
  const t = useT();
  const win = getCurrentWindow();
  const [maximized, setMaximized] = useState(false);

  useEffect(() => {
    let un: (() => void) | undefined;
    void win.isMaximized().then(setMaximized);
    void win.onResized(() => void win.isMaximized().then(setMaximized)).then((f) => (un = f));
    return () => un?.();
  }, [win]);

  const btn = "hoverable flex h-full w-12 items-center justify-center text-muted hover:text-text";
  return (
    <header
      data-tauri-drag-region
      onDoubleClick={() => void win.toggleMaximize()}
      className="drag-region flex h-10 shrink-0 items-center border-b border-border pl-4"
    >
      <div data-tauri-drag-region className="flex items-center gap-2.5">
        <span className="grid size-4 place-items-center rounded-[5px] bg-accent text-on-accent" aria-hidden>
          <Icon name="play" size={10} strokeWidth={0} fill="currentColor" />
        </span>
        <span data-tauri-drag-region className="text-[13px] font-semibold tracking-tight">
          0x-clipper
        </span>
      </div>
      <div data-tauri-drag-region className="flex-1" />
      <div className="no-drag mr-2">
        <StatusPill />
      </div>
      <div className="no-drag flex h-full">
        <button type="button" aria-label={t("titlebar.minimize")} className={btn} onClick={() => void win.minimize()}>
          <Icon name="minus" size={16} />
        </button>
        <button type="button" aria-label={t("titlebar.maximize")} className={btn} onClick={() => void win.toggleMaximize()}>
          <Icon name={maximized ? "restore" : "square"} size={14} />
        </button>
        <button
          type="button"
          aria-label={t("titlebar.close")}
          title={t("titlebar.close")}
          className="group relative flex h-full w-12 items-center justify-center text-muted hover:text-white"
          onClick={() => void win.close()}
        >
          {/* Rote Fläche nur hier (Schließen = destruktiv), eingeblendet per opacity. */}
          <span aria-hidden className="fade absolute inset-0 bg-danger opacity-0 group-hover:opacity-100" />
          <Icon name="x" size={16} className="relative" />
        </button>
      </div>
    </header>
  );
}
