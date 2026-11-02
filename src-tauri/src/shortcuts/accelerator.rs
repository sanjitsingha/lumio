//! Canonical keyboard shortcut representation shared by all backends.
//!
//! Text form is `Ctrl+Alt+Shift+Super+Key` (modifiers always in that order),
//! which is what settings store and the UI displays.

use std::fmt;
use std::str::FromStr;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub super_key: bool,
}

impl Modifiers {
    pub fn is_empty(&self) -> bool {
        !(self.ctrl || self.alt || self.shift || self.super_key)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Key {
    /// `A`-`Z` or `0`-`9`.
    Char(char),
    /// `F1`-`F24`.
    F(u8),
    Space,
    Enter,
    Tab,
    Backspace,
    Delete,
    Insert,
    Home,
    End,
    PageUp,
    PageDown,
    ArrowUp,
    ArrowDown,
    ArrowLeft,
    ArrowRight,
    Escape,
    Comma,
    Period,
    Slash,
    Semicolon,
    Quote,
    BracketLeft,
    BracketRight,
    Backslash,
    Minus,
    Equal,
    Backquote,
}

/// (canonical name, gtk keysym name, aliases)
const NAMED_KEYS: &[(Key, &str, &str, &[&str])] = &[
    (Key::Space, "Space", "space", &[]),
    (Key::Enter, "Enter", "Return", &["return"]),
    (Key::Tab, "Tab", "Tab", &[]),
    (Key::Backspace, "Backspace", "BackSpace", &[]),
    (Key::Delete, "Delete", "Delete", &["del"]),
    (Key::Insert, "Insert", "Insert", &["ins"]),
    (Key::Home, "Home", "Home", &[]),
    (Key::End, "End", "End", &[]),
    (Key::PageUp, "PageUp", "Page_Up", &["pgup", "prior"]),
    (Key::PageDown, "PageDown", "Page_Down", &["pgdn", "next"]),
    (Key::ArrowUp, "Up", "Up", &["arrowup"]),
    (Key::ArrowDown, "Down", "Down", &["arrowdown"]),
    (Key::ArrowLeft, "Left", "Left", &["arrowleft"]),
    (Key::ArrowRight, "Right", "Right", &["arrowright"]),
    (Key::Escape, "Escape", "Escape", &["esc"]),
    (Key::Comma, "Comma", "comma", &[","]),
    (Key::Period, "Period", "period", &["."]),
    (Key::Slash, "Slash", "slash", &["/"]),
    (Key::Semicolon, "Semicolon", "semicolon", &[";"]),
    (Key::Quote, "Quote", "apostrophe", &["'", "apostrophe"]),
    (Key::BracketLeft, "BracketLeft", "bracketleft", &["["]),
    (Key::BracketRight, "BracketRight", "bracketright", &["]"]),
    (Key::Backslash, "Backslash", "backslash", &["\\"]),
    (Key::Minus, "Minus", "minus", &["-"]),
    (Key::Equal, "Equal", "equal", &["=", "equals"]),
    (Key::Backquote, "Backquote", "grave", &["`", "grave"]),
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Accelerator {
    pub mods: Modifiers,
    pub key: Key,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ParseError {
    Empty,
    UnknownToken(String),
    MissingKey,
    MultipleKeys,
}

impl fmt::Display for ParseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParseError::Empty => write!(f, "The shortcut is empty."),
            ParseError::UnknownToken(t) => write!(f, "\"{t}\" is not a supported key."),
            ParseError::MissingKey => write!(f, "A shortcut needs a key in addition to the modifiers."),
            ParseError::MultipleKeys => write!(f, "A shortcut can only contain one non-modifier key."),
        }
    }
}

/// Why a syntactically valid shortcut cannot be used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ValidationError {
    NeedsModifier,
    ShiftOnly,
    Escape,
    Reserved(&'static str),
}

impl fmt::Display for ValidationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ValidationError::NeedsModifier => write!(
                f,
                "Add a modifier such as Ctrl, Alt or Super. Only F13-F24 can be used on their own."
            ),
            ValidationError::ShiftOnly => {
                write!(f, "Shift on its own would interfere with typing. Add Ctrl, Alt or Super.")
            }
            ValidationError::Escape => write!(f, "Escape is reserved for closing Lummo windows."),
            ValidationError::Reserved(what) => write!(f, "This shortcut is reserved by the operating system ({what})."),
        }
    }
}

