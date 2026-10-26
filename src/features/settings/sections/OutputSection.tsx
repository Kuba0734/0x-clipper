import { open } from "@tauri-apps/plugin-dialog";
import { FolderOpen, RotateCcw } from "lucide-react";
import { useEffect, useState } from "react";

import { Button, IconButton } from "@/components/ui/Button";
import { Field, Section } from "@/components/ui/Field";
import { NumberInput } from "@/components/ui/NumberInput";
import { Toggle } from "@/components/ui/Toggle";
import { useT } from "@/i18n";
import { commands } from "@/lib/ipc";
import { useSection } from "../useSettingsDraft";

function previewName(pattern: string) {
  const now = new Date();
  const pad = (n: number) => String(n).padStart(2, "0");
  const date = `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`;
  const time = `${pad(now.getHours())}-${pad(now.getMinutes())}-${pad(now.getSeconds())}`;
  return `${pattern.replaceAll("{game}", "Valorant").replaceAll("{date}", date).replaceAll("{time}", time).replaceAll("{type}", "Clip").replace(/[<>:"/\\|?*]/g, "_")}.mp4`;
}

export function OutputSection() {
  const t = useT();
  const [o, set] = useSection("output");
  const [rec, setRec] = useSection("recording");
  const [defaultDir, setDefaultDir] = useState("");
  const [draft, setDraft] = useState<string | null>(null);
  const pattern = draft ?? o.filenamePattern;
  useEffect(() => {
    void commands.defaultOutputDir().then(setDefaultDir);
  }, [o.folder]);

  const choose = async () => {
    const dir = await open({ directory: true, defaultPath: defaultDir });
    if (typeof dir === "string") set({ folder: dir });
  };

  return (
    <>
      <Section>
        <Field label={t("settings.output.folder")} hint={<span className="break-all">{o.folder || defaultDir}</span>}>
          {o.folder && (
            <IconButton label={t("settings.output.default")} onClick={() => set({ folder: "" })}>
              <RotateCcw className="size-4" />
            </IconButton>
          )}
          <Button size="sm" icon={<FolderOpen className="size-4" />} onClick={() => void choose()}>
            {t("settings.output.choose")}
          </Button>
        </Field>
        <Field
          label={t("settings.output.pattern")}
          hint={`${t("settings.output.patternHint")} · ${t("settings.output.preview")}: ${previewName(pattern)}`}
          stacked
        >
          <input
            value={pattern}
            onChange={(e) => setDraft(e.target.value)}
            onBlur={() => {
              if (pattern !== o.filenamePattern) set({ filenamePattern: pattern });
              setDraft(null);
            }}
            className="h-9 w-full rounded-[10px] border border-border bg-surface-2 px-3 font-mono text-[13px] outline-none focus-visible:border-accent"
          />
        </Field>
        <Field label={t("settings.output.subfolder")}>
          <Toggle checked={o.subfolderPerGame} onChange={(subfolderPerGame) => set({ subfolderPerGame })} label={t("settings.output.subfolder")} />
        </Field>
        <Field label={t("settings.output.lowSpace")} hint={t("settings.output.lowSpaceHint")}>
          <NumberInput value={o.lowSpaceWarningGb} min={0} max={10000} suffix="GB" onChange={(lowSpaceWarningGb) => set({ lowSpaceWarningGb })} />
        </Field>
      </Section>
      <Section>
        <Field label={t("settings.output.cleanup")} hint={t("settings.output.cleanupHint")}>
          <Toggle
            checked={o.cleanup.enabled}
            onChange={(enabled) => set({ cleanup: { ...o.cleanup, enabled } })}
            label={t("settings.output.cleanup")}
          />
        </Field>
        <Field label={t("settings.output.maxAge")}>
          <NumberInput
            value={o.cleanup.maxAgeDays}
            min={0}
            max={3650}
            suffix={t("settings.output.days")}
            width="w-28"
            onChange={(maxAgeDays) => set({ cleanup: { ...o.cleanup, maxAgeDays } })}
          />
        </Field>
        <Field label={t("settings.output.maxTotal")}>
          <NumberInput
            value={o.cleanup.maxTotalGb}
            min={0}
            max={100000}
            suffix="GB"
            onChange={(maxTotalGb) => set({ cleanup: { ...o.cleanup, maxTotalGb } })}
          />
        </Field>
      </Section>
      <Section>
        <Field label={t("settings.output.recMaxDuration")} hint={`0 = ${t("settings.output.off")}`}>
          <NumberInput value={rec.maxDurationMins} min={0} max={1440} suffix="min" onChange={(maxDurationMins) => setRec({ maxDurationMins })} />
        </Field>
        <Field label={t("settings.output.recSplit")} hint={`0 = ${t("settings.output.off")}`}>
          <NumberInput value={rec.splitSizeMb} min={0} max={1000000} suffix="MB" width="w-32" onChange={(splitSizeMb) => setRec({ splitSizeMb })} />
        </Field>
      </Section>
    </>
  );
}
