use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::json_store;
use super::migrate::{self, MigrationOutcome};
use crate::shortcuts::accelerator::Accelerator;
use crate::shortcuts::actions::SLOTS;

/// Current settings schema. Bump together with a new entry in `migrate::MIGRATIONS`.
pub const SCHEMA_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub schema_version: u32,
    pub general: GeneralSettings,
    pub spotlight: SpotlightSettings,
    pub shortcuts: ShortcutSettings,
    pub caps_lock: CapsLockSettings,
    pub appearance: AppearanceSettings,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            general: GeneralSettings::default(),
            spotlight: SpotlightSettings::default(),
            shortcuts: ShortcutSettings::default(),
            caps_lock: CapsLockSettings::default(),
            appearance: AppearanceSettings::default(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GeneralSettings {
    pub launch_at_startup: bool,
    /// When launched at login, stay in the background instead of opening Settings.
    pub start_minimized: bool,
    /// Keep shortcuts and the Caps Lock indicator active with no windows open.
    pub run_in_background: bool,
    pub show_tray_icon: bool,
    /// Check the release feed in the background (at startup and every few hours).
    pub check_for_updates: bool,
    /// Download and install found updates without asking, then restart Lumio.
    pub auto_install_updates: bool,
    /// The Settings close button keeps Lumio running instead of quitting it.
    pub close_to_background: bool,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self {
            launch_at_startup: false,
            start_minimized: true,
            run_in_background: true,
            show_tray_icon: true,
            check_for_updates: true,
            auto_install_updates: false,
            close_to_background: true,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SpotlightSettings {
    pub enabled: bool,
    pub search_applications: bool,
    pub search_files: bool,
    pub search_folders: bool,
    pub search_actions: bool,
    pub keep_history: bool,
    pub max_results: u32,
    pub search_delay_ms: u32,
    /// Folder names (e.g. `node_modules`) or absolute paths skipped by the file index.
    pub exclusions: Vec<String>,
    /// Folders indexed for file search. Empty means the user's home folder.
    pub index_roots: Vec<String>,
    pub index_hidden: bool,
}

pub const MAX_RESULTS_RANGE: (u32, u32) = (5, 60);
pub const SEARCH_DELAY_RANGE: (u32, u32) = (0, 500);

impl Default for SpotlightSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            search_applications: true,
            search_files: true,
            search_folders: true,
            search_actions: true,
            keep_history: true,
            max_results: 24,
            search_delay_ms: 60,
            exclusions: default_exclusions(),
            index_roots: Vec::new(),
            index_hidden: false,
        }
    }
}

