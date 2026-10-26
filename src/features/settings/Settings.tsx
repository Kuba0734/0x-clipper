import { open, save } from "@tauri-apps/plugin-dialog";
import { useState } from "react";

import { Button } from "@/components/ui/Button";
import { Skeleton } from "@/components/ui/Card";
import { Crossfade } from "@/components/ui/Crossfade";
import { Icon, type IconName } from "@/components/ui/Icon";
import { useT, type TKey } from "@/i18n";
import { commands, errorMessage } from "@/lib/ipc";
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

type CatId = "general" | "replay" | "video" | "audio" | "hotkeys" | "output" | "overlay";

const categories: { id: CatId; icon: IconName; Comp: () => React.ReactNode }[] = [
  { id: "general", icon: "sliders", Comp: GeneralSection },
  { id: "replay", icon: "timer", Comp: ReplaySection },
  { id: "video", icon: "monitorPlay", Comp: VideoSection },
  { id: "audio", icon: "audio", Comp: AudioSection },
  { id: "hotkeys", icon: "keyboard", Comp: HotkeysSection },
  { id: "output", icon: "folderOut", Comp: OutputSection },
  { id: "overlay", icon: "layers", Comp: OverlaySection },
];

const ITEM_H = 36;
const GAP = 2;

export function SettingsPage() {
  const t = useT();
  const settings = useSettings((s) => s.settings);
  const replace = useSettings((s) => s.replace);
  const { settingsCategory, setSettingsCategory } = useUi();
  const push = useToasts((s) => s.push);
  const [confirmReset, setConfirmReset] = useState(false);
  const idx = Math.max(0, categories.findIndex((c) => c.id === settingsCategory));
  const active = categories[idx] ?? categories[0];

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
    <div className="mx-auto flex max-w-6xl gap-8 px-8 py-8">
      <aside className="sticky top-0 w-52 shrink-0 self-start">
        <h1 className="anim-rise-in mb-5 text-[28px] font-semibold tracking-tight">{t("settings.title")}</h1>
        <nav className="relative flex flex-col" style={{ gap: GAP }}>
          <span
            aria-hidden
            className="slide-indicator absolute inset-x-0 top-0 rounded-sm border border-border bg-surface-2"
            style={{ height: ITEM_H, transform: `translateY(${idx * (ITEM_H + GAP)}px)` }}
          />
          {categories.map(({ id, icon }) => {
            const isActive = id === active?.id;
            return (
              <button
                key={id}
                type="button"
                onClick={() => setSettingsCategory(id)}
                style={{ height: ITEM_H }}
                className={`press relative flex items-center gap-2.5 rounded-sm px-3 text-[13.5px] font-medium ${
                  isActive ? "text-text" : "text-muted hover:text-text"
                }`}
              >
                <Icon name={icon} size={16} />
                <span>{t(`settings.categories.${id}` as TKey)}</span>
              </button>
            );
          })}
        </nav>
        <div className="mt-6 flex flex-col gap-1">
          <Button size="sm" variant="ghost" icon={<Icon name="upload" size={16} />} onClick={() => void doExport()} className="justify-start">
            {t("settings.export")}
          </Button>
          <Button size="sm" variant="ghost" icon={<Icon name="download" size={16} />} onClick={() => void doImport()} className="justify-start">
            {t("settings.import")}
          </Button>
          <Button
            size="sm"
            variant={confirmReset ? "danger" : "ghost"}
            icon={<Icon name="reset" size={16} />}
            onClick={() => void doReset()}
            onBlur={() => setConfirmReset(false)}
            className="justify-start"
          >
            {confirmReset ? t("settings.resetConfirm") : t("settings.reset")}
          </Button>
        </div>
      </aside>
      <div className="min-w-0 flex-1 pt-[60px]">
        {!settings ? (
          <div className="space-y-3">
            <Skeleton className="h-16" />
            <Skeleton className="h-16" />
            <Skeleton className="h-16" />
          </div>
        ) : (
          <Crossfade
            active={active?.id ?? "general"}
            render={(id) => {
              const Comp = categories.find((c) => c.id === id)?.Comp ?? GeneralSection;
              return (
                <div className="space-y-4">
                  <Comp />
                </div>
              );
            }}
          />
        )}
      </div>
    </div>
  );
}
