//! Decides when the indicator is shown or hidden. Pure logic, no I/O.

use std::time::{Duration, Instant};

/// Upper bound for how long the "off" state flashes.
const OFF_FLASH: Duration = Duration::from_millis(900);

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Behavior {
    pub enabled: bool,
    /// `None` = keep showing while Caps Lock is on.
    pub duration: Option<Duration>,
    pub show_when_disabled: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    Show { on: bool },
    Hide,
    None,
}

#[derive(Debug, Default)]
pub struct IndicatorState {
    known: Option<bool>,
    visible: Option<bool>,
    hide_at: Option<Instant>,
}

impl IndicatorState {
    #[cfg(test)]
    pub fn caps_lock(&self) -> Option<bool> {
        self.known
    }

    #[cfg(test)]
    pub fn is_visible(&self) -> bool {
        self.visible.is_some()
    }

    pub fn deadline(&self) -> Option<Instant> {
        self.hide_at
    }

    /// Feeds a Caps Lock reading from the platform monitor.
    pub fn on_caps(&mut self, on: bool, now: Instant, b: &Behavior) -> Command {
        let previous = self.known.replace(on);
        let Some(previous) = previous else {
            // initial reading: only persistent mode shows an already-active Caps Lock
            if on && b.enabled && b.duration.is_none() {
                return self.show(true, None);
            }
            return Command::None;
        };
        if previous == on {
            return Command::None;
        }
        if !b.enabled {
            return self.hide();
        }
        if on {
            self.show(true, b.duration.map(|d| now + d))
        } else if b.show_when_disabled {
            let flash = b.duration.map_or(OFF_FLASH, |d| d.min(OFF_FLASH));
            self.show(false, Some(now + flash))
        } else {
            self.hide()
        }
    }

    /// Call when the deadline passes.
    pub fn on_tick(&mut self, now: Instant) -> Command {
        match self.hide_at {
            Some(t) if now >= t => self.hide(),
            _ => Command::None,
        }
    }

    /// Settings changed: hide if disabled; enter/leave persistent mode.
    pub fn on_settings(&mut self, now: Instant, b: &Behavior) -> Command {
        if !b.enabled {
            return self.hide();
        }
        match (self.known, b.duration) {
            (Some(true), None) => self.show(true, None),
            (_, Some(d)) if self.visible.is_some() && self.hide_at.is_none() => {
                self.hide_at = Some(now + d);
                Command::None
            }
            _ => Command::None,
        }
    }

    /// Shows a sample of the indicator (Settings "Preview" button).
    pub fn preview(&mut self, on: bool, now: Instant) -> Command {
        self.show(on, Some(now + Duration::from_millis(1800)))
    }

    fn show(&mut self, on: bool, hide_at: Option<Instant>) -> Command {
        self.visible = Some(on);
        self.hide_at = hide_at;
        Command::Show { on }
    }

