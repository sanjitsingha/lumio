//! Versioned settings migrations.
//!
//! Every settings file carries `schemaVersion`. When the schema changes,
//! bump `settings::SCHEMA_VERSION` and append a step to [`MIGRATIONS`] that
//! upgrades a document from `from` to `from + 1`. Steps operate on raw JSON so
//! they keep working even after the Rust structs have moved on.

use serde_json::{Map, Value};

use super::settings::SCHEMA_VERSION;

/// A single upgrade step: mutates a document at version `from` into `from + 1`.
pub struct Migration {
    pub from: u32,
    pub describe: &'static str,
    pub apply: fn(&mut Map<String, Value>),
}

/// Registered migrations, ordered by `from`.
pub const MIGRATIONS: &[Migration] = &[Migration {
    from: 1,
    describe: "one Spotlight shortcut instead of two",
    apply: single_spotlight_shortcut,
}];

/// v1 had a second Spotlight shortcut (Ctrl+Space) next to Alt+Space, which
/// Linux desktops keep for the window menu, and two optional shortcuts that
/// no longer exist. On Linux the second one takes the remaining place when
/// the first was Alt+Space or empty, so the shortcut that worked keeps working.
fn single_spotlight_shortcut(obj: &mut Map<String, Value>) {
    let Some(bindings) = obj.get_mut("shortcuts").and_then(|s| s.get_mut("bindings")).and_then(Value::as_object_mut)
    else {
        return;
    };
    let alternate = bindings.remove("spotlight.toggleAlternate");
    bindings.remove("settings.open");
    bindings.remove("capsLock.toggle");
    let primary = bindings.get("spotlight.toggle").and_then(Value::as_str);
    if cfg!(target_os = "linux") && matches!(primary, None | Some("Alt+Space")) {
        if let Some(Value::String(alternate)) = alternate {
            bindings.insert("spotlight.toggle".into(), Value::String(alternate));
        }
    }
}

#[derive(Debug, PartialEq, Eq)]
pub enum MigrationOutcome {
    Current,
    Migrated { from: u32 },
    /// Written by a newer build; loaded best-effort (unknown keys are ignored).
    NewerThanSupported { found: u32 },
}

pub fn migrate(value: &mut Value) -> Result<MigrationOutcome, String> {
    migrate_with(value, SCHEMA_VERSION, MIGRATIONS)
}

