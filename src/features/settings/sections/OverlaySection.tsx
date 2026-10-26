import { Field, Section } from "@/components/ui/Field";
import { Slider } from "@/components/ui/Slider";
import { Toggle } from "@/components/ui/Toggle";
import { useT, type TKey } from "@/i18n";
import type { Settings } from "@/lib/ipc";
import { useSection } from "../useSettingsDraft";

type Pos = Settings["overlay"]["position"];
const POSITIONS: Pos[] = ["topLeft", "topRight", "bottomLeft", "bottomRight"];

// Mini-Bildschirm 160 × 96 mit 8 px Innenabstand; Marker 36 × 12.
const W = 160;
const H = 96;
const PAD = 8;
const MW = 36;
const MH = 12;

/** Positionswahl als Mini-Bildschirm; der Marker gleitet per translate in die Ecke. */
function PositionPicker({ value, onChange }: { value: Pos; onChange: (p: Pos) => void }) {
  const t = useT();
  const x = value.endsWith("Right") ? W - 2 * PAD - MW : 0;
  const y = value.startsWith("bottom") ? H - 2 * PAD - MH : 0;
  return (
    <div role="radiogroup" className="relative grid grid-cols-2 grid-rows-2 rounded-md border border-border bg-surface" style={{ width: W, height: H }}>
      <span
        aria-hidden
        className="slide-indicator absolute rounded-[4px] bg-accent"
        style={{ left: PAD, top: PAD, width: MW, height: MH, transform: `translate(${x}px, ${y}px)` }}
      />
      {POSITIONS.map((p) => (
        <button
          key={p}
          type="button"
          role="radio"
          aria-checked={value === p}
          aria-label={t(`settings.overlay.positions.${p}` as TKey)}
          title={t(`settings.overlay.positions.${p}` as TKey)}
          onClick={() => onChange(p)}
          className="hoverable press rounded-sm"
        />
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
      <Section index={0}>
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
      <Section index={1}>
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
