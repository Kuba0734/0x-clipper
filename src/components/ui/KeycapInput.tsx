import { useState } from "react";

import { useT } from "@/i18n";
import { commands } from "@/lib/ipc";
import { keycaps } from "@/lib/format";
import { Icon } from "./Icon";

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

/** Einzelne Taste; erscheint mit Pop, beim Drücken kurzer Press-Effekt (scale). */
export function Keycap({ k, pressed, small }: { k: string; pressed?: boolean; small?: boolean }) {
  return (
    <kbd
      className={`anim-key-pop inline-flex items-center justify-center rounded-sm border border-border-strong border-b-[3px] bg-surface-2 font-sans font-semibold text-text ${
        small ? "h-6 min-w-6 px-1.5 text-[11px]" : "h-7 min-w-7 px-2 text-[12px]"
      }`}
      style={{
        transform: pressed ? "translateY(1px) scale(0.94)" : undefined,
        transition: "transform var(--dur-fast) var(--ease)",
      }}
    >
      {k}
    </kbd>
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
          className={`press hoverable flex h-10 min-w-44 items-center justify-center gap-1.5 rounded-md border px-3 ${
            recording ? "border-accent bg-surface-2" : error ? "border-danger/70 bg-surface" : "border-border bg-surface"
          }`}
        >
          {caps.length > 0 ? (
            caps.map((k, i) => <Keycap key={`${k}-${i}`} k={k} pressed={recording && pressed} />)
          ) : (
            <span key={recording ? "rec" : "idle"} className="anim-fade-in text-[13px] text-muted">
              {recording ? t("settings.hotkeys.press") : t("dashboard.notSet")}
            </span>
          )}
        </button>
        {value && !recording && (
          <button
            type="button"
            aria-label={t("settings.hotkeys.clear")}
            title={t("settings.hotkeys.clear")}
            onClick={() => onChange("")}
            className="press hoverable flex size-8 items-center justify-center rounded-sm text-muted hover:text-text"
          >
            <Icon name="x" size={16} />
          </button>
        )}
      </div>
      {error && (
        <span key={error} className="anim-rise-in text-[12px] text-danger">
          {error}
        </span>
      )}
    </div>
  );
}
