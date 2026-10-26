import { Field, Section } from "@/components/ui/Field";
import { Slider } from "@/components/ui/Slider";
import { Toggle } from "@/components/ui/Toggle";
import { useT } from "@/i18n";
import { formatDuration } from "@/lib/format";
import { useSettings } from "@/stores/settings";
import { useSection } from "../useSettingsDraft";

/** Nichtlineare Skala: feine Schritte bei kurzen Längen, grobe bei langen. */
const STEPS = [5, 10, 15, 20, 30, 45, 60, 90, 120, 180, 240, 300, 420, 600];

export function ReplaySection() {
  const t = useT();
  const [r, set] = useSection("replay");
  const kbps = useSettings((s) => (s.settings?.video.bitrateKbps ?? 20000) + (s.settings?.audio.bitrateKbps ?? 160) * 2);
  const idx = Math.max(0, STEPS.findIndex((s) => s >= r.durationSecs));
  const estMb = Math.round((kbps * 1000 * r.durationSecs) / 8 / 1024 / 1024);
  return (
    <Section>
      <Field label={t("settings.replay.enabled")} hint={t("settings.replay.enabledHint")}>
        <Toggle checked={r.enabled} onChange={(enabled) => set({ enabled })} label={t("settings.replay.enabled")} />
      </Field>
      <Field label={t("settings.replay.duration")} hint={t("settings.replay.estimate", { mb: estMb, kbps })} stacked>
        <Slider
          value={idx}
          min={0}
          max={STEPS.length - 1}
          onChange={(i) => set({ durationSecs: STEPS[i] ?? 30 })}
          format={(i) => formatDuration(STEPS[i] ?? 30)}
          label={t("settings.replay.duration")}
        />
      </Field>
      <Field label={t("settings.replay.ramLimit")} stacked>
        <Slider
          value={r.ramLimitMb}
          min={64}
          max={4096}
          step={64}
          onChange={(ramLimitMb) => set({ ramLimitMb })}
          format={(v) => `${v} MB`}
          label={t("settings.replay.ramLimit")}
        />
      </Field>
    </Section>
  );
}