fn parse_key(token: &str) -> Option<Key> {
    let lower = token.to_ascii_lowercase();
    if token.chars().count() == 1 {
        let c = token.chars().next().unwrap().to_ascii_uppercase();
        if c.is_ascii_alphanumeric() {
            return Some(Key::Char(c));
        }
    }
    if let Some(rest) = lower.strip_prefix("key") {
        if rest.len() == 1 && rest.chars().all(|c| c.is_ascii_alphabetic()) {
            return Some(Key::Char(rest.chars().next().unwrap().to_ascii_uppercase()));
        }
    }
    if let Some(rest) = lower.strip_prefix("digit") {
        if rest.len() == 1 && rest.chars().all(|c| c.is_ascii_digit()) {
            return Some(Key::Char(rest.chars().next().unwrap()));
        }
    }
    if let Some(rest) = lower.strip_prefix('f') {
        if let Ok(n) = rest.parse::<u8>() {
            if (1..=24).contains(&n) {
                return Some(Key::F(n));
            }
        }
    }
    NAMED_KEYS.iter().find_map(|(key, name, gtk, aliases)| {
        let matches = lower == name.to_ascii_lowercase()
            || lower == gtk.to_ascii_lowercase()
            || aliases.iter().any(|a| *a == lower);
        matches.then_some(*key)
    })
}

fn apply_modifier(mods: &mut Modifiers, token: &str) -> bool {
    match token.to_ascii_lowercase().as_str() {
        "ctrl" | "control" | "primary" | "cmdorctrl" | "commandorcontrol" => mods.ctrl = true,
        "alt" | "option" | "altgr" => mods.alt = true,
        "shift" => mods.shift = true,
        "super" | "meta" | "win" | "windows" | "cmd" | "command" | "logo" => mods.super_key = true,
        _ => return false,
    }
    true
}

impl FromStr for Accelerator {
    type Err = ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let s = s.trim();
        if s.is_empty() {
            return Err(ParseError::Empty);
        }
        let mut mods = Modifiers::default();
        let mut key = None;
        for token in s.split('+').map(str::trim) {
            if token.is_empty() {
                return Err(ParseError::UnknownToken("+".into()));
            }
            if apply_modifier(&mut mods, token) {
                continue;
            }
            let parsed = parse_key(token).ok_or_else(|| ParseError::UnknownToken(token.to_string()))?;
            if key.replace(parsed).is_some() {
                return Err(ParseError::MultipleKeys);
            }
        }
        key.map(|key| Accelerator { mods, key }).ok_or(ParseError::MissingKey)
    }
}

impl Key {
    pub fn name(&self) -> String {
        match self {
            Key::Char(c) => c.to_string(),
            Key::F(n) => format!("F{n}"),
            other => NAMED_KEYS.iter().find(|(k, ..)| k == other).map(|(_, n, ..)| n.to_string()).unwrap_or_default(),
        }
    }

    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    pub fn gtk_name(&self) -> String {
        match self {
            Key::Char(c) => c.to_ascii_lowercase().to_string(),
            Key::F(n) => format!("F{n}"),
            other => NAMED_KEYS.iter().find(|(k, ..)| k == other).map(|(_, _, g, _)| g.to_string()).unwrap_or_default(),
        }
    }
}

impl fmt::Display for Accelerator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let m = &self.mods;
        for (on, name) in [(m.ctrl, "Ctrl"), (m.alt, "Alt"), (m.shift, "Shift"), (m.super_key, "Super")] {
            if on {
                write!(f, "{name}+")?;
            }
        }
        write!(f, "{}", self.key.name())
    }
}

impl Accelerator {
    /// GTK/Mutter accelerator syntax, e.g. `<Control><Alt>space`.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    pub fn to_gtk(self) -> String {
        let m = &self.mods;
        let mut out = String::new();
        for (on, name) in [(m.ctrl, "<Control>"), (m.alt, "<Alt>"), (m.shift, "<Shift>"), (m.super_key, "<Super>")] {
            if on {
                out.push_str(name);
            }
        }
        out.push_str(&self.key.gtk_name());
        out
    }

    /// Parses GTK accelerator syntax as stored in GSettings keybinding keys.
    #[cfg_attr(not(target_os = "linux"), allow(dead_code))]
    pub fn from_gtk(s: &str) -> Option<Accelerator> {
        let mut mods = Modifiers::default();
        let mut rest = s.trim();
        while let Some(stripped) = rest.strip_prefix('<') {
            let end = stripped.find('>')?;
            if !apply_modifier(&mut mods, &stripped[..end]) {
                return None;
            }
            rest = &stripped[end + 1..];
        }
        let key = parse_key(rest)?;
        Some(Accelerator { mods, key })
    }

    pub fn validate(&self) -> Result<(), ValidationError> {
        let m = &self.mods;
        if self.key == Key::Escape {
            return Err(ValidationError::Escape);
        }
        let is_function_key = matches!(self.key, Key::F(_));
        if m.is_empty() && !matches!(self.key, Key::F(13..=24)) {
            return Err(ValidationError::NeedsModifier);
        }
        if m.shift && !(m.ctrl || m.alt || m.super_key) && !is_function_key {
            return Err(ValidationError::ShiftOnly);
        }
        if let Some(reason) = reserved_reason(self) {
            return Err(ValidationError::Reserved(reason));
        }
        Ok(())
    }
}

