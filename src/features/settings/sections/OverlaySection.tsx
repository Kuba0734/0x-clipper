import { m } from "motion/react";

import { Field, Section } from "@/components/ui/Field";
import { Slider } from "@/components/ui/Slider";
import { Toggle } from "@/components/ui/Toggle";
import { useT, type TKey } from "@/i18n";
import { spring } from "@/lib/motion";
import type { Settings } from "@/lib/ipc";
import { useSection } from "../useSettingsDraft";

type Pos = Settings["overlay"]["position"];
const POSITIONS: Pos[] = ["topLeft", "topRight", "bottomLeft", "bottomRight"];

/** Positionswahl als Mini-Bildschirm; der Marker gleitet per layoutId in die Ecke. */
function PositionPicker({ value, onChange }: { value: Pos; onChange: (p: Pos) => void }) {
  const t = useT();
  return (
    <div role="radiogroup" className="grid h-24 w-40 grid-cols-2 grid-rows-2 gap-1 rounded-[10px] bg-surface-2 p-1.5 ring-1 ring-border">
      {POSITIONS.map((p) => (
        <button
          key={p}
          type="button"
          role="radio"
          aria-checked={value === p}
          aria-label={t(`settings.overlay.positions.${p}` as TKey)}
          title={t(`settings.overlay.positions.${p}` as TKey)}
          onClick={() => onChange(p)}
          className={`flex rounded-[6px] p-1 hover:bg-surface ${p.startsWith("top") ? "items-start" : "items-end"} ${
            p.endsWith("Left") ? "justify-start" : "justify-end"
          }`}
        >
          {value === p && <m.span layoutId="overlay-pos" transition={spring} className="h-3 w-9 rounded-[4px] bg-gradient-accent" />}
        </button>
      ))}
    </div>
  );
}

export function OverlaySection() {
  const t = useT();
  const [o, set] = useSection("overlay");
  const [s, setSound] = useSection("sound");
  return (
    <>
      <Section>
        <Field label={t("settings.overlay.enabled")} hint={t("settings.overlay.enabledHint")}>
          <Toggle checked={o.enabled} onChange={(enabled) => set({ enabled })} label={t("settings.overlay.enabled")} />
        </Field>
        <Field label={t("settings.overlay.position")} hint={t(`settings.overlay.positions.${o.position}` as TKey)}>
          <PositionPicker value={o.position} onChange={(position) => set({ position })} />
        </Field>
        <Field label={t("settings.overlay.duration")} stacked>
          <Slider
            value={o.durationMs}
            min={1000}
            max={8000}
            step={250}
            onChange={(durationMs) => set({ durationMs })}
            format={(v) => `${(v / 1000).toFixed(2).replace(/0$/, "")} s`}
          />
        </Field>
      </Section>
      <Section>
        <Field label={t("settings.overlay.sound")}>
          <Toggle checked={s.enabled} onChange={(enabled) => setSound({ enabled })} label={t("settings.overlay.sound")} />
        </Field>
        <Field label={t("settings.overlay.soundVolume")} stacked>
          <Slider
            value={Math.round(s.volume * 100)}
            min={0}
            max={100}
            step={5}
            disabled={!s.enabled}
            onChange={(v) => setSound({ volume: v / 100 })}
            format={(v) => `${v} %`}
          />
        </Field>
      </Section>
    </>
  );
}
