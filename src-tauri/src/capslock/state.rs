//! Decides when the indicator is shown or hidden. Pure logic, no I/O.

use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Behavior {
    pub enabled: bool,
    /// `None` = keep showing while Caps Lock is on.
    pub duration: Option<Duration>,
    /// Show the indicator when a text field gets focus with Caps Lock on.
    pub show_on_focus: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Command {
    /// The indicator only ever shows Caps Lock on: as on a Mac, turning it
    /// off just hides it.
    Show,
    Hide,
    None,
}

#[derive(Debug, Default)]
pub struct IndicatorState {
    known: Option<bool>,
    visible: bool,
    hide_at: Option<Instant>,
}

impl IndicatorState {
    pub fn caps_lock(&self) -> Option<bool> {
        self.known
    }

    #[cfg(test)]
    pub fn is_visible(&self) -> bool {
        self.visible
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
                return self.show(None);
            }
            return Command::None;
        };
        if previous == on {
            return Command::None;
        }
        if on && b.enabled {
            self.show(b.duration.map(|d| now + d))
        } else {
            self.hide()
        }
    }

    /// A text field got keyboard focus: with Caps Lock already on, show the
    /// indicator there as if it had just been turned on, so typing never
    /// starts in capitals unnoticed. Persistent mode always comes back, as it
    /// only went away with the previous field.
    pub fn on_focus(&mut self, now: Instant, b: &Behavior) -> Command {
        if b.enabled && (b.show_on_focus || b.duration.is_none()) && self.known == Some(true) {
            self.show(b.duration.map(|d| now + d))
        } else {
            Command::None
        }
    }

    /// No text field has keyboard focus any more: the indicator belongs to
    /// the text cursor, so it goes too (even in persistent mode).
    pub fn on_blur(&mut self) -> Command {
        self.hide()
    }

    /// The text cursor is back after a selection hid it: what was showing
    /// shows again for the rest of its time, otherwise it is like focusing
    /// the field.
    pub fn on_cursor_back(&mut self, now: Instant, b: &Behavior) -> Command {
        if self.visible {
            Command::Show
        } else {
            self.on_focus(now, b)
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
            (Some(true), None) => self.show(None),
            (_, Some(d)) if self.visible && self.hide_at.is_none() => {
                self.hide_at = Some(now + d);
                Command::None
            }
            _ => Command::None,
        }
    }

    fn show(&mut self, hide_at: Option<Instant>) -> Command {
        self.visible = true;
        self.hide_at = hide_at;
        Command::Show
    }

    fn hide(&mut self) -> Command {
        self.hide_at = None;
        if std::mem::take(&mut self.visible) {
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
        Behavior { enabled: true, duration: Some(Duration::from_millis(ms)), show_on_focus: true }
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
        assert_eq!(s.on_caps(true, t0, &timed(1500)), Command::Show);
        assert_eq!(s.on_tick(t0 + Duration::from_millis(1000)), Command::None);
        assert_eq!(s.on_tick(t0 + Duration::from_millis(1500)), Command::Hide);
        assert!(!s.is_visible());
    }

    #[test]
    fn focusing_a_field_with_caps_lock_on_shows_it() {
        let t0 = Instant::now();
        let mut s = IndicatorState::default();
        s.on_caps(true, t0, &timed(1500));
        assert_eq!(s.on_tick(t0 + Duration::from_secs(5)), Command::None);
        let t1 = t0 + Duration::from_secs(10);
        assert_eq!(s.on_focus(t1, &timed(1500)), Command::Show);
        assert_eq!(s.on_tick(t1 + Duration::from_millis(1500)), Command::Hide);
    }

    #[test]
    fn focusing_a_field_with_caps_lock_off_or_the_option_off_shows_nothing() {
        let t0 = Instant::now();
        let mut s = IndicatorState::default();
        assert_eq!(s.on_focus(t0, &timed(1500)), Command::None, "state unknown");
        s.on_caps(false, t0, &timed(1500));
        assert_eq!(s.on_focus(t0, &timed(1500)), Command::None);
        s.on_caps(true, t0, &timed(1500));
        let off = Behavior { show_on_focus: false, ..timed(1500) };
        s.on_tick(t0 + Duration::from_secs(2));
        assert_eq!(s.on_focus(t0 + Duration::from_secs(3), &off), Command::None);
    }

    #[test]
    fn short_durations_hide_on_time() {
        let t0 = Instant::now();
        let mut s = IndicatorState::default();
        s.on_caps(false, t0, &timed(200));
        assert_eq!(s.on_caps(true, t0, &timed(200)), Command::Show);
        assert_eq!(s.on_tick(t0 + Duration::from_millis(199)), Command::None);
        assert_eq!(s.on_tick(t0 + Duration::from_millis(200)), Command::Hide);
    }

    #[test]
    fn turning_off_only_hides_as_on_a_mac() {
        let t0 = Instant::now();
        let mut s = IndicatorState::default();
        s.on_caps(false, t0, &timed(3000));
        s.on_caps(true, t0, &timed(3000));
        assert_eq!(s.on_caps(false, t0, &timed(3000)), Command::Hide);
        assert_eq!(s.deadline(), None);
        // turned off after it was gone: nothing shows
        s.on_caps(true, t0, &timed(3000));
        s.on_tick(t0 + Duration::from_secs(3));
        assert_eq!(s.on_caps(false, t0 + Duration::from_secs(4), &timed(3000)), Command::None);
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
        assert_eq!(s.on_caps(true, t0, &b), Command::Show, "already on at startup");
        assert_eq!(s.deadline(), None);
        assert_eq!(s.on_tick(t0 + Duration::from_secs(3600)), Command::None);
        assert_eq!(s.on_caps(false, t0, &b), Command::Hide);
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
    fn leaving_the_field_hides_persistent_mode_until_the_next_field() {
        let t0 = Instant::now();
        let b = Behavior { duration: None, show_on_focus: false, ..timed(0) };
        let mut s = IndicatorState::default();
        s.on_caps(false, t0, &b);
        assert_eq!(s.on_caps(true, t0, &b), Command::Show);
        assert_eq!(s.on_blur(), Command::Hide);
        assert!(!s.is_visible());
        assert_eq!(s.on_blur(), Command::None, "already hidden");
        assert_eq!(s.on_focus(t0 + Duration::from_secs(5), &b), Command::Show);
        assert_eq!(s.deadline(), None, "stays while Caps Lock is on");
        s.on_caps(false, t0, &b);
        s.on_blur();
        assert_eq!(s.on_focus(t0 + Duration::from_secs(9), &b), Command::None, "Caps Lock is off");
    }

    #[test]
    fn the_cursor_coming_back_shows_the_rest_or_acts_like_focus() {
        let t0 = Instant::now();
        let mut s = IndicatorState::default();
        s.on_caps(false, t0, &timed(1500));
        s.on_caps(true, t0, &timed(1500));
        // the selection collapsed within the duration: no new timer
        assert_eq!(s.on_cursor_back(t0 + Duration::from_millis(500), &timed(1500)), Command::Show);
        assert_eq!(s.deadline(), Some(t0 + Duration::from_millis(1500)));
        // after it ran out: as if the field had just been focused
        s.on_tick(t0 + Duration::from_millis(1500));
        let t1 = t0 + Duration::from_secs(3);
        assert_eq!(s.on_cursor_back(t1, &timed(1500)), Command::Show);
        assert_eq!(s.deadline(), Some(t1 + Duration::from_millis(1500)));
        s.on_tick(t1 + Duration::from_millis(1500));
        let off = Behavior { show_on_focus: false, ..timed(1500) };
        assert_eq!(s.on_cursor_back(t1 + Duration::from_secs(2), &off), Command::None);
    }

    #[test]
    fn switching_to_persistent_mode_shows_active_caps_lock() {
        let t0 = Instant::now();
        let mut s = IndicatorState::default();
        s.on_caps(false, t0, &timed(1500));
        s.on_caps(true, t0, &timed(1500));
        s.on_tick(t0 + Duration::from_secs(2));
        let persistent = Behavior { duration: None, ..timed(0) };
        assert_eq!(s.on_settings(t0, &persistent), Command::Show);
    }

}
