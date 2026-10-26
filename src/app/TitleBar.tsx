import { getCurrentWindow } from "@tauri-apps/api/window";
import { Copy, Minus, Square, X } from "lucide-react";
import { useEffect, useState } from "react";

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

  const btn = "flex h-full w-12 items-center justify-center text-muted transition-colors hover:bg-surface-2 hover:text-text";
  return (
    <header
      data-tauri-drag-region
      onDoubleClick={() => void win.toggleMaximize()}
      className="drag-region flex h-10 shrink-0 items-center border-b border-border/60 bg-bg pl-4"
    >
      <div data-tauri-drag-region className="flex items-center gap-2.5">
        <span className="size-4 rounded-[5px] bg-gradient-accent" aria-hidden />
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
          <Minus className="size-4" />
        </button>
        <button type="button" aria-label={t("titlebar.maximize")} className={btn} onClick={() => void win.toggleMaximize()}>
          {maximized ? <Copy className="size-3.5" /> : <Square className="size-3.5" />}
        </button>
        <button
          type="button"
          aria-label={t("titlebar.close")}
          title={t("titlebar.close")}
          className={`${btn} hover:bg-danger hover:text-white`}
          onClick={() => void win.close()}
        >
          <X className="size-4" />
        </button>
      </div>
    </header>
  );
}