/// Runs `steps` until the document reaches `current`. Documents without a
/// version are treated as v1 (the first schema that existed).
pub fn migrate_with(value: &mut Value, current: u32, steps: &[Migration]) -> Result<MigrationOutcome, String> {
    let obj = value.as_object_mut().ok_or("settings document is not an object")?;
    let found = match obj.get("schemaVersion") {
        None => 1,
        Some(v) => v
            .as_u64()
            .and_then(|n| u32::try_from(n).ok())
            .ok_or_else(|| format!("schemaVersion must be a positive integer, got {v}"))?,
    };
    if found == current {
        return Ok(MigrationOutcome::Current);
    }
    if found > current {
        return Ok(MigrationOutcome::NewerThanSupported { found });
    }
    let mut version = found;
    while version < current {
        let step = steps
            .iter()
            .find(|m| m.from == version)
            .ok_or_else(|| format!("no migration registered from schema v{version}"))?;
        log::info!("settings migration v{} -> v{}: {}", step.from, step.from + 1, step.describe);
        (step.apply)(obj);
        version += 1;
        obj.insert("schemaVersion".into(), Value::from(version));
    }
    Ok(MigrationOutcome::Migrated { from: found })
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn v1_keeps_the_spotlight_shortcut_that_worked() {
        let mut doc = json!({ "schemaVersion": 1, "shortcuts": { "bindings": {
            "spotlight.toggle": "Alt+Space", "spotlight.toggleAlternate": "Ctrl+Space",
            "settings.open": null, "capsLock.toggle": null
        } } });
        assert_eq!(migrate(&mut doc), Ok(MigrationOutcome::Migrated { from: 1 }));
        let expected = if cfg!(target_os = "linux") { "Ctrl+Space" } else { "Alt+Space" };
        assert_eq!(doc["shortcuts"]["bindings"], json!({ "spotlight.toggle": expected }));
        assert_eq!(doc["schemaVersion"], 2);
    }

    #[test]
    fn v1_custom_spotlight_shortcut_is_kept() {
        let mut doc = json!({ "schemaVersion": 1, "shortcuts": { "bindings": {
            "spotlight.toggle": "Super+Space", "spotlight.toggleAlternate": "Ctrl+Space"
        } } });
        migrate(&mut doc).unwrap();
        assert_eq!(doc["shortcuts"]["bindings"], json!({ "spotlight.toggle": "Super+Space" }));
    }

    fn rename_theme(obj: &mut Map<String, Value>) {
        if let Some(theme) = obj.remove("theme") {
            obj.insert("appearance".into(), json!({ "theme": theme }));
        }
    }

    fn add_caps_section(obj: &mut Map<String, Value>) {
        obj.entry("capsLock").or_insert_with(|| json!({ "enabled": true }));
    }

    const TEST_STEPS: &[Migration] = &[
        Migration { from: 1, describe: "move theme into appearance", apply: rename_theme },
        Migration { from: 2, describe: "add caps lock section", apply: add_caps_section },
    ];

    #[test]
    fn current_version_is_untouched() {
        let mut doc = json!({ "schemaVersion": SCHEMA_VERSION, "general": {} });
        let before = doc.clone();
        assert_eq!(migrate(&mut doc).unwrap(), MigrationOutcome::Current);
        assert_eq!(doc, before);
    }

    #[test]
    fn runs_every_step_in_order() {
        let mut doc = json!({ "schemaVersion": 1, "theme": "dark" });
        let outcome = migrate_with(&mut doc, 3, TEST_STEPS).unwrap();
        assert_eq!(outcome, MigrationOutcome::Migrated { from: 1 });
        assert_eq!(doc["schemaVersion"], 3);
        assert_eq!(doc["appearance"]["theme"], "dark");
        assert_eq!(doc["capsLock"]["enabled"], true);
        assert!(doc.get("theme").is_none());
    }

    #[test]
    fn partial_chain_starts_at_found_version() {
        let mut doc = json!({ "schemaVersion": 2, "theme": "dark" });
        migrate_with(&mut doc, 3, TEST_STEPS).unwrap();
        // step 1 did not run, so the legacy key is untouched
        assert_eq!(doc["theme"], "dark");
        assert!(doc.get("capsLock").is_some());
    }

    #[test]
    fn missing_version_is_treated_as_v1() {
        let mut doc = json!({ "theme": "light" });
        assert_eq!(migrate_with(&mut doc, 2, TEST_STEPS).unwrap(), MigrationOutcome::Migrated { from: 1 });
        assert_eq!(doc["appearance"]["theme"], "light");
    }

    #[test]
    fn newer_versions_are_reported_not_modified() {
        let mut doc = json!({ "schemaVersion": 99 });
        assert_eq!(migrate_with(&mut doc, 3, TEST_STEPS).unwrap(), MigrationOutcome::NewerThanSupported { found: 99 });
        assert_eq!(doc["schemaVersion"], 99);
    }

    #[test]
    fn gaps_in_the_chain_are_errors() {
        let mut doc = json!({ "schemaVersion": 1 });
        assert!(migrate_with(&mut doc, 3, &TEST_STEPS[1..]).is_err());
    }

    #[test]
    fn invalid_version_values_are_errors() {
        assert!(migrate(&mut json!({ "schemaVersion": "one" })).is_err());
        assert!(migrate(&mut json!({ "schemaVersion": -4 })).is_err());
        assert!(migrate(&mut json!([1, 2])).is_err());
    }
}
