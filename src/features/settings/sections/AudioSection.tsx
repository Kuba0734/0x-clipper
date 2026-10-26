import { useEffect, useState } from "react";

import { Field, Section } from "@/components/ui/Field";
import { Segmented } from "@/components/ui/Segmented";
import { Select } from "@/components/ui/Select";
import { Slider } from "@/components/ui/Slider";
import { Toggle } from "@/components/ui/Toggle";
import { useT } from "@/i18n";
import { commands, type AudioDevices, type Settings } from "@/lib/ipc";
import { useSection } from "../useSettingsDraft";

type Source = Settings["audio"]["system"];

function SourceBlock({
  title,
  value,
  devices,
  onChange,
}: {
  title: string;
  value: Source;
  devices: { id: string; name: string; isDefault: boolean }[];
  onChange: (p: Partial<Source>) => void;
}) {
  const t = useT();
  return (
    <Section title={title}>
      <Field label={title}>
        <Toggle checked={value.enabled} onChange={(enabled) => onChange({ enabled })} label={title} />
      </Field>
      <Field label={t("settings.audio.device")}>
        <Select
          className="w-72"
          value={value.deviceId}
          disabled={!value.enabled}
          onChange={(deviceId) => onChange({ deviceId })}
          options={[
            { value: "", label: t("settings.audio.default") },
            ...devices.map((d) => ({ value: d.id, label: d.name })),
          ]}
        />
      </Field>
      <Field label={t("settings.audio.volume")} stacked>
        <Slider
          value={Math.round(value.volume * 100)}
          min={0}
          max={200}
          step={5}
          disabled={!value.enabled}
          onChange={(v) => onChange({ volume: v / 100 })}
          format={(v) => `${v} %`}
        />
      </Field>
    </Section>
  );
}

export function AudioSection() {
  const t = useT();
  const [a, set] = useSection("audio");
  const [devices, setDevices] = useState<AudioDevices>({ outputs: [], inputs: [] });
  useEffect(() => {
    void commands.listAudioDevices().then(setDevices).catch(() => {});
  }, []);
  return (
    <>
      <SourceBlock
        title={t("settings.audio.system")}
        value={a.system}
        devices={devices.outputs}
        onChange={(p) => set({ system: { ...a.system, ...p } })}
      />
      <SourceBlock
        title={t("settings.audio.mic")}
        value={a.mic}
        devices={devices.inputs}
        onChange={(p) => set({ mic: { ...a.mic, ...p } })}
      />
      <Section>
        <Field label={t("settings.audio.tracks")} hint={t("settings.audio.tracksHint")}>
          <Segmented
            value={a.trackMode}
            onChange={(trackMode) => set({ trackMode })}
            options={[
              { value: "separate", label: t("settings.audio.separate") },
              { value: "mixed", label: t("settings.audio.mixed") },
            ]}
          />
        </Field>
        <Field label={t("settings.audio.pushToTalk")} hint={t("settings.audio.pushToTalkHint")}>
          <Toggle checked={a.pushToTalk} onChange={(pushToTalk) => set({ pushToTalk })} label={t("settings.audio.pushToTalk")} />
        </Field>
        <Field label={t("settings.audio.bitrate")}>
          <Select
            className="w-36"
            value={String(a.bitrateKbps)}
            onChange={(v) => set({ bitrateKbps: Number(v) })}
            options={["96", "128", "160", "192", "256", "320"].map((v) => ({ value: v, label: `${v} kbit/s` }))}
          />
        </Field>
      </Section>
    </>
  );
}