pub fn default_exclusions() -> Vec<String> {
    [
        "node_modules", ".git", ".hg", ".svn", "target", "__pycache__", ".venv", "venv",
        ".cache", ".npm", ".cargo", ".rustup", "snap", "AppData", "$RECYCLE.BIN",
    ]
    .iter()
    .map(|s| s.to_string())
    .collect()
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct ShortcutSettings {
    /// Shortcut slot id -> accelerator (`None` = cleared by the user).
    pub bindings: BTreeMap<String, Option<String>>,
}

impl Default for ShortcutSettings {
    fn default() -> Self {
        Self {
            bindings: SLOTS.iter().map(|s| (s.id.to_string(), s.default.map(str::to_string))).collect(),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum IndicatorPosition {
    Near,
    Below,
    Above,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AnimationSpeed {
    Off,
    Fast,
    Normal,
    Slow,
}

impl AnimationSpeed {
    pub fn duration_ms(self) -> u64 {
        match self {
            AnimationSpeed::Off => 0,
            AnimationSpeed::Fast => 90,
            AnimationSpeed::Normal => 170,
            AnimationSpeed::Slow => 300,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum AppearanceMode {
    System,
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum WaylandFallback {
    /// Use the compositor's on-screen display (elementary OS / Gala).
    Osd,
    None,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CapsLockSettings {
    pub enabled: bool,
    pub position: IndicatorPosition,
    /// Edge length of the indicator in logical pixels.
    pub size: u32,
    pub opacity: f32,
    /// How long the indicator stays visible. `0` = while Caps Lock is on.
    pub duration_ms: u32,
    pub animation: AnimationSpeed,
    pub appearance: AppearanceMode,
    /// Briefly show the "off" state when Caps Lock is turned off.
    pub show_when_disabled: bool,
    pub follow_cursor: bool,
    pub wayland_fallback: WaylandFallback,
}

pub const INDICATOR_SIZE_RANGE: (u32, u32) = (24, 96);
pub const INDICATOR_OPACITY_RANGE: (f32, f32) = (0.3, 1.0);
pub const INDICATOR_DURATION_MAX: u32 = 10_000;

impl Default for CapsLockSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            position: IndicatorPosition::Near,
            size: 40,
            opacity: 0.96,
            duration_ms: 1500,
            animation: AnimationSpeed::Normal,
            appearance: AppearanceMode::System,
            show_when_disabled: true,
            follow_cursor: true,
            wayland_fallback: WaylandFallback::Osd,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum Accent {
    System,
    Blue,
    Purple,
    Pink,
    Red,
    Orange,
    Yellow,
    Green,
    Graphite,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum MotionPreference {
    System,
    Reduced,
    Full,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppearanceSettings {
    pub theme: AppearanceMode,
    pub accent: Accent,
    pub motion: MotionPreference,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self { theme: AppearanceMode::System, accent: Accent::Blue, motion: MotionPreference::System }
    }
}

impl Settings {
    /// Deserializes section by section so one malformed section (e.g. a string
    /// where a number belongs) falls back to defaults without losing the rest.
    pub fn from_value_lenient(value: &Value) -> (Settings, Vec<String>) {
        let mut warnings = Vec::new();
        let mut settings = Settings::default();
        let Some(obj) = value.as_object() else {
            warnings.push("Settings file did not contain an object; defaults were used.".into());
            return (settings, warnings);
        };
        fn section<T: DeserializeOwned + Default>(obj: &serde_json::Map<String, Value>, key: &str, warnings: &mut Vec<String>) -> T {
            match obj.get(key) {
                None => T::default(),
                Some(v) => serde_json::from_value(v.clone()).unwrap_or_else(|e| {
                    warnings.push(format!("The \"{key}\" settings were invalid ({e}) and have been reset."));
                    T::default()
                }),
            }
        }
        settings.general = section(obj, "general", &mut warnings);
        settings.spotlight = section(obj, "spotlight", &mut warnings);
        settings.shortcuts = section(obj, "shortcuts", &mut warnings);
        settings.caps_lock = section(obj, "capsLock", &mut warnings);
        settings.appearance = section(obj, "appearance", &mut warnings);
        settings.schema_version = SCHEMA_VERSION;
        (settings, warnings)
    }

    /// Clamps numeric ranges and repairs shortcut bindings. Returns a
    /// description of every correction made.
    pub fn sanitize(&mut self) -> Vec<String> {
        let mut fixes = Vec::new();
        let s = &mut self.spotlight;
        let clamped = s.max_results.clamp(MAX_RESULTS_RANGE.0, MAX_RESULTS_RANGE.1);
        if clamped != s.max_results {
            fixes.push(format!("Maximum results adjusted to {clamped}."));
            s.max_results = clamped;
        }
        let clamped = s.search_delay_ms.clamp(SEARCH_DELAY_RANGE.0, SEARCH_DELAY_RANGE.1);
        if clamped != s.search_delay_ms {
            fixes.push(format!("Search delay adjusted to {clamped} ms."));
            s.search_delay_ms = clamped;
        }
        let before = s.exclusions.len();
        let mut seen = HashSet::new();
        s.exclusions = s
            .exclusions
            .iter()
            .map(|e| e.trim().to_string())
            .filter(|e| !e.is_empty() && seen.insert(e.clone()))
            .collect();
        if s.exclusions.len() != before {
            fixes.push("Removed empty or duplicate search exclusions.".into());
        }
        let before = s.index_roots.len();
        s.index_roots.retain(|r| !r.trim().is_empty() && Path::new(r.trim()).is_absolute());
        if s.index_roots.len() != before {
            fixes.push("Removed index folders that were not absolute paths.".into());
        }

        let c = &mut self.caps_lock;
        let size = c.size.clamp(INDICATOR_SIZE_RANGE.0, INDICATOR_SIZE_RANGE.1);
        if size != c.size {
            fixes.push(format!("Indicator size adjusted to {size}."));
            c.size = size;
        }
        if !c.opacity.is_finite() {
            c.opacity = CapsLockSettings::default().opacity;
            fixes.push("Indicator opacity reset.".into());
        }
        let opacity = c.opacity.clamp(INDICATOR_OPACITY_RANGE.0, INDICATOR_OPACITY_RANGE.1);
        if (opacity - c.opacity).abs() > f32::EPSILON {
            fixes.push("Indicator opacity adjusted.".into());
            c.opacity = opacity;
        }
        if c.duration_ms > INDICATOR_DURATION_MAX {
            c.duration_ms = INDICATOR_DURATION_MAX;
            fixes.push("Indicator duration adjusted.".into());
        }

        fixes.extend(self.shortcuts.sanitize());
        self.schema_version = SCHEMA_VERSION;
        fixes
    }
}

impl ShortcutSettings {
    pub fn sanitize(&mut self) -> Vec<String> {
        let mut fixes = Vec::new();
        let known: HashSet<&str> = SLOTS.iter().map(|s| s.id).collect();
        self.bindings.retain(|k, _| {
            let keep = known.contains(k.as_str());
            if !keep {
                fixes.push(format!("Removed unknown shortcut \"{k}\"."));
            }
            keep
        });
        let mut used: HashSet<Accelerator> = HashSet::new();
        for slot in SLOTS {
            let entry = self.bindings.entry(slot.id.to_string()).or_insert_with(|| slot.default.map(str::to_string));
            let Some(text) = entry.clone() else { continue };
            match text.parse::<Accelerator>() {
                Ok(accel) if used.insert(accel) => *entry = Some(accel.to_string()),
                Ok(accel) => {
                    fixes.push(format!("{accel} was assigned twice; cleared it for \"{}\".", slot.label));
                    *entry = None;
                }
                Err(_) => {
                    fixes.push(format!("Shortcut \"{text}\" for \"{}\" was invalid and has been reset.", slot.label));
                    *entry = slot.default.map(str::to_string).filter(|d| used.insert(d.parse().expect("valid default")));
                }
            }
        }
        fixes
    }
}

/// Result of loading settings from disk.
pub struct LoadedSettings {
    pub settings: Settings,
    pub first_run: bool,
    /// User-facing notes about repairs (corrupt file, invalid values, migrations).
    pub notices: Vec<String>,
}

/// Loads, migrates, validates and (when anything changed) re-saves settings.
pub fn load_settings(path: &Path) -> LoadedSettings {
    let mut notices = Vec::new();
    let mut needs_save = false;
    let raw = match json_store::read_json_value(path) {
        Ok(None) => {
            let settings = Settings::default();
            if let Err(e) = save_settings(path, &settings) {
                log::error!("could not write initial settings to {}: {e}", path.display());
            }
            return LoadedSettings { settings, first_run: true, notices };
        }
        Ok(Some(Ok(value))) => Some(value),
        Ok(Some(Err(parse_err))) => {
            log::error!("settings file {} is not valid JSON: {parse_err}", path.display());
            match json_store::quarantine(path, "corrupt") {
                Ok(backup) => notices.push(format!(
                    "Your settings file was damaged and has been reset. The old file was kept as {}.",
                    backup.display()
                )),
                Err(e) => log::error!("could not back up corrupt settings: {e}"),
            }
            needs_save = true;
            None
        }
        Err(io_err) => {
            log::error!("could not read settings {}: {io_err}", path.display());
            notices.push(format!("Settings could not be read ({io_err}); defaults are in use."));
            // Do not overwrite a file we failed to read (e.g. permissions).
            return LoadedSettings { settings: Settings::default(), first_run: false, notices };
        }
    };

    let mut settings = match raw {
        None => Settings::default(),
        Some(mut value) => {
            match migrate::migrate(&mut value) {
                Ok(MigrationOutcome::Current) => {}
                Ok(MigrationOutcome::Migrated { from }) => {
                    log::info!("migrated settings from schema v{from} to v{SCHEMA_VERSION}");
                    needs_save = true;
                }
                Ok(MigrationOutcome::NewerThanSupported { found }) => {
                    log::warn!("settings schema v{found} is newer than supported v{SCHEMA_VERSION}");
                    let backup = path.with_file_name(format!("settings.v{found}.backup.json"));
                    if std::fs::copy(path, &backup).is_ok() {
                        notices.push(format!(
                            "Settings were created by a newer version of the app. A copy was kept as {}.",
                            backup.display()
                        ));
                    }
                    needs_save = true;
                }
                Err(e) => {
                    log::error!("settings migration failed: {e}");
                    notices.push("Some settings could not be upgraded and were reset.".into());
                    needs_save = true;
                }
            }
            let (settings, warnings) = Settings::from_value_lenient(&value);
            if !warnings.is_empty() {
                needs_save = true;
            }
            notices.extend(warnings);
            settings
        }
    };

    let fixes = settings.sanitize();
    if !fixes.is_empty() {
        for f in &fixes {
            log::warn!("settings repaired: {f}");
        }
        notices.extend(fixes);
        needs_save = true;
    }
    if needs_save {
        if let Err(e) = save_settings(path, &settings) {
            log::error!("could not save repaired settings: {e}");
        }
    }
    LoadedSettings { settings, first_run: false, notices }
}

pub fn save_settings(path: &Path, settings: &Settings) -> std::io::Result<()> {
    json_store::write_json_atomic(path, settings)
}

/// Applies a partial JSON object (as sent by the UI) on top of `current`.
pub fn apply_patch(current: &Settings, patch: &Value) -> Result<Settings, String> {
    if !patch.is_object() {
        return Err("settings patch must be an object".into());
    }
    let mut merged = serde_json::to_value(current).map_err(|e| e.to_string())?;
    merge_json(&mut merged, patch);
    let mut next: Settings = serde_json::from_value(merged).map_err(|e| format!("invalid settings value: {e}"))?;
    next.sanitize();
    Ok(next)
}

fn merge_json(target: &mut Value, patch: &Value) {
    match (target, patch) {
        (Value::Object(t), Value::Object(p)) => {
            for (k, v) in p {
                match t.get_mut(k) {
                    Some(existing) if existing.is_object() && v.is_object() => merge_json(existing, v),
                    _ => {
                        t.insert(k.clone(), v.clone());
                    }
                }
            }
        }
        (t, p) => *t = p.clone(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn defaults_round_trip_through_json() {
        let settings = Settings::default();
        let json = serde_json::to_string(&settings).unwrap();
        let back: Settings = serde_json::from_str(&json).unwrap();
        assert_eq!(settings, back);
    }

    #[test]
    fn serializes_with_camel_case_keys() {
        let value = serde_json::to_value(Settings::default()).unwrap();
        assert_eq!(value["schemaVersion"], SCHEMA_VERSION);
        assert_eq!(value["capsLock"]["position"], "near");
        assert_eq!(value["shortcuts"]["bindings"]["spotlight.toggle"], "Alt+Space");
        assert_eq!(value["shortcuts"]["bindings"]["spotlight.toggleAlternate"], "Ctrl+Space");
    }

    #[test]
    fn missing_fields_take_defaults() {
        let (settings, warnings) = Settings::from_value_lenient(&json!({ "general": { "launchAtStartup": true } }));
        assert!(warnings.is_empty());
        assert!(settings.general.launch_at_startup);
        assert_eq!(settings.spotlight, SpotlightSettings::default());
    }

    #[test]
    fn invalid_section_is_reset_without_losing_others() {
        let (settings, warnings) = Settings::from_value_lenient(&json!({
            "general": { "launchAtStartup": true },
            "spotlight": { "maxResults": "lots" }
        }));
        assert_eq!(warnings.len(), 1);
        assert!(settings.general.launch_at_startup);
        assert_eq!(settings.spotlight.max_results, SpotlightSettings::default().max_results);
    }

    #[test]
    fn sanitize_clamps_ranges() {
        let mut s = Settings::default();
        s.spotlight.max_results = 1000;
        s.spotlight.search_delay_ms = 9000;
        s.caps_lock.size = 2;
        s.caps_lock.opacity = 5.0;
        s.caps_lock.duration_ms = 999_999;
        let fixes = s.sanitize();
        assert_eq!(s.spotlight.max_results, MAX_RESULTS_RANGE.1);
        assert_eq!(s.spotlight.search_delay_ms, SEARCH_DELAY_RANGE.1);
        assert_eq!(s.caps_lock.size, INDICATOR_SIZE_RANGE.0);
        assert_eq!(s.caps_lock.opacity, 1.0);
        assert_eq!(s.caps_lock.duration_ms, INDICATOR_DURATION_MAX);
        assert_eq!(fixes.len(), 5);
    }

    #[test]
    fn sanitize_repairs_invalid_and_duplicate_shortcuts() {
        let mut s = Settings::default();
        s.shortcuts.bindings.insert("spotlight.toggle".into(), Some("Ctrl+Space".into()));
        s.shortcuts.bindings.insert("spotlight.toggleAlternate".into(), Some("ctrl+space".into()));
        s.shortcuts.bindings.insert("settings.open".into(), Some("Banana".into()));
        s.shortcuts.bindings.insert("unknown.action".into(), Some("Alt+K".into()));
        let fixes = s.sanitize();
        assert_eq!(s.shortcuts.bindings["spotlight.toggle"].as_deref(), Some("Ctrl+Space"));
        assert_eq!(s.shortcuts.bindings["spotlight.toggleAlternate"], None);
        assert_eq!(s.shortcuts.bindings["settings.open"], None);
        assert!(!s.shortcuts.bindings.contains_key("unknown.action"));
        assert_eq!(fixes.len(), 3);
    }

    #[test]
    fn sanitize_normalizes_shortcut_spelling() {
        let mut s = Settings::default();
        s.shortcuts.bindings.insert("settings.open".into(), Some("shift+control+comma".into()));
        s.sanitize();
        assert_eq!(s.shortcuts.bindings["settings.open"].as_deref(), Some("Ctrl+Shift+Comma"));
    }

    #[test]
    fn patch_merges_nested_values() {
        let current = Settings::default();
        let next = apply_patch(&current, &json!({ "capsLock": { "size": 64 }, "appearance": { "theme": "dark" } })).unwrap();
        assert_eq!(next.caps_lock.size, 64);
        assert_eq!(next.caps_lock.opacity, current.caps_lock.opacity);
        assert_eq!(next.appearance.theme, AppearanceMode::Dark);
    }

    #[test]
    fn patch_rejects_bad_types() {
        assert!(apply_patch(&Settings::default(), &json!({ "capsLock": { "size": "big" } })).is_err());
        assert!(apply_patch(&Settings::default(), &json!(42)).is_err());
    }

    #[test]
    fn load_creates_defaults_on_first_run() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let loaded = load_settings(&path);
        assert!(loaded.first_run);
        assert!(path.exists());
        assert_eq!(loaded.settings, Settings::default());
    }

    #[test]
    fn load_persists_and_reads_back() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let mut s = Settings::default();
        s.appearance.accent = Accent::Green;
        s.caps_lock.position = IndicatorPosition::Above;
        save_settings(&path, &s).unwrap();
        let loaded = load_settings(&path);
        assert!(!loaded.first_run);
        assert!(loaded.notices.is_empty());
        assert_eq!(loaded.settings, s);
    }

    #[test]
    fn load_recovers_from_corrupt_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("settings.json");
        std::fs::write(&path, "{ this is not json").unwrap();
        let loaded = load_settings(&path);
        assert_eq!(loaded.settings, Settings::default());
        assert_eq!(loaded.notices.len(), 1);
        let backups: Vec<_> = std::fs::read_dir(dir.path())
            .unwrap()
            .filter_map(|e| e.ok())
            .filter(|e| e.file_name().to_string_lossy().contains("corrupt"))
            .collect();
        assert_eq!(backups.len(), 1);
        // a valid file was written back
        assert!(load_settings(&path).notices.is_empty());
    }
}
