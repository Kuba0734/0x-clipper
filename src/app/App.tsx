import { AnimatePresence, LazyMotion, MotionConfig, domMax, m } from "motion/react";
import { useEffect } from "react";

import { Dashboard } from "@/features/dashboard/Dashboard";
import { Library } from "@/features/library/Library";
import { ToastHost } from "@/features/overlay/ToastHost";
import { SettingsPage } from "@/features/settings/Settings";
import { translate } from "@/i18n";
import { commands, on } from "@/lib/ipc";
import { pageVariants } from "@/lib/motion";
import { useLibrary } from "@/stores/library";
import { useSettings } from "@/stores/settings";
import { useStatus } from "@/stores/status";
import { useToasts } from "@/stores/toasts";
import { useUi } from "@/stores/ui";
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
    <LazyMotion features={domMax} strict>
      <MotionConfig reducedMotion={reduce ? "always" : "user"}>
        <div className="flex h-full flex-col bg-bg">
          <TitleBar />
          <div className="flex min-h-0 flex-1">
            <Sidebar />
            <main className="relative min-w-0 flex-1 overflow-hidden">
              <AnimatePresence mode="wait" initial={false}>
                <m.div
                  key={page}
                  variants={pageVariants}
                  initial="initial"
                  animate="animate"
                  exit="exit"
                  className="absolute inset-0 overflow-y-auto"
                >
                  {page === "dashboard" && <Dashboard />}
                  {page === "library" && <Library />}
                  {page === "settings" && <SettingsPage />}
                </m.div>
              </AnimatePresence>
            </main>
          </div>
          <ToastHost />
        </div>
      </MotionConfig>
    </LazyMotion>
  );
}
