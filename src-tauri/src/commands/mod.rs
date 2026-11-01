//! Frontend-callable commands. Each window may only call the commands its
//! capability file grants (see `capabilities/`).

pub mod capslock;
pub mod search;
pub mod settings;
pub mod shortcuts;
pub mod system;
pub mod updates;
pub mod window;
