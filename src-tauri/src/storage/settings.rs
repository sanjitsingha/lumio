use std::collections::{BTreeMap, HashSet};
use std::path::Path;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::json_store;
use super::migrate::{self, MigrationOutcome};
use crate::search::fuzzy::fold;
use crate::shortcuts::accelerator::Accelerator;
use crate::shortcuts::actions::{self, ShortcutAction, SlotInfo, SLOTS};

/// Current settings schema. Bump together with a new entry in `migrate::MIGRATIONS`.
pub const SCHEMA_VERSION: u32 = 2;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    pub schema_version: u32,
    pub general: GeneralSettings,
    pub spotlight: SpotlightSettings,
    pub shortcuts: ShortcutSettings,
    pub commands: CommandSettings,
    pub caps_lock: CapsLockSettings,
    pub appearance: AppearanceSettings,
    pub notch: NotchSettings,
    /// Sections this version does not know, written by a newer or a test
    /// build: kept as they are, so going back and forth between versions
    /// loses nothing.
    #[serde(flatten)]
    pub unknown: serde_json::Map<String, Value>,
}

/// The sections `Settings` reads; any other top-level key is kept in `unknown`.
const SECTIONS: &[&str] = &["schemaVersion", "general", "spotlight", "shortcuts", "commands", "capsLock", "appearance", "notch"];

impl Default for Settings {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            general: GeneralSettings::default(),
            spotlight: SpotlightSettings::default(),
            shortcuts: ShortcutSettings::default(),
            commands: CommandSettings::default(),
            caps_lock: CapsLockSettings::default(),
            appearance: AppearanceSettings::default(),
            notch: NotchSettings::default(),
            unknown: serde_json::Map::new(),
        }
    }
}

/// Lummo always runs in the background (started at login it stays hidden,
/// closing Settings keeps it running) and checks for updates: these are not
/// settings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct GeneralSettings {
    pub launch_at_startup: bool,
    /// Show the tray icon, where the desktop has a tray.
    pub show_in_tray: bool,
    /// Download and install found updates without asking, then restart Lummo.
    pub auto_install_updates: bool,
    /// Send anonymous usage counts (see `analytics`).
    pub share_usage_statistics: bool,
}

