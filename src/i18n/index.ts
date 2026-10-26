import { useCallback } from "react";

import { useSettings } from "@/stores/settings";
import de from "./de.json";
import en from "./en.json";

type Dict = typeof de;
type Leaves<T, P extends string = ""> = {
  [K in keyof T & string]: T[K] extends string ? `${P}${K}` : Leaves<T[K], `${P}${K}.`>;
}[keyof T & string];
export type TKey = Leaves<Dict>;
export type Vars = Record<string, string | number>;

const dicts: Record<"de" | "en", Dict> = { de, en };

function lookup(dict: Dict, key: string): string {
  let cur: unknown = dict;
  for (const part of key.split(".")) {
    if (cur && typeof cur === "object" && part in cur) {
      cur = (cur as Record<string, unknown>)[part];
    } else {
      return key;
    }
  }
  return typeof cur === "string" ? cur : key;
}

export function translate(lang: "de" | "en", key: TKey, vars?: Vars): string {
  const text = lookup(dicts[lang], key);
  if (!vars) return text;
  return text.replace(/\{(\w+)\}/g, (m, name: string) =>
    name in vars ? String(vars[name]) : m,
  );
}

/** Übersetzungsfunktion für die aktuelle Sprache aus den Einstellungen. */
export function useT() {
  const lang = useSettings((s) => s.settings?.general.language ?? "de");
  return useCallback((key: TKey, vars?: Vars) => translate(lang, key, vars), [lang]);
}

export function useLang() {
  return useSettings((s) => s.settings?.general.language ?? "de");
}
