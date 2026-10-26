import { save, open } from "@tauri-apps/plugin-dialog";
import { AnimatePresence, m } from "motion/react";
import { AudioLines, Download, Keyboard, Layers, MonitorPlay, RotateCcw, Settings2, Timer, Upload, FolderOutput } from "lucide-react";
import { useState } from "react";

import { Button } from "@/components/ui/Button";
import { Skeleton } from "@/components/ui/Card";
import { useT, type TKey } from "@/i18n";
import { commands, errorMessage } from "@/lib/ipc";
import { spring } from "@/lib/motion";
import { useSettings } from "@/stores/settings";
import { useToasts } from "@/stores/toasts";
import { useUi } from "@/stores/ui";
import { AudioSection } from "./sections/AudioSection";
import { GeneralSection } from "./sections/GeneralSection";
import { HotkeysSection } from "./sections/HotkeysSection";
import { OutputSection } from "./sections/OutputSection";
import { OverlaySection } from "./sections/OverlaySection";
import { ReplaySection } from "./sections/ReplaySection";
import { VideoSection } from "./sections/VideoSection";

const categories = [
  { id: "general", icon: Settings2, Comp: GeneralSection },
  { id: "replay", icon: Timer, Comp: ReplaySection },
  { id: "video", icon: MonitorPlay, Comp: VideoSection },
  { id: "audio", icon: AudioLines, Comp: AudioSection },
  { id: "hotkeys", icon: Keyboard, Comp: HotkeysSection },
  { id: "output", icon: FolderOutput, Comp: OutputSection },
  { id: "overlay", icon: Layers, Comp: OverlaySection },
] as const;

export function SettingsPage() {
  const t = useT();
  const settings = useSettings((s) => s.settings);
  const saving = useSettings((s) => s.saving);
  const replace = useSettings((s) => s.replace);
  const { settingsCategory: cat, setSettingsCategory } = useUi();
  const push = useToasts((s) => s.push);
  const [confirmReset, setConfirmReset] = useState(false);
  const active = categories.find((c) => c.id === cat) ?? categories[0];

  const doExport = async () => {
    const path = await save({ defaultPath: "0x-clipper-settings.json", filters: [{ name: "JSON", extensions: ["json"] }] });
    if (!path) return;
    try {
      await commands.exportSettings(path);
    } catch (e) {
      push({ kind: "error", title: errorMessage(e) });
    }
  };
  const doImport = async () => {
    const path = await open({ multiple: false, filters: [{ name: "JSON", extensions: ["json"] }] });
    if (typeof path !== "string") return;
    try {
      replace(await commands.importSettings(path));
    } catch (e) {
      push({ kind: "error", title: errorMessage(e) });
    }
  };
  const doReset = async () => {
    if (!confirmReset) return setConfirmReset(true);
    setConfirmReset(false);
    replace(await commands.resetSettings());
  };

  return (
    <div className="mx-auto flex max-w-6xl gap-8 px-8 py-7">
      <aside className="sticky top-0 w-52 shrink-0 self-start">
        <h1 className="mb-4 text-[26px] font-semibold tracking-tight">{t("settings.title")}</h1>
        <nav className="flex flex-col gap-0.5">
          {categories.map(({ id, icon: Icon }) => {
            const isActive = id === active.id;
            return (
              <button
                key={id}
                type="button"
                onClick={() => setSettingsCategory(id)}
                className={`relative flex h-9 items-center gap-2.5 rounded-[10px] px-3 text-[13.5px] font-medium transition-colors ${
                  isActive ? "text-text" : "text-muted hover:text-text"
                }`}
              >
                {isActive && (
                  <m.span layoutId="settings-cat" transition={spring} className="absolute inset-0 rounded-[10px] bg-surface ring-1 ring-border" />
                )}
                <Icon className={`relative size-4 ${isActive ? "text-accent-2" : ""}`} />
                <span className="relative">{t(`settings.categories.${id}` as TKey)}</span>
              </button>
            );
          })}
        </nav>
        <div className="mt-6 flex flex-col gap-1.5">
          <Button size="sm" variant="ghost" icon={<Upload className="size-4" />} onClick={() => void doExport()} className="justify-start">
            {t("settings.export")}
          </Button>
          <Button size="sm" variant="ghost" icon={<Download className="size-4" />} onClick={() => void doImport()} className="justify-start">
            {t("settings.import")}
          </Button>
          <Button
            size="sm"
            variant={confirmReset ? "danger" : "ghost"}
            icon={<RotateCcw className="size-4" />}
            onClick={() => void doReset()}
            onBlur={() => setConfirmReset(false)}
            className="justify-start"
          >
            {confirmReset ? t("settings.resetConfirm") : t("settings.reset")}
          </Button>
        </div>
        <AnimatePresence>
          {saving && (
            <m.div initial={{ opacity: 0 }} animate={{ opacity: 1 }} exit={{ opacity: 0 }} className="mt-4 px-3 text-[12px] text-muted">
              …
            </m.div>
          )}
        </AnimatePresence>
      </aside>
      <div className="min-w-0 flex-1 pt-14">
        {!settings ? (
          <div className="space-y-3">
            <Skeleton className="h-16" />
            <Skeleton className="h-16" />
            <Skeleton className="h-16" />
          </div>
        ) : (
          <AnimatePresence mode="wait" initial={false}>
            <m.div
              key={active.id}
              initial={{ opacity: 0, y: 8 }}
              animate={{ opacity: 1, y: 0 }}
              exit={{ opacity: 0, y: -6 }}
              transition={{ duration: 0.18 }}
              className="space-y-4"
            >
              <active.Comp />
            </m.div>
          </AnimatePresence>
        )}
      </div>
    </div>
  );
}