impl Default for GeneralSettings {
    fn default() -> Self {
        Self { launch_at_startup: false, show_in_tray: true, auto_install_updates: false, share_usage_statistics: true }
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
    /// Snapping, moving and centering the window in use, found by "window" (Linux).
    pub search_windows: bool,
    /// Pages of the system's settings app ("Open dark mode settings").
    pub search_settings: bool,
    pub keep_history: bool,
    pub max_results: u32,
    pub search_delay_ms: u32,
    /// Folder names (e.g. `node_modules`) or absolute paths skipped by the file index.
    pub exclusions: Vec<String>,
    /// Folders indexed for file search. Empty means the user's home folder.
    pub index_roots: Vec<String>,
    pub index_hidden: bool,
    /// Offer "Search Google for …" below the results.
    pub web_search: bool,
    /// Browser for web searches: an id from `get_browsers`, or empty for the system default.
    pub web_browser: String,
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
            search_windows: true,
            search_settings: true,
            keep_history: true,
            max_results: 24,
            search_delay_ms: 60,
            exclusions: default_exclusions(),
            index_roots: Vec::new(),
            index_hidden: false,
            web_search: true,
            web_browser: String::new(),
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

/// The commands of the Shortcut page: quick actions and pages of the system's
/// settings app, by their result id (`action:…`, `setting:…`).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CommandSettings {
    /// Commands left out of Spotlight; their hotkeys are released too.
    pub disabled: Vec<String>,
    /// Command -> the word that finds it first in Spotlight.
    pub aliases: BTreeMap<String, String>,
}

pub const MAX_ALIAS_CHARS: usize = 24;

impl CommandSettings {
    fn sanitize(&mut self) {
        let mut seen = HashSet::new();
        self.disabled.retain(|id| actions::is_command(id) && seen.insert(id.clone()));
        let mut taken = HashSet::new();
        self.aliases = std::mem::take(&mut self.aliases)
            .into_iter()
            .filter_map(|(id, alias)| {
                let alias: String = fold(alias.trim()).chars().take(MAX_ALIAS_CHARS).collect();
                let alias = alias.trim_end().to_string();
                (actions::is_command(&id) && !alias.is_empty() && taken.insert(alias.clone())).then_some((id, alias))
            })
            .collect();
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

/// The indicator always sits at the text cursor (where there is none, it is
/// not shown) and takes its light or dark look from the app's appearance.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct CapsLockSettings {
    pub enabled: bool,
    pub position: IndicatorPosition,
    /// Edge length of the indicator in logical pixels.
    pub size: u32,
    pub opacity: f32,
    /// Corner radius in percent of the edge length (50 = a circle).
    pub corner_radius: u32,
    /// How long the indicator stays visible. `0` = while Caps Lock is on.
    pub duration_ms: u32,
    /// Stay until you type, as on a Mac (`duration_ms` is then unused).
    pub until_typing: bool,
    pub animation: AnimationSpeed,
    /// Show the indicator when a text field gets focus while Caps Lock is on.
    pub show_on_focus: bool,
    pub follow_cursor: bool,
}

pub const INDICATOR_SIZE_RANGE: (u32, u32) = (24, 96);
pub const INDICATOR_RADIUS_MAX: u32 = 50;
pub const INDICATOR_OPACITY_RANGE: (f32, f32) = (0.3, 1.0);
pub const INDICATOR_DURATION_MAX: u32 = 10_000;

impl Default for CapsLockSettings {
    fn default() -> Self {
        Self {
            enabled: true,
            position: IndicatorPosition::Near,
            size: 40,
            opacity: 0.96,
            corner_radius: 27,
            duration_ms: 1500,
            until_typing: false,
            animation: AnimationSpeed::Normal,
            show_on_focus: true,
            follow_cursor: true,
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

/// How large the text of Lummo's windows is (`TEXT_SCALE` in `useAppearance.js`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum TextSize {
    Small,
    Medium,
    Large,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct AppearanceSettings {
    pub theme: AppearanceMode,
    pub accent: Accent,
    pub motion: MotionPreference,
    pub text_size: TextSize,
}

impl Default for AppearanceSettings {
    fn default() -> Self {
        Self { theme: AppearanceMode::Dark, accent: Accent::Blue, motion: MotionPreference::System, text_size: TextSize::Medium }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RetentionUnit {
    Hours,
    Days,
}

/// The notch at the top of the screen (Windows only): its clipboard
/// history keeps each copy for `keep_for` hours or days after it was last
/// copied, then lets it go.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct NotchSettings {
    pub keep_for: u32,
    pub keep_unit: RetentionUnit,
}

impl Default for NotchSettings {
    fn default() -> Self {
        Self { keep_for: 1, keep_unit: RetentionUnit::Days }
    }
}

impl NotchSettings {
    /// The longest `keep_for` allowed in each unit: 30 days in hours, a year in days.
    pub fn keep_for_max(unit: RetentionUnit) -> u32 {
        match unit {
            RetentionUnit::Hours => 720,
            RetentionUnit::Days => 365,
        }
    }

    #[cfg_attr(not(windows), allow(dead_code))] // only the notch reads it, which is Windows only
    pub fn keep_duration(&self) -> std::time::Duration {
        let hours = match self.keep_unit {
            RetentionUnit::Hours => u64::from(self.keep_for),
            RetentionUnit::Days => u64::from(self.keep_for) * 24,
        };
        std::time::Duration::from_secs(hours * 3600)
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
        settings.commands = section(obj, "commands", &mut warnings);
        settings.caps_lock = section(obj, "capsLock", &mut warnings);
        settings.appearance = section(obj, "appearance", &mut warnings);
        settings.notch = section(obj, "notch", &mut warnings);
        settings.unknown =
            obj.iter().filter(|(key, _)| !SECTIONS.contains(&key.as_str())).map(|(k, v)| (k.clone(), v.clone())).collect();
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
        if c.corner_radius > INDICATOR_RADIUS_MAX {
            c.corner_radius = INDICATOR_RADIUS_MAX;
            fixes.push("Indicator corner radius adjusted.".into());
        }

        let n = &mut self.notch;
        let keep_for = n.keep_for.clamp(1, NotchSettings::keep_for_max(n.keep_unit));
        if keep_for != n.keep_for {
            fixes.push("How long the notch keeps copies was adjusted.".into());
            n.keep_for = keep_for;
        }

        // Lummo is dark only; a light or system value saved earlier is not worth a notice
        self.appearance.theme = AppearanceMode::Dark;

        self.commands.sanitize();
        fixes.extend(self.shortcuts.sanitize());
        self.schema_version = SCHEMA_VERSION;
        fixes
    }

    /// Whether a command can run: Spotlight and its group are on, and so is the command.
    pub fn command_is_on(&self, id: &str) -> bool {
        let group = if id.starts_with("action:") { self.spotlight.search_actions } else { self.spotlight.search_settings };
        self.spotlight.enabled && group && !self.commands.disabled.iter().any(|d| d == id)
    }
}

impl ShortcutSettings {
    /// The other shortcut or command this combination is saved for, even if
    /// that one is turned off for now.
    pub fn assigned_to(&self, accel: Accelerator, except: &str) -> Option<&'static SlotInfo> {
        actions::all().filter(|s| s.id != except).find(|s| {
            self.bindings.get(s.id).and_then(|b| b.as_deref()).and_then(|b| b.parse::<Accelerator>().ok()) == Some(accel)
        })
    }

    pub fn sanitize(&mut self) -> Vec<String> {
        let mut fixes = Vec::new();
        self.bindings.retain(|k, v| {
            let Some(slot) = actions::slot(k) else {
                // shortcuts that no longer exist go silently unless one was assigned
                if v.is_some() {
                    fixes.push(format!("Removed unknown shortcut \"{k}\"."));
                }
                return false;
            };
            // a command is only saved while it has a hotkey
            slot.action != ShortcutAction::RunCommand || v.is_some()
        });
        let mut used: HashSet<Accelerator> = HashSet::new();
        for slot in actions::all() {
            let entry = if slot.action == ShortcutAction::RunCommand {
                let Some(entry) = self.bindings.get_mut(slot.id) else { continue };
                entry
            } else {
                self.bindings.entry(slot.id.to_string()).or_insert_with(|| slot.default.map(str::to_string))
            };
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
    fn text_size_is_medium_unless_chosen() {
        assert_eq!(Settings::default().appearance.text_size, TextSize::Medium);
        // saved before this setting existed
        let saved: AppearanceSettings = serde_json::from_value(json!({ "motion": "full" })).unwrap();
        assert_eq!((saved.text_size, saved.motion), (TextSize::Medium, MotionPreference::Full));
        let next = apply_patch(&Settings::default(), &json!({ "appearance": { "textSize": "large" } })).unwrap();
        assert_eq!(next.appearance.text_size, TextSize::Large);
    }

    #[test]
    fn commands_keep_tidy_aliases_and_hotkeys() {
        let next = apply_patch(
            &Settings::default(),
            &json!({ "commands": {
                "disabled": ["setting:wifi", "setting:wifi", "setting:gone"],
                "aliases": { "action:calculator": "  CALC ", "setting:wifi": "calc", "setting:gone": "x", "setting:vpn": "" }
            } }),
        )
        .unwrap();
        assert_eq!(next.commands.disabled, ["setting:wifi"]);
        assert_eq!(next.commands.aliases.len(), 1, "{:?}", next.commands.aliases);
        assert_eq!(next.commands.aliases["action:calculator"], "calc");

        // clearing an alias sends an empty one
        let cleared = apply_patch(&next, &json!({ "commands": { "aliases": { "action:calculator": "" } } })).unwrap();
        assert!(cleared.commands.aliases.is_empty());

        let mut s = Settings::default();
        s.shortcuts.bindings.insert("action:calculator".into(), Some("ctrl+alt+c".into()));
        s.shortcuts.bindings.insert("setting:wifi".into(), None);
        assert!(s.sanitize().is_empty());
        assert_eq!(s.shortcuts.bindings["action:calculator"].as_deref(), Some("Ctrl+Alt+C"));
        assert!(!s.shortcuts.bindings.contains_key("setting:wifi"), "only commands with a hotkey are saved");
        let accel = "Ctrl+Alt+C".parse().unwrap();
        assert_eq!(s.shortcuts.assigned_to(accel, "setting:wifi").unwrap().id, "action:calculator");
        assert!(s.shortcuts.assigned_to(accel, "action:calculator").is_none());
    }

    #[test]
    fn a_command_is_on_only_with_spotlight_and_its_group() {
        let mut s = Settings::default();
        assert!(s.command_is_on("action:calculator") && s.command_is_on("setting:wifi"));
        s.commands.disabled = vec!["setting:wifi".into()];
        assert!(!s.command_is_on("setting:wifi") && s.command_is_on("setting:vpn"));
        s.spotlight.search_settings = false;
        assert!(!s.command_is_on("setting:vpn") && s.command_is_on("action:calculator"));
        s.spotlight.enabled = false;
        assert!(!s.command_is_on("action:calculator"));
    }

    #[test]
    fn the_tray_icon_is_on_unless_turned_off() {
        assert!(Settings::default().general.show_in_tray);
        // saved before this setting existed
        let saved: GeneralSettings = serde_json::from_value(json!({ "launchAtStartup": true })).unwrap();
        assert!(saved.show_in_tray);
        let next = apply_patch(&Settings::default(), &json!({ "general": { "showInTray": false } })).unwrap();
        assert!(!next.general.show_in_tray);
    }

    #[test]
    fn notch_keeps_copies_for_a_day_and_clamps_to_its_unit() {
        let value = serde_json::to_value(Settings::default()).unwrap();
        assert_eq!(value["notch"], json!({ "keepFor": 1, "keepUnit": "days" }));
        assert_eq!(Settings::default().notch.keep_duration(), std::time::Duration::from_secs(24 * 3600));

        let next = apply_patch(&Settings::default(), &json!({ "notch": { "keepFor": 900, "keepUnit": "hours" } })).unwrap();
        assert_eq!(next.notch.keep_for, 720);
        assert_eq!(next.notch.keep_duration(), std::time::Duration::from_secs(720 * 3600));
        let next = apply_patch(&Settings::default(), &json!({ "notch": { "keepFor": 0 } })).unwrap();
        assert_eq!(next.notch.keep_for, 1);
    }

    #[test]
    fn serializes_with_camel_case_keys() {
        let value = serde_json::to_value(Settings::default()).unwrap();
        assert_eq!(value["schemaVersion"], SCHEMA_VERSION);
        assert_eq!(value["capsLock"]["position"], "near");
        assert_eq!(value["capsLock"]["cornerRadius"], 27);
        let shortcut = if cfg!(windows) { "Alt+Space" } else { "Ctrl+Space" };
        assert_eq!(value["shortcuts"]["bindings"]["spotlight.toggle"], shortcut);
        assert_eq!(value["shortcuts"]["bindings"].as_object().unwrap().len(), 1);
    }

    #[test]
    fn missing_fields_take_defaults() {
        let (settings, warnings) = Settings::from_value_lenient(&json!({ "general": { "launchAtStartup": true } }));
        assert!(warnings.is_empty());
        assert!(settings.general.launch_at_startup);
        assert!(settings.general.share_usage_statistics);
        assert_eq!(settings.spotlight, SpotlightSettings::default());
    }

    #[test]
    fn unknown_sections_survive_loading_and_saving() {
        let value = json!({ "general": { "launchAtStartup": false }, "futureFeature": { "on": true, "level": 3 } });
        let (settings, warnings) = Settings::from_value_lenient(&value);
        assert!(warnings.is_empty(), "{warnings:?}");
        let saved = serde_json::to_value(&settings).unwrap();
        assert_eq!(saved["futureFeature"], json!({ "on": true, "level": 3 }));
        assert_eq!(saved["general"]["launchAtStartup"], false);
        let (again, _) = Settings::from_value_lenient(&saved);
        assert_eq!(again, settings);
    }

    #[test]
    fn removed_indicator_options_are_ignored() {
        // options of 0.1.x that no longer exist: the rest of the section is kept
        let (settings, warnings) = Settings::from_value_lenient(&json!({ "capsLock": {
            "position": "below", "followCursor": false, "anchor": "pointer", "caretFallback": "osd",
            "appearance": "dark", "waylandFallback": "none"
        } }));
        assert!(warnings.is_empty());
        assert!(settings.caps_lock.show_on_focus);
        assert_eq!(settings.caps_lock.position, IndicatorPosition::Below);
        assert!(!settings.caps_lock.follow_cursor);
        assert_eq!(settings.caps_lock.corner_radius, 27);
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
    fn sanitize_repairs_invalid_shortcuts() {
        let mut s = Settings::default();
        s.shortcuts.bindings.insert("spotlight.toggle".into(), Some("Banana".into()));
        s.shortcuts.bindings.insert("unknown.action".into(), Some("Alt+K".into()));
        // a removed shortcut that was never assigned goes silently
        s.shortcuts.bindings.insert("settings.open".into(), None);
        let fixes = s.sanitize();
        let default = crate::shortcuts::actions::SLOTS[0].default;
        assert_eq!(s.shortcuts.bindings["spotlight.toggle"].as_deref(), default);
        assert!(!s.shortcuts.bindings.contains_key("unknown.action"));
        assert!(!s.shortcuts.bindings.contains_key("settings.open"));
        assert_eq!(fixes.len(), 2, "{fixes:?}");
    }

    #[test]
    fn sanitize_normalizes_shortcut_spelling() {
        let mut s = Settings::default();
        s.shortcuts.bindings.insert("spotlight.toggle".into(), Some("shift+control+comma".into()));
        s.sanitize();
        assert_eq!(s.shortcuts.bindings["spotlight.toggle"].as_deref(), Some("Ctrl+Shift+Comma"));
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
    fn sanitize_keeps_the_app_dark() {
        let mut s = Settings::default();
        s.appearance.theme = AppearanceMode::Light;
        assert!(s.sanitize().is_empty(), "no notice for this");
        assert_eq!(s.appearance.theme, AppearanceMode::Dark);
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
