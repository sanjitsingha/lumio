//! Input method cursor positions: Windows reads the system caret directly
//! (see `caret.rs`), so there is nothing to watch.

use crate::capslock::InputMethodEvent;

pub struct ImeWatch;

impl ImeWatch {
    pub fn start(_on_event: impl Fn(InputMethodEvent) + Send + 'static) -> Result<Self, String> {
        Err("not used on Windows".into())
    }

    pub fn is_alive(&self) -> bool {
        false
    }
}
