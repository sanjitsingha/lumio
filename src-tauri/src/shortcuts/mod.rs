//! Global shortcut management: validation, conflict detection, OS
//! registration through a pluggable backend, and per-binding status.

pub mod accelerator;
pub mod actions;
pub mod backend;
#[cfg(target_os = "linux")]
pub mod gala_backend;
pub mod plugin_backend;

use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use serde::Serialize;

use accelerator::Accelerator;
use actions::{ShortcutAction, SlotInfo, SLOTS};
use backend::{BackendKind, RegisterError, ShortcutBackend};

use crate::error::{AppError, AppResult};

/// Ignore activations closer together than this (key auto-repeat while held).
const REPEAT_GUARD: Duration = Duration::from_millis(280);

#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutErrorInfo {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SlotStatus {
    pub id: &'static str,
    pub action: ShortcutAction,
    pub label: &'static str,
    pub description: &'static str,
    pub accelerator: Option<String>,
    pub default_accelerator: Option<&'static str>,
    pub registered: bool,
    pub error: Option<ShortcutErrorInfo>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutReport {
    pub backend: BackendKind,
    pub backend_label: String,
    pub available: bool,
    pub unavailable_reason: Option<String>,
    pub recording: bool,
    pub slots: Vec<SlotStatus>,
}

#[derive(Default)]
struct State {
    /// Desired bindings (slot id -> accelerator), mirrors settings.
    bindings: BTreeMap<String, Option<String>>,
    errors: HashMap<&'static str, ShortcutErrorInfo>,
    suspended: bool,
}

pub struct ShortcutManager {
    backend: Option<Arc<dyn ShortcutBackend>>,
    unavailable_reason: Option<String>,
    state: Mutex<State>,
    /// Accelerators currently registered with the OS -> slot. Read by the
    /// activation path, which must never wait on backend calls.
    active: RwLock<HashMap<Accelerator, &'static str>>,
    /// Serializes operations that talk to the backend.
    op_lock: Mutex<()>,
    last_fire: Mutex<Option<Instant>>,
}

fn err(code: &'static str, message: impl Into<String>) -> AppError {
    AppError::Shortcut { code, message: message.into() }
}

fn register_error_info(e: RegisterError) -> ShortcutErrorInfo {
    match e {
        RegisterError::Conflict(message) => ShortcutErrorInfo { code: "conflict", message },
        RegisterError::Failed(message) => ShortcutErrorInfo { code: "registration-failed", message },
    }
}

impl ShortcutManager {
    pub fn new(backend: Option<Arc<dyn ShortcutBackend>>, unavailable_reason: Option<String>) -> Self {
        Self {
            backend,
            unavailable_reason,
            state: Mutex::default(),
            active: RwLock::default(),
            op_lock: Mutex::new(()),
            last_fire: Mutex::new(None),
        }
    }

    fn state(&self) -> std::sync::MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    /// Replaces all bindings (e.g. after loading or resetting settings).
    pub fn apply(&self, bindings: &BTreeMap<String, Option<String>>) {
        let _op = self.op_lock.lock().unwrap_or_else(|e| e.into_inner());
        self.state().bindings = bindings.clone();
        self.sync_locked();
    }

    /// Parses and validates a shortcut for `slot_id`, including duplicates
    /// against the other bindings. Returns the canonical text.
    pub fn validate(&self, slot_id: &str, text: &str) -> AppResult<Accelerator> {
        let slot = actions::slot(slot_id).ok_or_else(|| err("invalid-shortcut", format!("Unknown shortcut \"{slot_id}\".")))?;
        let accel: Accelerator = text.parse().map_err(|e: accelerator::ParseError| err("invalid-shortcut", e.to_string()))?;
        accel.validate().map_err(|e| err("invalid-shortcut", e.to_string()))?;
        let state = self.state();
        for other in SLOTS.iter().filter(|s| s.id != slot.id) {
            let taken = state
                .bindings
                .get(other.id)
                .and_then(|b| b.as_deref())
                .and_then(|b| b.parse::<Accelerator>().ok());
            if taken == Some(accel) {
                return Err(err("duplicate", format!("{accel} is already assigned to “{}”.", other.label)));
            }
        }
        Ok(accel)
    }

