import { AnimatePresence, m } from "motion/react";
import { Circle, Clapperboard, Cpu, Gauge, HardDrive, MemoryStick, MicOff, Monitor, Power, Square } from "lucide-react";
import { useEffect } from "react";

import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { useT, type TKey } from "@/i18n";
import { commands } from "@/lib/ipc";
import { formatBytes, formatDuration, keycaps } from "@/lib/format";
import { listVariants, spring } from "@/lib/motion";
import { useLibrary } from "@/stores/library";
import { useSettings } from "@/stores/settings";
import { useStatus } from "@/stores/status";
import { useUi } from "@/stores/ui";
import { ClipThumb } from "@/features/library/ClipThumb";
import { BufferRing } from "./BufferRing";

function Stat({ icon: Icon, label, value, sub }: { icon: typeof Cpu; label: string; value: string; sub?: string }) {
  return (
    <Card className="flex items-center gap-3 p-4">
      <span className="flex size-9 shrink-0 items-center justify-center rounded-[10px] bg-surface-2 text-accent-2">
        <Icon className="size-[18px]" />
      </span>
      <div className="min-w-0">
        <div className="text-[12px] text-muted">{label}</div>
        <div className="tabular truncate text-[15px] font-semibold">{value}</div>
        {sub && <div className="tabular truncate text-[11.5px] text-muted">{sub}</div>}
      </div>
    </Card>
  );
}