fn reserved_reason(accel: &Accelerator) -> Option<&'static str> {
    let text = accel.to_string();
    let common: &[(&str, &str)] = &[
        ("Ctrl+Alt+Delete", "secure attention sequence"),
        ("Alt+Tab", "window switching"),
        ("Alt+Shift+Tab", "window switching"),
        ("Alt+F4", "close window"),
    ];
    #[cfg(windows)]
    let platform: &[(&str, &str)] = &[
        ("Super+L", "lock screen"),
        ("Ctrl+Shift+Escape", "Task Manager"),
        ("Ctrl+Escape", "Start menu"),
        ("Alt+Escape", "window cycling"),
        ("Super+Tab", "Task View"),
    ];
    #[cfg(not(windows))]
    let platform: &[(&str, &str)] = &[("Ctrl+Alt+Backspace", "restart display server"), ("Super+L", "lock screen")];
    common.iter().chain(platform).find(|(combo, _)| *combo == text).map(|(_, why)| *why)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn acc(s: &str) -> Accelerator {
        s.parse().unwrap_or_else(|e| panic!("{s}: {e}"))
    }

    #[test]
    fn parses_and_formats_canonically() {
        assert_eq!(acc("Alt+Space").to_string(), "Alt+Space");
        assert_eq!(acc("space+alt").to_string(), "Alt+Space");
        assert_eq!(acc("shift+super+ctrl+k").to_string(), "Ctrl+Shift+Super+K");
        assert_eq!(acc("Control+KeyP").to_string(), "Ctrl+P");
        assert_eq!(acc("Ctrl+Digit1").to_string(), "Ctrl+1");
        assert_eq!(acc("CmdOrCtrl+Return").to_string(), "Ctrl+Enter");
        assert_eq!(acc("Meta+PgUp").to_string(), "Super+PageUp");
        assert_eq!(acc("Ctrl+,").to_string(), "Ctrl+Comma");
        assert_eq!(acc("F13").to_string(), "F13");
    }

    #[test]
    fn rejects_malformed_input() {
        assert_eq!("".parse::<Accelerator>(), Err(ParseError::Empty));
        assert_eq!("Ctrl+Alt".parse::<Accelerator>(), Err(ParseError::MissingKey));
        assert_eq!("Ctrl+A+B".parse::<Accelerator>(), Err(ParseError::MultipleKeys));
        assert!(matches!("Ctrl+Banana".parse::<Accelerator>(), Err(ParseError::UnknownToken(_))));
        assert!(matches!("Ctrl++".parse::<Accelerator>(), Err(ParseError::UnknownToken(_))));
        assert!(matches!("F25".parse::<Accelerator>(), Err(ParseError::UnknownToken(_))));
    }

    #[test]
    fn converts_to_and_from_gtk_syntax() {
        assert_eq!(acc("Alt+Space").to_gtk(), "<Alt>space");
        assert_eq!(acc("Ctrl+Shift+PageDown").to_gtk(), "<Control><Shift>Page_Down");
        assert_eq!(acc("Super+Backquote").to_gtk(), "<Super>grave");
        assert_eq!(Accelerator::from_gtk("<Alt>space"), Some(acc("Alt+Space")));
        assert_eq!(Accelerator::from_gtk("<Primary><Shift>t"), Some(acc("Ctrl+Shift+T")));
        assert_eq!(Accelerator::from_gtk("<Super><Alt>Left"), Some(acc("Alt+Super+Left")));
        assert_eq!(Accelerator::from_gtk("<Hyper>x"), None);
        for s in ["Alt+Space", "Ctrl+Alt+F12", "Super+Slash", "Ctrl+Quote"] {
            assert_eq!(Accelerator::from_gtk(&acc(s).to_gtk()), Some(acc(s)), "{s}");
        }
    }

    #[test]
    fn validation_rules() {
        assert!(acc("Alt+Space").validate().is_ok());
        assert!(acc("Ctrl+Space").validate().is_ok());
        assert!(acc("Shift+F5").validate().is_ok());
        assert!(acc("F13").validate().is_ok());
        assert_eq!(acc("K").validate(), Err(ValidationError::NeedsModifier));
        assert_eq!(acc("F5").validate(), Err(ValidationError::NeedsModifier));
        assert_eq!(acc("Shift+K").validate(), Err(ValidationError::ShiftOnly));
        assert_eq!(acc("Ctrl+Escape").validate(), Err(ValidationError::Escape));
        assert!(matches!(acc("Alt+Tab").validate(), Err(ValidationError::Reserved(_))));
        assert!(matches!(acc("Ctrl+Alt+Delete").validate(), Err(ValidationError::Reserved(_))));
    }
}