    /// Changes one binding. The new shortcut is registered with the OS before
    /// the old one is released, so a refusal leaves the previous binding intact.
    /// Returns the canonical text that should be persisted.
    pub fn set_binding(&self, slot_id: &str, text: Option<&str>) -> AppResult<Option<String>> {
        let slot = actions::slot(slot_id).ok_or_else(|| err("invalid-shortcut", format!("Unknown shortcut \"{slot_id}\".")))?;
        let new_accel = text.map(|t| self.validate(slot_id, t)).transpose()?;
        let _op = self.op_lock.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(accel) = new_accel {
            let already_active = self.active.read().unwrap_or_else(|e| e.into_inner()).get(&accel) == Some(&slot.id);
            if !already_active {
                let backend = self.backend.as_ref().ok_or_else(|| {
                    err("unavailable", self.unavailable_reason.clone().unwrap_or_else(|| "Global shortcuts are unavailable.".into()))
                })?;
                backend.register(accel).map_err(|e| {
                    let info = register_error_info(e);
                    err(info.code, info.message)
                })?;
                self.active.write().unwrap_or_else(|e| e.into_inner()).insert(accel, slot.id);
            }
        }
        {
            let mut state = self.state();
            state.bindings.insert(slot.id.to_string(), new_accel.map(|a| a.to_string()));
            state.errors.remove(slot.id);
            state.suspended = false;
        }
        self.sync_locked();
        Ok(new_accel.map(|a| a.to_string()))
    }

    /// Releases every shortcut while the user records a new one, so pressing
    /// an existing combination is captured instead of triggering it.
    pub fn suspend(&self) {
        let _op = self.op_lock.lock().unwrap_or_else(|e| e.into_inner());
        self.state().suspended = true;
        self.sync_locked();
    }

    pub fn resume(&self) {
        let _op = self.op_lock.lock().unwrap_or_else(|e| e.into_inner());
        self.state().suspended = false;
        self.sync_locked();
    }

    /// Unregisters everything (application exit).
    pub fn shutdown(&self) {
        let _op = self.op_lock.lock().unwrap_or_else(|e| e.into_inner());
        let active: Vec<Accelerator> = self.active.write().unwrap_or_else(|e| e.into_inner()).drain().map(|(a, _)| a).collect();
        if let Some(backend) = &self.backend {
            for accel in active {
                if let Err(e) = backend.unregister(accel) {
                    log::warn!("could not release {accel} on exit: {e:?}");
                }
            }
        }
    }

