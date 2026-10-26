import { RefreshCw } from "lucide-react";
import { useEffect, useState } from "react";

import { IconButton } from "@/components/ui/Button";
import { Field, Section } from "@/components/ui/Field";
import { NumberInput } from "@/components/ui/NumberInput";
import { Segmented } from "@/components/ui/Segmented";
import { Select } from "@/components/ui/Select";
import { Slider } from "@/components/ui/Slider";
import { Toggle } from "@/components/ui/Toggle";
import { useT, type TKey } from "@/i18n";
import { commands, type MonitorInfo, type Settings, type WindowInfo } from "@/lib/ipc";
import { useSettings } from "@/stores/settings";
import { useSection } from "../useSettingsDraft";

type Video = Settings["video"];
type Capture = Settings["capture"];

const PRESET_VALUES: Record<Exclude<Video["preset"], "custom">, Pick<Video, "rateControl" | "bitrateKbps">> = {
  performance: { rateControl: "cbr", bitrateKbps: 12000 },
  balanced: { rateControl: "cbr", bitrateKbps: 20000 },
  quality: { rateControl: "vbr", bitrateKbps: 40000 },
};

function SourceFields() {
  const t = useT();
  const [c, set] = useSection("capture");
  const [monitors, setMonitors] = useState<MonitorInfo[]>([]);
  const [windows, setWindows] = useState<WindowInfo[]>([]);
  const refresh = () => {
    void commands.listMonitors().then(setMonitors);
    void commands.listWindows().then(setWindows);
  };
  useEffect(refresh, []);

  const kind = c.source.kind;
  const setKind = (k: Capture["source"]["kind"]) => {
    if (k === "autoGame") set({ source: { kind: "autoGame" } });
    if (k === "monitor") set({ source: { kind: "monitor", device: monitors.find((m) => m.primary)?.device ?? "" } });
    if (k === "window") {
      const w = windows[0];
      set({ source: { kind: "window", exe: w?.exe ?? "", title: w?.title ?? "" } });
    }
  };
  const winKey = (w: { exe: string; title: string }) => `${w.exe}\u0000${w.title}`;

  return (
    <>
      <Field label={t("settings.video.source")}>
        <Segmented
          value={kind}
          onChange={setKind}
          options={[
            { value: "autoGame", label: t("settings.video.sourceAuto") },
            { value: "monitor", label: t("settings.video.sourceMonitor") },
            { value: "window", label: t("settings.video.sourceWindow") },
          ]}
        />
      </Field>
      {c.source.kind === "monitor" && (
        <Field label={t("settings.video.monitor")}>
          <Select
            className="w-72"
            value={c.source.device}
            onChange={(device) => set({ source: { kind: "monitor", device } })}
            options={monitors.map((m) => ({
              value: m.device,
              label: `${m.device.replace("\\\\.\\", "")} · ${m.width}×${m.height}${m.primary ? ` (${t("settings.video.primary")})` : ""}`,
            }))}
          />
          <IconButton label={t("settings.video.refresh")} onClick={refresh}>
            <RefreshCw className="size-4" />
          </IconButton>
        </Field>
      )}
      {c.source.kind === "window" && (
        <Field label={t("settings.video.window")}>
          <Select
            className="w-72"
            value={winKey(c.source)}
            onChange={(v) => {
              const [exe = "", title = ""] = v.split("\u0000");
              set({ source: { kind: "window", exe, title } });
            }}
            options={[
              ...(windows.some((w) => winKey(w) === winKey(c.source as WindowInfo)) ? [] : [c.source as WindowInfo]),
              ...windows,
            ].map((w) => ({ value: winKey(w), label: `${w.title} — ${w.exe}` }))}
          />
          <IconButton label={t("settings.video.refresh")} onClick={refresh}>
            <RefreshCw className="size-4" />
          </IconButton>
        </Field>
      )}
      <Field label={t("settings.video.backend")}>
        <Select
          className="w-48"
          value={c.backend}
          onChange={(backend) => set({ backend })}
          options={[
            { value: "auto", label: t("settings.video.backendAuto") },
            { value: "wgc", label: "Windows Graphics Capture" },
            { value: "dxgi", label: "DXGI Desktop Duplication" },
          ]}
        />
      </Field>
      <Field label={t("settings.video.cursor")}>
        <Toggle checked={c.captureCursor} onChange={(captureCursor) => set({ captureCursor })} label={t("settings.video.cursor")} />
      </Field>
      <Field label={t("settings.video.hideBorder")} hint={t("settings.video.hideBorderHint")}>
        <Toggle checked={c.hideBorder} onChange={(hideBorder) => set({ hideBorder })} label={t("settings.video.hideBorder")} />
      </Field>
    </>
  );
}