    fn hide(&mut self) -> Command {
        self.hide_at = None;
        if self.visible.take().is_some() {
            Command::Hide
        } else {
            Command::None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn timed(ms: u64) -> Behavior {
        Behavior { enabled: true, duration: Some(Duration::from_millis(ms)), show_when_disabled: true }
    }

    #[test]
    fn initial_reading_does_not_flash() {
        let mut s = IndicatorState::default();
        assert_eq!(s.on_caps(true, Instant::now(), &timed(1500)), Command::None);
        assert_eq!(s.caps_lock(), Some(true));
    }

    #[test]
    fn turning_on_shows_then_hides_after_duration() {
        let t0 = Instant::now();
        let mut s = IndicatorState::default();
        s.on_caps(false, t0, &timed(1500));
        assert_eq!(s.on_caps(true, t0, &timed(1500)), Command::Show { on: true });
        assert_eq!(s.on_tick(t0 + Duration::from_millis(1000)), Command::None);
        assert_eq!(s.on_tick(t0 + Duration::from_millis(1500)), Command::Hide);
        assert!(!s.is_visible());
    }

    #[test]
    fn short_durations_hide_on_time() {
        let t0 = Instant::now();
        let mut s = IndicatorState::default();
        s.on_caps(false, t0, &timed(200));
        assert_eq!(s.on_caps(true, t0, &timed(200)), Command::Show { on: true });
        assert_eq!(s.on_tick(t0 + Duration::from_millis(199)), Command::None);
        assert_eq!(s.on_tick(t0 + Duration::from_millis(200)), Command::Hide);
        // the off flash never outlasts the chosen duration
        s.on_caps(true, t0, &timed(300));
        assert_eq!(s.on_caps(false, t0, &timed(300)), Command::Show { on: false });
        assert_eq!(s.deadline(), Some(t0 + Duration::from_millis(300)));
    }

    #[test]
    fn turning_off_flashes_off_state_briefly() {
        let t0 = Instant::now();
        let mut s = IndicatorState::default();
        s.on_caps(false, t0, &timed(3000));
        s.on_caps(true, t0, &timed(3000));
        assert_eq!(s.on_caps(false, t0, &timed(3000)), Command::Show { on: false });
        assert_eq!(s.deadline(), Some(t0 + OFF_FLASH));
    }

    #[test]
    fn turning_off_hides_immediately_when_flash_disabled() {
        let t0 = Instant::now();
        let b = Behavior { show_when_disabled: false, ..timed(1500) };
        let mut s = IndicatorState::default();
        s.on_caps(false, t0, &b);
        s.on_caps(true, t0, &b);
        assert_eq!(s.on_caps(false, t0, &b), Command::Hide);
    }

    #[test]
    fn duplicate_readings_are_ignored() {
        let t0 = Instant::now();
        let mut s = IndicatorState::default();
        s.on_caps(false, t0, &timed(1500));
        s.on_caps(true, t0, &timed(1500));
        assert_eq!(s.on_caps(true, t0, &timed(1500)), Command::None);
    }

    #[test]
    fn persistent_mode_stays_while_on() {
        let t0 = Instant::now();
        let b = Behavior { duration: None, ..timed(0) };
        let mut s = IndicatorState::default();
        assert_eq!(s.on_caps(true, t0, &b), Command::Show { on: true }, "already on at startup");
        assert_eq!(s.deadline(), None);
        assert_eq!(s.on_tick(t0 + Duration::from_secs(3600)), Command::None);
        assert_eq!(s.on_caps(false, t0, &b), Command::Show { on: false });
        assert_eq!(s.on_tick(t0 + OFF_FLASH), Command::Hide);
    }

    #[test]
    fn disabled_indicator_never_shows_and_hides_on_disable() {
        let t0 = Instant::now();
        let off = Behavior { enabled: false, ..timed(1500) };
        let mut s = IndicatorState::default();
        s.on_caps(false, t0, &off);
        assert_eq!(s.on_caps(true, t0, &off), Command::None);
        let mut s = IndicatorState::default();
        s.on_caps(false, t0, &timed(1500));
        s.on_caps(true, t0, &timed(1500));
        assert_eq!(s.on_settings(t0, &off), Command::Hide);
    }

    #[test]
    fn switching_to_persistent_mode_shows_active_caps_lock() {
        let t0 = Instant::now();
        let mut s = IndicatorState::default();
        s.on_caps(false, t0, &timed(1500));
        s.on_caps(true, t0, &timed(1500));
        s.on_tick(t0 + Duration::from_secs(2));
        let persistent = Behavior { duration: None, ..timed(0) };
        assert_eq!(s.on_settings(t0, &persistent), Command::Show { on: true });
    }

    #[test]
    fn preview_shows_requested_state_temporarily() {
        let t0 = Instant::now();
        let mut s = IndicatorState::default();
        assert_eq!(s.preview(true, t0), Command::Show { on: true });
        assert_eq!(s.on_tick(t0 + Duration::from_secs(2)), Command::Hide);
    }
}
