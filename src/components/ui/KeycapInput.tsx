import { AnimatePresence, m } from "motion/react";
import { X } from "lucide-react";
import { useState } from "react";

import { useT } from "@/i18n";
import { commands } from "@/lib/ipc";
import { keycaps } from "@/lib/format";
import { spring } from "@/lib/motion";

interface Props {
  value: string;
  onChange: (accel: string) => void;
  error?: string;
  label: string;
}

const MODIFIERS = new Set(["ControlLeft", "ControlRight", "AltLeft", "AltRight", "ShiftLeft", "ShiftRight", "MetaLeft", "MetaRight"]);

/** KeyboardEvent.code → Accelerator-Taste von tauri-plugin-global-shortcut. */
function keyName(code: string): string | null {
  if (MODIFIERS.has(code)) return null;
  if (/^Key[A-Z]$/.test(code)) return code.slice(3);
  if (/^Digit\d$/.test(code)) return code.slice(5);
  if (/^F\d{1,2}$/.test(code)) return code;
  if (/^Numpad/.test(code)) return code;
  const map: Record<string, string> = {
    Space: "Space",
    Enter: "Enter",
    Tab: "Tab",
    Insert: "Insert",
    Delete: "Delete",
    Home: "Home",
    End: "End",
    PageUp: "PageUp",
    PageDown: "PageDown",
    ArrowUp: "Up",
    ArrowDown: "Down",
    ArrowLeft: "Left",
    ArrowRight: "Right",
    Backquote: "Backquote",
    Minus: "Minus",
    Equal: "Equal",
    BracketLeft: "BracketLeft",
    BracketRight: "BracketRight",
    Backslash: "Backslash",
    Semicolon: "Semicolon",
    Quote: "Quote",
    Comma: "Comma",
    Period: "Period",
    Slash: "Slash",
    PrintScreen: "PrintScreen",
    ScrollLock: "ScrollLock",
    Pause: "Pause",
  };
  return map[code] ?? null;
}

function Keycap({ k, pressed }: { k: string; pressed?: boolean }) {
  return (
    <m.kbd
      layout
      initial={{ opacity: 0, y: -4, scale: 0.9 }}
      animate={{ opacity: 1, y: pressed ? 1 : 0, scale: pressed ? 0.94 : 1 }}
      exit={{ opacity: 0, scale: 0.9 }}
      transition={spring}
      className="inline-flex h-7 min-w-7 items-center justify-center rounded-[8px] border border-border border-b-[3px] bg-surface-2 px-2 font-sans text-[12px] font-semibold text-text"
    >
      {k}
    </m.kbd>
  );
}

/**
 * Hotkey-Eingabe als „Keycaps“. Während der Aufnahme sind globale Hotkeys pausiert,
 * damit bestehende Kombinationen nicht auslösen.
 */
export function KeycapInput({ value, onChange, error, label }: Props) {
  const t = useT();
  const [recording, setRecording] = useState(false);
  const [live, setLive] = useState<string[]>([]);
  const [pressed, setPressed] = useState(false);

  const start = () => {
    setRecording(true);
    setLive([]);
    void commands.suspendHotkeys(true);
  };
  const stop = () => {
    setRecording(false);
    setLive([]);
    void commands.suspendHotkeys(false);
  };

  const onKeyDown = (e: React.KeyboardEvent) => {
    if (!recording) {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        start();
      }
      return;
    }
    e.preventDefault();
    e.stopPropagation();
    if (e.code === "Escape") return stop();
    if ((e.code === "Backspace" || e.code === "Delete") && !e.ctrlKey && !e.altKey && !e.shiftKey) {
      onChange("");
      return stop();
    }
    const mods = [e.ctrlKey && "Ctrl", e.altKey && "Alt", e.shiftKey && "Shift"].filter(Boolean) as string[];
    const key = keyName(e.code);
    setPressed(true);
    setLive(key ? [...mods, key] : mods);
    if (key) {
      onChange([...mods, key].join("+"));
      window.setTimeout(stop, 220);
    }
  };

  const caps = recording ? live : keycaps(value);
  return (
    <div className="flex flex-col items-end gap-1">
      <div className="flex items-center gap-1.5">
        <button
          type="button"
          aria-label={label}
          onClick={() => (recording ? stop() : start())}
          onKeyDown={onKeyDown}
          onKeyUp={() => setPressed(false)}
          onBlur={() => recording && stop()}
          className={`flex h-10 min-w-44 items-center justify-center gap-1.5 rounded-[12px] border px-3 transition-colors ${
            recording
              ? "border-accent bg-accent/10"
              : error
                ? "border-danger/70 bg-surface-2"
                : "border-border bg-surface-2 hover:border-muted/40"
          }`}
        >
          <AnimatePresence mode="popLayout" initial={false}>
            {caps.length > 0 ? (
              caps.map((k, i) => <Keycap key={`${k}-${i}`} k={k} pressed={recording && pressed} />)
            ) : (
              <m.span key="placeholder" initial={{ opacity: 0 }} animate={{ opacity: 1 }} className="text-[13px] text-muted">
                {recording ? t("settings.hotkeys.press") : t("dashboard.notSet")}
              </m.span>
            )}
          </AnimatePresence>
        </button>
        {value && !recording && (
          <button
            type="button"
            aria-label={t("settings.hotkeys.clear")}
            title={t("settings.hotkeys.clear")}
            onClick={() => onChange("")}
            className="flex size-8 items-center justify-center rounded-[10px] text-muted hover:bg-surface-2 hover:text-text"
          >
            <X className="size-4" />
          </button>
        )}
      </div>
      <AnimatePresence>
        {error && (
          <m.span
            initial={{ opacity: 0, y: -4 }}
            animate={{ opacity: 1, y: 0 }}
            exit={{ opacity: 0 }}
            className="text-[12px] text-danger"
          >
            {error}
          </m.span>
        )}
      </AnimatePresence>
    </div>
  );
}