    /// Resolves an OS activation to the slot it belongs to, suppressing
    /// auto-repeat while the keys are held down.
    pub fn resolve_activation(&self, accel: Accelerator) -> Option<&'static SlotInfo> {
        let slot_id = *self.active.read().unwrap_or_else(|e| e.into_inner()).get(&accel)?;
        let now = Instant::now();
        let mut last = self.last_fire.lock().unwrap_or_else(|e| e.into_inner());
        let repeated = last.is_some_and(|t| now.duration_since(t) < REPEAT_GUARD);
        *last = Some(now);
        if repeated {
            return None;
        }
        actions::slot(slot_id)
    }

    pub fn report(&self) -> ShortcutReport {
        let state = self.state();
        let active = self.active.read().unwrap_or_else(|e| e.into_inner());
        let slots = SLOTS
            .iter()
            .map(|slot| {
                let accelerator = state.bindings.get(slot.id).cloned().flatten();
                let registered = accelerator
                    .as_deref()
                    .and_then(|a| a.parse::<Accelerator>().ok())
                    .is_some_and(|a| active.get(&a) == Some(&slot.id));
                SlotStatus {
                    id: slot.id,
                    action: slot.action,
                    label: slot.label,
                    description: slot.description,
                    accelerator,
                    default_accelerator: slot.default,
                    registered,
                    error: state.errors.get(slot.id).cloned(),
                }
            })
            .collect();
        ShortcutReport {
            backend: self.backend.as_ref().map(|b| b.kind()).unwrap_or(BackendKind::Unavailable),
            backend_label: self.backend.as_ref().map(|b| b.label()).unwrap_or_else(|| "Unavailable".into()),
            available: self.backend.is_some(),
            unavailable_reason: self.unavailable_reason.clone(),
            recording: state.suspended,
            slots,
        }
    }

    /// Reconciles OS registrations with the desired bindings. Caller holds `op_lock`.
    fn sync_locked(&self) {
        let (desired, suspended) = {
            let state = self.state();
            let desired: HashMap<Accelerator, &'static str> = SLOTS
                .iter()
                .filter_map(|slot| {
                    let text = state.bindings.get(slot.id)?.as_deref()?;
                    Some((text.parse::<Accelerator>().ok()?, slot.id))
                })
                .collect();
            (desired, state.suspended)
        };
        let target: HashMap<Accelerator, &'static str> = if suspended { HashMap::new() } else { desired.clone() };
        let current: HashMap<Accelerator, &'static str> = self.active.read().unwrap_or_else(|e| e.into_inner()).clone();

        let Some(backend) = &self.backend else {
            let mut state = self.state();
            state.errors.clear();
            if !suspended {
                let message = self.unavailable_reason.clone().unwrap_or_else(|| "Global shortcuts are unavailable.".into());
                for slot_id in desired.values() {
                    state.errors.insert(*slot_id, ShortcutErrorInfo { code: "unavailable", message: message.clone() });
                }
            }
            return;
        };

        for (accel, slot) in &current {
            if target.get(accel) != Some(slot) {
                if let Err(e) = backend.unregister(*accel) {
                    log::warn!("could not unregister {accel}: {e:?}");
                }
                self.active.write().unwrap_or_else(|e| e.into_inner()).remove(accel);
            }
        }
        let mut errors = HashMap::new();
        for (accel, slot) in &target {
            if current.get(accel) == Some(slot) {
                continue;
            }
            match backend.register(*accel) {
                Ok(()) => {
                    self.active.write().unwrap_or_else(|e| e.into_inner()).insert(*accel, slot);
                }
                Err(e) => {
                    let info = register_error_info(e);
                    log::warn!("shortcut {accel} for {slot} not registered: {}", info.message);
                    errors.insert(*slot, info);
                }
            }
        }
        let mut state = self.state();
        // keep errors for slots that are still failing, drop errors for slots that recovered
        if !suspended {
            state.errors = errors;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    /// Simulates an OS that refuses some combinations.
    #[derive(Default)]
    struct FakeBackend {
        taken: HashSet<String>,
        registered: Mutex<HashSet<Accelerator>>,
        calls: Mutex<Vec<String>>,
    }

    impl ShortcutBackend for FakeBackend {
        fn kind(&self) -> BackendKind {
            BackendKind::System
        }
        fn label(&self) -> String {
            "fake".into()
        }
        fn register(&self, accel: Accelerator) -> Result<(), RegisterError> {
            self.calls.lock().unwrap().push(format!("+{accel}"));
            if self.taken.contains(&accel.to_string()) {
                return Err(RegisterError::Conflict(format!("{accel} is taken")));
            }
            self.registered.lock().unwrap().insert(accel);
            Ok(())
        }
        fn unregister(&self, accel: Accelerator) -> Result<(), RegisterError> {
            self.calls.lock().unwrap().push(format!("-{accel}"));
            self.registered.lock().unwrap().remove(&accel);
            Ok(())
        }
    }

    fn manager(taken: &[&str]) -> (ShortcutManager, Arc<FakeBackend>) {
        let backend = Arc::new(FakeBackend { taken: taken.iter().map(|s| s.to_string()).collect(), ..Default::default() });
        (ShortcutManager::new(Some(backend.clone()), None), backend)
    }

    fn defaults() -> BTreeMap<String, Option<String>> {
        crate::storage::settings::ShortcutSettings::default().bindings
    }

    fn acc(s: &str) -> Accelerator {
        s.parse().unwrap()
    }

    #[test]
    fn applies_default_bindings() {
        let (m, backend) = manager(&[]);
        m.apply(&defaults());
        let reg = backend.registered.lock().unwrap().clone();
        assert!(reg.contains(&acc("Alt+Space")) && reg.contains(&acc("Ctrl+Space")));
        let report = m.report();
        assert!(report.slots.iter().filter(|s| s.accelerator.is_some()).all(|s| s.registered));
    }

    #[test]
    fn os_conflicts_are_reported_per_slot() {
        let (m, _) = manager(&["Alt+Space"]);
        m.apply(&defaults());
        let report = m.report();
        let primary = report.slots.iter().find(|s| s.id == "spotlight.toggle").unwrap();
        assert!(!primary.registered);
        assert_eq!(primary.error.as_ref().unwrap().code, "conflict");
        let alt = report.slots.iter().find(|s| s.id == "spotlight.toggleAlternate").unwrap();
        assert!(alt.registered && alt.error.is_none());
    }

    #[test]
    fn set_binding_registers_new_before_releasing_old() {
        let (m, backend) = manager(&[]);
        m.apply(&defaults());
        backend.calls.lock().unwrap().clear();
        let saved = m.set_binding("spotlight.toggle", Some("ctrl+alt+k")).unwrap();
        assert_eq!(saved.as_deref(), Some("Ctrl+Alt+K"));
        let calls = backend.calls.lock().unwrap().clone();
        assert_eq!(calls, vec!["+Ctrl+Alt+K", "-Alt+Space"]);
    }

    #[test]
    fn refused_binding_keeps_previous_shortcut() {
        let (m, backend) = manager(&["Ctrl+Alt+T"]);
        m.apply(&defaults());
        let e = m.set_binding("spotlight.toggle", Some("Ctrl+Alt+T")).unwrap_err();
        assert_eq!(e.code(), "conflict");
        assert!(backend.registered.lock().unwrap().contains(&acc("Alt+Space")));
        assert_eq!(m.report().slots[0].accelerator.as_deref(), Some("Alt+Space"));
    }

    #[test]
    fn rejects_duplicates_and_invalid_shortcuts() {
        let (m, _) = manager(&[]);
        m.apply(&defaults());
        assert_eq!(m.set_binding("settings.open", Some("Ctrl+Space")).unwrap_err().code(), "duplicate");
        assert_eq!(m.set_binding("settings.open", Some("Q")).unwrap_err().code(), "invalid-shortcut");
        assert_eq!(m.set_binding("settings.open", Some("Ctrl+Nope")).unwrap_err().code(), "invalid-shortcut");
        assert_eq!(m.set_binding("no.such.slot", Some("Ctrl+K")).unwrap_err().code(), "invalid-shortcut");
    }

    #[test]
    fn clearing_a_binding_unregisters_it() {
        let (m, backend) = manager(&[]);
        m.apply(&defaults());
        assert_eq!(m.set_binding("spotlight.toggleAlternate", None).unwrap(), None);
        assert!(!backend.registered.lock().unwrap().contains(&acc("Ctrl+Space")));
    }

    #[test]
    fn suspend_and_resume_for_recording() {
        let (m, backend) = manager(&[]);
        m.apply(&defaults());
        m.suspend();
        assert!(backend.registered.lock().unwrap().is_empty());
        assert!(m.report().recording);
        assert!(m.resolve_activation(acc("Alt+Space")).is_none());
        m.resume();
        assert_eq!(backend.registered.lock().unwrap().len(), 2);
    }

    #[test]
    fn activation_resolves_slot_and_ignores_auto_repeat() {
        let (m, _) = manager(&[]);
        m.apply(&defaults());
        assert_eq!(m.resolve_activation(acc("Ctrl+Space")).unwrap().id, "spotlight.toggleAlternate");
        assert!(m.resolve_activation(acc("Ctrl+Space")).is_none(), "repeat within guard window");
        assert!(m.resolve_activation(acc("Ctrl+Shift+Q")).is_none(), "not bound");
    }

    #[test]
    fn shutdown_releases_everything() {
        let (m, backend) = manager(&[]);
        m.apply(&defaults());
        m.shutdown();
        assert!(backend.registered.lock().unwrap().is_empty());
    }

    #[test]
    fn without_backend_every_binding_reports_unavailable() {
        let m = ShortcutManager::new(None, Some("Wayland does not allow global shortcuts here.".into()));
        m.apply(&defaults());
        let report = m.report();
        assert!(!report.available);
        assert!(report.slots.iter().filter(|s| s.accelerator.is_some()).all(|s| s.error.as_ref().unwrap().code == "unavailable"));
        assert_eq!(m.set_binding("settings.open", Some("Ctrl+Alt+K")).unwrap_err().code(), "unavailable");
    }
}
