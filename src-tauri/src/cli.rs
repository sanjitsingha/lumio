//! Command-line flags. A second `lumio` process forwards its flags to the
//! running instance (single-instance plugin), so `lumio --toggle` can be
//! bound to a desktop keyboard shortcut where global shortcuts are not
//! available to apps.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Launch {
    /// Started from the login autostart entry.
    Autostart,
    /// Show or hide Spotlight.
    Toggle,
    Settings,
    Quit,
    /// Developer harness command (`--dev-*`); only meaningful for a running instance.
    Dev,
    /// Restarted by the updater (`--after-update <old pid>`).
    Updated,
    /// Started by the user without flags.
    Manual,
}

impl Launch {
    /// Commands addressed to an already-running instance. When no instance
    /// runs, the process exits instead of becoming one.
    pub fn requires_running_instance(self) -> bool {
        matches!(self, Launch::Quit | Launch::Dev)
    }
}

pub fn parse<S: AsRef<str>>(args: &[S]) -> Launch {
    let flags: Vec<&str> = args.iter().skip(1).map(|a| a.as_ref()).collect();
    if flags.iter().any(|f| f.starts_with("--dev-")) {
        Launch::Dev
    } else if flags.contains(&AFTER_UPDATE_FLAG) {
        Launch::Updated
    } else if flags.contains(&"--quit") {
        Launch::Quit
    } else if flags.iter().any(|f| *f == "--toggle" || *f == "--spotlight") {
        Launch::Toggle
    } else if flags.contains(&"--settings") {
        Launch::Settings
    } else if flags.contains(&"--autostart") || flags.contains(&"--minimized") {
        Launch::Autostart
    } else {
        Launch::Manual
    }
}

pub const AFTER_UPDATE_FLAG: &str = "--after-update";

/// The previous process id passed with `--after-update`.
pub fn after_update_pid<S: AsRef<str>>(args: &[S]) -> Option<u32> {
    let pos = args.iter().position(|a| a.as_ref() == AFTER_UPDATE_FLAG)?;
    args.get(pos + 1)?.as_ref().parse().ok()
}

pub const HELP: &str = "Usage: lumio [--toggle | --settings | --quit | --autostart]";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_flags() {
        assert_eq!(parse(&["lumio"]), Launch::Manual);
        assert_eq!(parse(&["lumio", "--toggle"]), Launch::Toggle);
        assert_eq!(parse(&["lumio", "--spotlight"]), Launch::Toggle);
        assert_eq!(parse(&["lumio", "--autostart"]), Launch::Autostart);
        assert_eq!(parse(&["lumio", "--settings"]), Launch::Settings);
        assert_eq!(parse(&["lumio", "--toggle", "--quit"]), Launch::Quit, "quit wins");
        assert_eq!(parse(&["lumio", "--unknown"]), Launch::Manual);
        assert_eq!(parse(&["lumio", "--dev-eval", "spotlight", "1"]), Launch::Dev);
        assert!(Launch::Dev.requires_running_instance() && Launch::Quit.requires_running_instance());
        assert!(!Launch::Toggle.requires_running_instance());
        assert_eq!(parse(&["lumio", "--after-update", "4242"]), Launch::Updated);
        assert_eq!(after_update_pid(&["lumio", "--after-update", "4242"]), Some(4242));
        assert_eq!(after_update_pid(&["lumio", "--after-update"]), None);
        assert_eq!(after_update_pid(&["lumio"]), None);
    }
}