export function VideoSection() {
  const t = useT();
  const [v, set] = useSection("video");
  const encoders = useSettings((s) => s.encoders);
  const resKind = v.resolution.kind;
  const forCodec = encoders.filter((e) => e.codec === v.codec);
  const codecs = (["h264", "hevc", "av1"] as const).filter((c) => encoders.some((e) => e.codec === c));

  const setPreset = (preset: Video["preset"]) =>
    set(preset === "custom" ? { preset } : { preset, ...PRESET_VALUES[preset] });
  // Manuelle Änderung an Bitrate/RC → „Benutzerdefiniert“.
  const setCustom = (patch: Partial<Video>) => set({ ...patch, preset: "custom" });

  return (
    <>
      <Section title={t("settings.video.source")}>
        <SourceFields />
      </Section>
      <Section title={t("settings.categories.video")}>
        <Field label={t("settings.video.preset")}>
          <Segmented
            value={v.preset}
            onChange={setPreset}
            options={(["performance", "balanced", "quality", "custom"] as const).map((p) => ({
              value: p,
              label: t(`settings.video.presets.${p}` as TKey),
            }))}
          />
        </Field>
        <Field label={t("settings.video.resolution")}>
          <Select
            className="w-40"
            value={resKind}
            onChange={(kind) =>
              set({ resolution: kind === "custom" ? { kind, width: 1920, height: 1080 } : ({ kind } as Video["resolution"]) })
            }
            options={[
              { value: "native", label: t("settings.video.native") },
              { value: "p1440", label: "1440p" },
              { value: "p1080", label: "1080p" },
              { value: "p720", label: "720p" },
              { value: "custom", label: t("settings.video.custom") },
            ]}
          />
          {v.resolution.kind === "custom" && (
            <>
              <NumberInput
                width="w-24"
                value={v.resolution.width}
                min={320}
                max={7680}
                onChange={(width) => set({ resolution: { ...(v.resolution as { kind: "custom"; width: number; height: number }), width } })}
              />
              <span className="text-muted">×</span>
              <NumberInput
                width="w-24"
                value={v.resolution.height}
                min={180}
                max={4320}
                onChange={(height) => set({ resolution: { ...(v.resolution as { kind: "custom"; width: number; height: number }), height } })}
              />
            </>
          )}
        </Field>
        <Field label={t("settings.video.fps")}>
          <Segmented
            value={String(v.fps)}
            onChange={(f) => set({ fps: Number(f) })}
            options={["30", "60", "120", "144", "240"].map((f) => ({ value: f, label: f }))}
          />
        </Field>
        <Field label={t("settings.video.codec")}>
          <Segmented
            value={v.codec}
            onChange={(codec) => set({ codec, encoder: "auto" })}
            options={codecs.map((c) => ({ value: c, label: c === "h264" ? "H.264" : c === "hevc" ? "HEVC" : "AV1" }))}
          />
        </Field>
        <Field label={t("settings.video.encoder")}>
          <Select
            className="w-64"
            value={v.encoder}
            onChange={(encoder) => set({ encoder })}
            options={[
              { value: "auto", label: t("settings.video.encoderAuto") },
              ...forCodec.map((e) => ({
                value: e.id,
                label: `${e.id}${e.hardware ? ` · ${e.vendor.toUpperCase()}` : ` · ${t("settings.video.software")}`}`,
              })),
            ]}
          />
        </Field>
        <Field label={t("settings.video.rateControl")}>
          <Segmented
            value={v.rateControl}
            onChange={(rateControl) => setCustom({ rateControl })}
            options={[
              { value: "cbr", label: "CBR" },
              { value: "vbr", label: "VBR" },
              { value: "cqp", label: "CQP" },
            ]}
          />
        </Field>
        {v.rateControl === "cqp" ? (
          <Field label={t("settings.video.cqp")} hint={t("settings.video.cqpHint")} stacked>
            <Slider value={v.cqp} min={10} max={40} onChange={(cqp) => setCustom({ cqp })} format={(q) => `QP ${q}`} />
          </Field>
        ) : (
          <Field label={t("settings.video.bitrate")} stacked>
            <Slider
              value={v.bitrateKbps}
              min={2000}
              max={100000}
              step={1000}
              onChange={(bitrateKbps) => setCustom({ bitrateKbps })}
              format={(k) => `${(k / 1000).toFixed(0)} Mbit/s`}
            />
          </Field>
        )}
        <Field label={t("settings.video.keyframe")}>
          <Segmented
            value={String(v.keyframeIntervalSecs)}
            onChange={(k) => set({ keyframeIntervalSecs: Number(k) })}
            options={[
              { value: "1", label: "1 s" },
              { value: "2", label: "2 s" },
            ]}
          />
        </Field>
      </Section>
    </>
  );
}