export function Dashboard() {
  const t = useT();
  const status = useStatus((s) => s.status);
  const settings = useSettings((s) => s.settings);
  const clips = useLibrary((s) => s.clips);
  const loaded = useLibrary((s) => s.loaded);
  const go = useUi((s) => s.go);

  useEffect(() => {
    if (!loaded) void useLibrary.getState().load();
  }, [loaded]);

  const target = settings?.replay.durationSecs ?? 30;
  const buffered = status?.bufferedSecs ?? 0;
  const recording = status?.recording ?? false;
  const bufferOn = status?.bufferEnabled ?? settings?.replay.enabled ?? false;
  const ramPct = status && status.bufferLimitBytes > 0 ? status.bufferBytes / status.bufferLimitBytes : 0;

  const hotkeys: [TKey, string][] = settings
    ? [
        ["settings.hotkeys.saveClip", settings.hotkeys.saveClip],
        ["settings.hotkeys.toggleRecording", settings.hotkeys.toggleRecording],
        ["settings.hotkeys.toggleBuffer", settings.hotkeys.toggleBuffer],
        ["settings.hotkeys.toggleMicMute", settings.hotkeys.toggleMicMute],
      ]
    : [];

  return (
    <div className="mx-auto max-w-6xl px-8 py-7">
      <div className="mb-6 flex items-end justify-between gap-4">
        <div>
          <h1 className="text-[26px] font-semibold tracking-tight">{t("dashboard.title")}</h1>
          <p className="text-[13.5px] text-muted">{t("dashboard.subtitle")}</p>
        </div>
        <div className="flex gap-2">
          <Button
            variant="secondary"
            icon={<Power className="size-4" />}
            onClick={() => void commands.setBufferEnabled(!bufferOn)}
          >
            {bufferOn ? t("dashboard.bufferOff") : t("dashboard.bufferOn")}
          </Button>
          <Button
            variant={recording ? "danger" : "secondary"}
            icon={recording ? <Square className="size-3.5 fill-current" /> : <Circle className="size-3.5 fill-danger text-danger" />}
            onClick={() => void commands.toggleRecording()}
          >
            {recording ? t("dashboard.stopRecording") : t("dashboard.startRecording")}
          </Button>
          <Button
            variant="primary"
            icon={<Clapperboard className="size-4" />}
            disabled={!bufferOn}
            onClick={() => void commands.saveClip()}
          >
            {t("dashboard.saveClip")}
          </Button>
        </div>
      </div>

      <m.div variants={listVariants} initial="initial" animate="animate" className="grid grid-cols-[auto_1fr] gap-4">
        <Card className="row-span-2 flex flex-col items-center justify-center gap-4 px-10 py-8">
          <BufferRing fill={bufferOn ? buffered / target : 0} recording={recording} active={bufferOn || recording}>
            <div>
              <div className="tabular text-[40px] leading-none font-semibold tracking-tight">
                {bufferOn ? buffered.toFixed(1) : "—"}
                <span className="text-[18px] text-muted"> s</span>
              </div>
              <div className="mt-1.5 text-[12.5px] text-muted">
                {t("dashboard.buffered")} · {t("dashboard.of")} {target} s
              </div>
              <AnimatePresence>
                {recording && (
                  <m.div
                    initial={{ opacity: 0, y: 6 }}
                    animate={{ opacity: 1, y: 0 }}
                    exit={{ opacity: 0 }}
                    transition={spring}
                    className="tabular mt-3 inline-flex items-center gap-1.5 rounded-full bg-danger/15 px-2.5 py-1 text-[12px] font-semibold text-danger"
                  >
                    <m.span
                      className="size-1.5 rounded-full bg-danger"
                      animate={{ opacity: [1, 0.3, 1] }}
                      transition={{ duration: 1.2, repeat: Infinity }}
                    />
                    REC {formatDuration(status?.recordingSecs ?? 0)}
                  </m.div>
                )}
              </AnimatePresence>
            </div>
          </BufferRing>
          <div className="flex items-center gap-2 text-[12.5px] text-muted">
            <span className={`size-2 rounded-full ${status?.state === "error" ? "bg-danger" : bufferOn ? "bg-success" : "bg-muted"}`} />
            {t(`state.${status?.state ?? "idle"}`)}
            {status?.micMuted && (
              <span className="ml-2 inline-flex items-center gap-1 text-danger">
                <MicOff className="size-3.5" /> {t("dashboard.micMuted")}
              </span>
            )}
          </div>
          {status?.lastError && status.state === "error" && (
            <p className="max-w-64 text-center text-[12px] text-danger">{status.lastError}</p>
          )}
        </Card>

        <div className="grid grid-cols-2 gap-4">
          <Card className="col-span-2 p-4">
            <div className="mb-2 flex items-center justify-between text-[12.5px]">
              <span className="flex items-center gap-2 text-muted">
                <MemoryStick className="size-4" /> {t("dashboard.ram")}
              </span>
              <span className="tabular font-medium">
                {formatBytes(status?.bufferBytes ?? 0)} / {formatBytes(status?.bufferLimitBytes ?? 0)}
              </span>
            </div>
            <div className="h-2 overflow-hidden rounded-full bg-surface-2">
              <m.div
                className="h-full w-full origin-left bg-gradient-accent"
                animate={{ scaleX: Math.min(1, ramPct) }}
                transition={spring}
              />
            </div>
          </Card>
          <Stat icon={Gauge} label={t("dashboard.bitrate")} value={`${((status?.bitrateKbps ?? 0) / 1000).toFixed(1)} Mbit/s`} sub={`${(status?.fps ?? 0).toFixed(0)} ${t("dashboard.fps")} · ${status?.droppedFrames ?? 0} ${t("dashboard.dropped")}`} />
          <Stat icon={Cpu} label={t("dashboard.encoder")} value={status?.encoder || "—"} sub={status?.resolution} />
          <Stat icon={HardDrive} label={t("dashboard.processRam")} value={formatBytes(status?.processRamBytes ?? 0)} />
          <Stat icon={Monitor} label={t("dashboard.source")} value={status?.source || "—"} sub={status?.game ?? t("dashboard.noGame")} />
          <Card className="col-span-2 p-4">
            <div className="mb-3 text-[12.5px] text-muted">{t("dashboard.hotkeys")}</div>
            <div className="grid grid-cols-2 gap-x-8 gap-y-2.5">
              {hotkeys.map(([key, accel]) => (
                <div key={key} className="flex items-center justify-between gap-2 text-[13px]">
                  <span className="truncate">{t(key)}</span>
                  <span className="flex shrink-0 gap-1">
                    {accel ? (
                      keycaps(accel).map((k) => (
                        <kbd key={k} className="rounded-[6px] border border-border border-b-2 bg-surface-2 px-1.5 py-0.5 font-sans text-[11px] font-semibold">
                          {k}
                        </kbd>
                      ))
                    ) : (
                      <span className="text-[12px] text-muted">{t("dashboard.notSet")}</span>
                    )}
                  </span>
                </div>
              ))}
            </div>
          </Card>
        </div>
      </m.div>

      <div className="mt-8 mb-3 flex items-center justify-between">
        <h2 className="text-[15px] font-semibold">{t("dashboard.recent")}</h2>
        <Button variant="ghost" size="sm" onClick={() => go("library")}>
          {t("dashboard.showAll")}
        </Button>
      </div>
      {clips.length === 0 && loaded ? (
        <Card className="p-6 text-center text-[13px] text-muted">
          {t("dashboard.noClips", { hotkey: settings?.hotkeys.saveClip || "—", secs: target })}
        </Card>
      ) : (
        <m.div variants={listVariants} initial="initial" animate="animate" className="grid grid-cols-4 gap-4">
          {clips.slice(0, 4).map((c) => (
            <ClipThumb key={c.path} clip={c} onOpen={() => go("library")} compact />
          ))}
        </m.div>
      )}
    </div>
  );
}
