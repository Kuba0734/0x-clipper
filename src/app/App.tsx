import { useEffect, useState, type ReactNode } from "react";

import { Dashboard } from "@/features/dashboard/Dashboard";
import { Library } from "@/features/library/Library";
import { ToastHost } from "@/features/overlay/ToastHost";
import { SettingsPage } from "@/features/settings/Settings";
import { translate } from "@/i18n";
import { EXIT_MS } from "@/lib/anim";
import { commands, on } from "@/lib/ipc";
import { useLibrary } from "@/stores/library";
import { useSettings } from "@/stores/settings";
import { useStatus } from "@/stores/status";
import { useToasts } from "@/stores/toasts";
import { useUi, type Page } from "@/stores/ui";
import { Sidebar } from "./Sidebar";
import { TitleBar } from "./TitleBar";

/** Verbindet Core-Events mit den Stores (einmalig). */
function useCoreEvents() {
  useEffect(() => {
    void useSettings.getState().load();
    void commands.getStatus().then(useStatus.getState().set);
    const subs = [
      on("status", useStatus.getState().set),
      on("settings-changed", useSettings.getState().replace),
      on("encoders", useSettings.getState().setEncoders),
      on("clip-saved", (clip) => {
        useLibrary.getState().upsert(clip);
        const lang = useSettings.getState().settings?.general.language ?? "de";
        useToasts.getState().push({ kind: "success", title: translate(lang, "toast.clipSaved"), detail: clip.fileName });
      }),
      on("library-changed", () => void useLibrary.getState().load()),
      on("core-error", (e) => useToasts.getState().push({ kind: "error", title: e.message })),
    ];
    return () => subs.forEach((p) => void p.then((un) => un()));
  }, []);
}

function pageContent(page: Page): ReactNode {
  switch (page) {
    case "dashboard":
      return <Dashboard />;
    case "library":
      return <Library />;
    case "settings":
      return <SettingsPage />;
  }
}

/**
 * Crossfade: Die neue Seite erscheint sofort (Eingaben werden nie verzögert),
 * die alte blendet darüber nur noch per opacity aus und nimmt keine Klicks mehr an.
 */
function PageSwitch({ page }: { page: Page }) {
  const [shown, setShown] = useState(page);
  const [leaving, setLeaving] = useState<Page | null>(null);
  if (page !== shown) {
    setLeaving(shown);
    setShown(page);
  }
  useEffect(() => {
    if (!leaving) return;
    const t = window.setTimeout(() => setLeaving(null), EXIT_MS);
    return () => window.clearTimeout(t);
  }, [leaving]);

  // Beide Seiten tragen ihren Seitennamen als Key: die alte behält ihre Instanz
  // (kein erneutes Laden), sie bekommt nur die Ausblend-Klasse.
  const pages = leaving && leaving !== shown ? [leaving, shown] : [shown];
  return (
    <>
      {pages.map((p) => {
        const out = p !== shown;
        return (
          <div
            key={p}
            aria-hidden={out || undefined}
            className={`absolute inset-0 ${out ? "anim-fade-out pointer-events-none overflow-hidden" : "anim-fade-in overflow-y-auto"}`}
          >
            {pageContent(p)}
          </div>
        );
      })}
    </>
  );
}

export function App() {
  useCoreEvents();
  const page = useUi((s) => s.page);
  const reduce = useSettings((s) => s.settings?.general.reduceMotion ?? false);
  const lang = useSettings((s) => s.settings?.general.language ?? "de");

  useEffect(() => {
    document.documentElement.lang = lang;
    document.documentElement.classList.toggle("reduce-motion", reduce);
  }, [lang, reduce]);

  return (
    <div className="flex h-full flex-col">
      <TitleBar />
      <div className="flex min-h-0 flex-1">
        <Sidebar />
        <main className="relative min-w-0 flex-1 overflow-hidden">
          <PageSwitch page={page} />
        </main>
      </div>
      <ToastHost />
    </div>
  );
}
