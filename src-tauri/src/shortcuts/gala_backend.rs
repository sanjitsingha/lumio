//! Global shortcuts through Gala's `GrabAccelerator` API. This is the only
//! working mechanism on elementary OS Wayland sessions, where X11 key grabs
//! do not see keys pressed in native Wayland windows.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use gtk::gio;

use super::accelerator::Accelerator;
use super::backend::{ActivationSink, BackendKind, RegisterError, ShortcutBackend};
use crate::platform::linux::{gala, keybindings};

pub struct GalaBackend {
    conn: gio::DBusConnection,
    grabs: Arc<Mutex<HashMap<Accelerator, u32>>>,
    _subscription: gio::SignalSubscriptionId,
}

impl GalaBackend {
    /// Must be called on the main thread: activation callbacks are delivered
    /// on the GLib main context that is current here.
    pub fn connect(sink: ActivationSink) -> Result<Self, String> {
        let conn = gala::session_bus()?;
        if !gala::gala_available(&conn) {
            return Err("the Gala compositor service is not available".into());
        }
        let grabs: Arc<Mutex<HashMap<Accelerator, u32>>> = Arc::default();
        let lookup = grabs.clone();
        let subscription = gala::on_accelerator_activated(&conn, move |action, timestamp| {
            let accel = lookup
                .lock()
                .unwrap_or_else(|e| e.into_inner())
                .iter()
                .find_map(|(accel, id)| (*id == action).then_some(*accel));
            if let Some(accel) = accel {
                sink(accel, timestamp);
            }
        });
        Ok(Self { conn, grabs, _subscription: subscription })
    }
}

impl ShortcutBackend for GalaBackend {
    fn kind(&self) -> BackendKind {
        BackendKind::Gala
    }

    fn label(&self) -> String {
        "elementary OS compositor (Gala)".into()
    }

    fn register(&self, accel: Accelerator) -> Result<(), RegisterError> {
        if self.grabs.lock().unwrap_or_else(|e| e.into_inner()).contains_key(&accel) {
            return Ok(());
        }
        let id = gala::grab_accelerator(&self.conn, &accel.to_gtk()).map_err(|e| {
            log::error!("GrabAccelerator({}) failed: {e}", accel.to_gtk());
            RegisterError::Failed(format!("The compositor could not register {accel}: {e}"))
        })?;
        if id == 0 {
            let owner = keybindings::find_system_owner(&accel);
            log::warn!("compositor refused {accel}; owner: {owner:?}");
            return Err(RegisterError::Conflict(match owner {
                Some(owner) => format!("{accel} is already used by the system shortcut {owner}."),
                None => format!("{accel} is already used by the system or another application."),
            }));
        }
        log::info!("grabbed {accel} via Gala (action {id})");
        self.grabs.lock().unwrap_or_else(|e| e.into_inner()).insert(accel, id);
        Ok(())
    }

    fn unregister(&self, accel: Accelerator) -> Result<(), RegisterError> {
        let Some(id) = self.grabs.lock().unwrap_or_else(|e| e.into_inner()).remove(&accel) else {
            return Ok(());
        };
        match gala::ungrab_accelerator(&self.conn, id) {
            Ok(true) => Ok(()),
            Ok(false) => {
                log::warn!("compositor reported {accel} (action {id}) was not grabbed by us");
                Ok(())
            }
            Err(e) => Err(RegisterError::Failed(e)),
        }
    }
}
