import { useEffect, useState } from "react";

import { Field, Section } from "@/components/ui/Field";
import { KeycapInput } from "@/components/ui/KeycapInput";
import { useT, type TKey } from "@/i18n";
import { commands, on, type HotkeyConflict, type Settings } from "@/lib/ipc";
import { useSection } from "../useSettingsDraft";

type Action = keyof Settings["hotkeys"];
const ACTIONS: Action[] = ["saveClip", "toggleRecording", "toggleBuffer", "toggleMicMute", "pushToTalk"];

export function HotkeysSection() {
  const t = useT();
  const [h, set] = useSection("hotkeys");
  const [conflicts, setConflicts] = useState<HotkeyConflict[]>([]);

  // Statische Prüfung sofort, „von anderer App belegt“ kommt nach dem Speichern als Event.
  useEffect(() => {
    void commands.validateHotkeys(h).then((c) =>
      setConflicts((prev) => [...c, ...prev.filter((p) => p.kind === "inUse" && h[p.action] === p.accelerator)]),
    );
  }, [h]);
  useEffect(() => {
    void commands.hotkeyConflicts().then(setConflicts);
    const un = on("hotkey-conflicts", setConflicts);
    return () => void un.then((f) => f());
  }, []);

  const message = (a: Action) => {
    const c = conflicts.find((x) => x.action === a && x.accelerator === h[a]);
    if (!c) return undefined;
    if (c.kind === "duplicate" && c.other) {
      return t("settings.hotkeys.conflict.duplicate", { other: t(`settings.hotkeys.${c.other}` as TKey) });
    }
    return t(`settings.hotkeys.conflict.${c.kind}` as TKey);
  };

  return (
    <Section>
      {ACTIONS.map((a) => (
        <Field key={a} label={t(`settings.hotkeys.${a}` as TKey)}>
          <KeycapInput
            label={t(`settings.hotkeys.${a}` as TKey)}
            value={h[a]}
            error={message(a)}
            onChange={(accel) => set({ [a]: accel } as Partial<Settings["hotkeys"]>)}
          />
        </Field>
      ))}
    </Section>
  );
}
