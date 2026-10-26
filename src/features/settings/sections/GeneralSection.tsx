import { Field, Section } from "@/components/ui/Field";
import { Segmented } from "@/components/ui/Segmented";
import { Toggle } from "@/components/ui/Toggle";
import { useT } from "@/i18n";
import type { Settings } from "@/lib/ipc";
import { useSection } from "../useSettingsDraft";

export function GeneralSection() {
  const t = useT();
  const [g, set] = useSection("general");
  return (
    <Section>
      <Field label={t("settings.general.language")}>
        <Segmented<Settings["general"]["language"]>
          value={g.language}
          onChange={(language) => set({ language })}
          options={[
            { value: "de", label: "Deutsch" },
            { value: "en", label: "English" },
          ]}
        />
      </Field>
      <Field label={t("settings.general.autostart")} hint={t("settings.general.autostartHint")}>
        <Toggle checked={g.autostart} onChange={(autostart) => set({ autostart })} label={t("settings.general.autostart")} />
      </Field>
      <Field label={t("settings.general.startMinimized")}>
        <Toggle
          checked={g.startMinimized}
          disabled={!g.autostart}
          onChange={(startMinimized) => set({ startMinimized })}
          label={t("settings.general.startMinimized")}
        />
      </Field>
      <Field label={t("settings.general.reduceMotion")} hint={t("settings.general.reduceMotionHint")}>
        <Toggle checked={g.reduceMotion} onChange={(reduceMotion) => set({ reduceMotion })} label={t("settings.general.reduceMotion")} />
      </Field>
    </Section>
  );
}
