//! Migrationen zwischen Settings-Versionen. Jede Migration hebt genau eine Version an
//! und arbeitet auf rohem JSON, damit alte Felder noch lesbar sind.

use serde_json::{Map, Value};

use super::schema::SETTINGS_VERSION;

type Migration = fn(&mut Map<String, Value>);

/// Index i migriert von Version i auf i + 1.
const MIGRATIONS: [Migration; SETTINGS_VERSION as usize] = [v0_to_v1];

/// v0 = Dateien ohne `version`-Feld (Vorabversionen). Dort hieß die Buffer-Länge
/// noch `bufferSeconds` auf oberster Ebene.
fn v0_to_v1(root: &mut Map<String, Value>) {
    if let Some(secs) = root.remove("bufferSeconds") {
        let replay = root.entry("replay").or_insert_with(|| Value::Object(Map::new()));
        if let Value::Object(replay) = replay {
            replay.entry("durationSecs").or_insert(secs);
        }
    }
}

/// Liefert die Version der Datei und migriert sie auf den aktuellen Stand.
/// Neuere Versionen (Downgrade) werden unverändert gelassen; unbekannte Felder ignoriert serde.
pub fn migrate(mut value: Value) -> (Value, u32) {
    let Value::Object(root) = &mut value else {
        return (Value::Object(Map::new()), 0);
    };
    let from = root.get("version").and_then(Value::as_u64).unwrap_or(0) as u32;
    for (v, step) in MIGRATIONS.iter().enumerate().skip(from as usize) {
        step(root);
        root.insert("version".into(), Value::from(v as u32 + 1));
    }
    (value, from)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::settings::schema::Settings;
    use serde_json::json;

    #[test]
    fn v0_moves_buffer_seconds() {
        let (v, from) = migrate(json!({ "bufferSeconds": 45 }));
        assert_eq!(from, 0);
        let s: Settings = serde_json::from_value(v).unwrap();
        assert_eq!(s.version, SETTINGS_VERSION);
        assert_eq!(s.replay.duration_secs, 45);
    }

    #[test]
    fn current_version_is_untouched() {
        let original = serde_json::to_value(Settings::default()).unwrap();
        let (v, from) = migrate(original.clone());
        assert_eq!(from, SETTINGS_VERSION);
        assert_eq!(v, original);
    }

    #[test]
    fn garbage_yields_defaults() {
        let (v, _) = migrate(json!([1, 2, 3]));
        let s: Settings = serde_json::from_value(v).unwrap();
        assert_eq!(s.hotkeys.save_clip, "Alt+F10");
    }

    #[test]
    fn partial_file_fills_defaults_and_sanitizes() {
        let (v, _) = migrate(json!({ "version": 1, "replay": { "durationSecs": 99999 } }));
        let mut s: Settings = serde_json::from_value(v).unwrap();
        s.sanitize();
        assert_eq!(s.replay.duration_secs, 600);
        assert!(s.replay.enabled);
    }
}
