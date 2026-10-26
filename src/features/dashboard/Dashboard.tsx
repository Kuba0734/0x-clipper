import { useEffect, type ReactNode } from "react";

import { Button } from "@/components/ui/Button";
import { Card } from "@/components/ui/Card";
import { Icon, type IconName } from "@/components/ui/Icon";
import { Keycap } from "@/components/ui/KeycapInput";
import { ClipThumb } from "@/features/library/ClipThumb";
import { useT, type TKey } from "@/i18n";
import { useCountUp } from "@/lib/anim";
import { formatBytes, formatDuration, keycaps } from "@/lib/format";
import { commands } from "@/lib/ipc";
import { useLibrary } from "@/stores/library";
import { useSettings } from "@/stores/settings";
import { useStatus } from "@/stores/status";
import { useUi } from "@/stores/ui";
import { BufferRing } from "./BufferRing";

/** Zahl, die bei jeder Änderung animiert zum neuen Wert hochzählt. */
function CountUp({ value, format }: { value: number; format: (v: number) => string }) {
  const v = useCountUp(value);
  return <>{format(v)}</>;
}

function Stat({ icon, label, value, sub, index }: { icon: IconName; label: string; value: ReactNode; sub?: ReactNode; index: number }) {
  return (
    <Card index={index} className="flex items-center gap-3.5 p-4">
      <span className="grid size-10 shrink-0 place-items-center rounded-md border border-border bg-surface-2 text-text">
        <Icon name={icon} />
      </span>
      <div className="min-w-0">
        <div className="text-[12px] text-muted">{label}</div>
        <div className="tabular truncate text-[16px] font-semibold tracking-tight">{value}</div>
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
  const ramPct = status && status.bufferLimitBytes > 0 ? Math.min(1, status.bufferBytes / status.bufferLimitBytes) : 0;
  const shownBuffered = useCountUp(bufferOn ? buffered : 0);

  const hotkeys: [TKey, string][] = settings
    ? [
        ["settings.hotkeys.saveClip", settings.hotkeys.saveClip],
        ["settings.hotkeys.toggleRecording", settings.hotkeys.toggleRecording],
        ["settings.hotkeys.toggleBuffer", settings.hotkeys.toggleBuffer],
        ["settings.hotkeys.toggleMicMute", settings.hotkeys.toggleMicMute],
      ]
    : [];

  return (
    <div className="mx-auto max-w-6xl px-8 py-8">
      <div className="mb-7 flex items-end justify-between gap-4">
        <div className="anim-rise-in">
          <h1 className="text-[28px] font-semibold tracking-tight">{t("dashboard.title")}</h1>
          <p className="mt-0.5 text-[13.5px] text-muted">{t("dashboard.subtitle")}</p>
        </div>
        <div className="flex gap-2">
          <Button icon={<Icon name="power" size={16} />} onClick={() => void commands.setBufferEnabled(!bufferOn)}>
            {bufferOn ? t("dashboard.bufferOff") : t("dashboard.bufferOn")}
          </Button>
          <Button
            variant={recording ? "danger" : "secondary"}
            icon={<Icon name={recording ? "square" : "record"} size={16} className={recording ? "" : "text-danger"} />}
            onClick={() => void commands.toggleRecording()}
          >
            {recording ? t("dashboard.stopRecording") : t("dashboard.startRecording")}
          </Button>
          <Button variant="primary" icon={<Icon name="clapper" size={16} />} disabled={!bufferOn} onClick={() => void commands.saveClip()}>
            {t("dashboard.saveClip")}
          </Button>
        </div>
      </div>

      <div className="grid grid-cols-[auto_1fr] gap-4">
        <Card index={0} className="row-span-2 flex flex-col items-center justify-center gap-5 px-10 py-8">
          <BufferRing fill={bufferOn ? buffered / target : 0} recording={recording} active={bufferOn || recording}>
            <div>
              <div className="tabular text-[44px] leading-none font-semibold tracking-tight">
                {bufferOn ? shownBuffered.toFixed(1) : "—"}
                <span className="text-[18px] font-medium text-muted"> s</span>
              </div>
              <div className="mt-2 text-[12.5px] text-muted">
                {t("dashboard.buffered")} · {t("dashboard.of")} {target} s
              </div>
              {recording && (
                <div className="anim-rise-in tabular mt-3 inline-flex items-center gap-1.5 rounded-full bg-danger/15 px-2.5 py-1 text-[12px] font-semibold text-danger">
                  <span className="anim-blink size-1.5 rounded-full bg-danger" />
                  REC {formatDuration(status?.recordingSecs ?? 0)}
                </div>
              )}
            </div>
          </BufferRing>
          <div className="flex items-center gap-2 text-[12.5px] text-muted">
            <span className={`size-2 rounded-full ${status?.state === "error" ? "bg-danger" : bufferOn ? "bg-accent" : "bg-subtle"}`} />
            <span key={status?.state} className="anim-fade-in">
              {t(`state.${status?.state ?? "idle"}`)}
            </span>
            {status?.micMuted && (
              <span className="anim-fade-in ml-2 inline-flex items-center gap-1 text-danger">
                <Icon name="micOff" size={14} /> {t("dashboard.micMuted")}
              </span>
            )}
          </div>
          {status?.lastError && status.state === "error" && (
            <p className="anim-fade-in max-w-64 text-center text-[12px] text-danger">{status.lastError}</p>
          )}
        </Card>

        <div className="grid grid-cols-2 gap-4">
          <Card index={1} className="col-span-2 p-4">
            <div className="mb-2.5 flex items-center justify-between text-[12.5px]">
              <span className="flex items-center gap-2 text-muted">
                <Icon name="memory" size={16} /> {t("dashboard.ram")}
              </span>
              <span className="tabular font-medium">
                <CountUp value={status?.bufferBytes ?? 0} format={formatBytes} /> / {formatBytes(status?.bufferLimitBytes ?? 0)}
              </span>
            </div>
            <div className="h-1.5 overflow-hidden rounded-full bg-surface-2">
              <div className="tween h-full w-full origin-left bg-accent" style={{ transform: `scaleX(${ramPct})` }} />
            </div>
          </Card>
          <Stat
            index={2}
            icon="gauge"
            label={t("dashboard.bitrate")}
            value={<CountUp value={(status?.bitrateKbps ?? 0) / 1000} format={(v) => `${v.toFixed(1)} Mbit/s`} />}
            sub={
              <>
                <CountUp value={status?.fps ?? 0} format={(v) => v.toFixed(0)} /> {t("dashboard.fps")} · {status?.droppedFrames ?? 0}{" "}
                {t("dashboard.dropped")}
              </>
            }
          />
          <Stat index={3} icon="cpu" label={t("dashboard.encoder")} value={status?.encoder || "—"} sub={status?.resolution} />
          <Stat
            index={4}
            icon="drive"
            label={t("dashboard.processRam")}
            value={<CountUp value={status?.processRamBytes ?? 0} format={formatBytes} />}
          />
          <Stat index={5} icon="monitor" label={t("dashboard.source")} value={status?.source || "—"} sub={status?.game ?? t("dashboard.noGame")} />
          <Card index={6} className="col-span-2 p-4">
            <div className="mb-3 text-[12.5px] text-muted">{t("dashboard.hotkeys")}</div>
            <div className="grid grid-cols-2 gap-x-8 gap-y-2.5">
              {hotkeys.map(([key, accel]) => (
                <div key={key} className="flex items-center justify-between gap-2 text-[13px]">
                  <span className="truncate">{t(key)}</span>
                  <span className="flex shrink-0 gap-1">
                    {accel ? (
                      keycaps(accel).map((k) => <Keycap key={k} k={k} small />)
                    ) : (
                      <span className="text-[12px] text-subtle">{t("dashboard.notSet")}</span>
                    )}
                  </span>
                </div>
              ))}
            </div>
          </Card>
        </div>
      </div>

      <div className="mt-9 mb-3 flex items-center justify-between">
        <h2 className="text-[15px] font-semibold">{t("dashboard.recent")}</h2>
        <Button variant="ghost" size="sm" onClick={() => go("library")}>
          {t("dashboard.showAll")}
        </Button>
      </div>
      {clips.length === 0 && loaded ? (
        <Card index={7} className="p-6 text-center text-[13px] text-muted">
          {t("dashboard.noClips", { hotkey: settings?.hotkeys.saveClip || "—", secs: target })}
        </Card>
      ) : (
        <div className="grid grid-cols-4 gap-4">
          {clips.slice(0, 4).map((c, i) => (
            <ClipThumb key={c.path} clip={c} index={7 + i} onOpen={() => go("library")} compact />
          ))}
        </div>
      )}
    </div>
  );
}
