//! Minimal freedesktop Desktop Entry parser (`[Desktop Entry]` group only).
//! <https://specifications.freedesktop.org/desktop-entry-spec/latest/>

use std::collections::HashMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Default, PartialEq)]
pub struct DesktopEntry {
    pub entry_type: String,
    pub name: String,
    pub generic_name: Option<String>,
    pub comment: Option<String>,
    pub keywords: Vec<String>,
    pub categories: Vec<String>,
    pub icon: Option<String>,
    pub exec: Option<String>,
    pub try_exec: Option<String>,
    pub no_display: bool,
    pub hidden: bool,
    pub terminal: bool,
    pub only_show_in: Vec<String>,
    pub not_show_in: Vec<String>,
}

/// Locale fallbacks for `LANG`-style strings, most specific first:
/// `de_DE.UTF-8@euro` -> `de_DE@euro`, `de_DE`, `de@euro`, `de`.
pub fn locale_candidates(locale: &str) -> Vec<String> {
    let without_encoding = match locale.split_once('.') {
        Some((base, rest)) => match rest.split_once('@') {
            Some((_, modifier)) => format!("{base}@{modifier}"),
            None => base.to_string(),
        },
        None => locale.to_string(),
    };
    let (base, modifier) = match without_encoding.split_once('@') {
        Some((b, m)) => (b.to_string(), Some(m.to_string())),
        None => (without_encoding.clone(), None),
    };
    let (lang, country) = match base.split_once('_') {
        Some((l, c)) => (l.to_string(), Some(c.to_string())),
        None => (base.clone(), None),
    };
    let mut out = Vec::new();
    if let (Some(c), Some(m)) = (&country, &modifier) {
        out.push(format!("{lang}_{c}@{m}"));
    }
    if let Some(c) = &country {
        out.push(format!("{lang}_{c}"));
    }
    if let Some(m) = &modifier {
        out.push(format!("{lang}@{m}"));
    }
    if !lang.is_empty() && lang != "C" && lang != "POSIX" {
        out.push(lang);
    }
    out
}

pub fn current_locales() -> Vec<String> {
    let locale = ["LC_ALL", "LC_MESSAGES", "LANG"]
        .iter()
        .find_map(|k| std::env::var(k).ok().filter(|v| !v.is_empty()))
        .unwrap_or_default();
    locale_candidates(&locale)
}

fn unescape(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    let mut chars = value.chars();
    while let Some(c) = chars.next() {
        if c != '\\' {
            out.push(c);
            continue;
        }
        match chars.next() {
            Some('s') => out.push(' '),
            Some('n') => out.push('\n'),
            Some('t') => out.push('\t'),
            Some('r') => out.push('\r'),
            Some('\\') => out.push('\\'),
            Some(';') => out.push_str("\\;"),
            Some(other) => {
                out.push('\\');
                out.push(other);
            }
            None => out.push('\\'),
        }
    }
    out
}

fn split_list(value: &str) -> Vec<String> {
    let mut items = Vec::new();
    let mut current = String::new();
    let mut chars = value.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\\' if chars.peek() == Some(&';') => {
                chars.next();
                current.push(';');
            }
            ';' => {
                let item = current.trim().to_string();
                if !item.is_empty() {
                    items.push(item);
                }
                current.clear();
            }
            c => current.push(c),
        }
    }
    let item = current.trim().to_string();
    if !item.is_empty() {
        items.push(item);
    }
    items
}

pub fn parse(text: &str, locales: &[String]) -> Option<DesktopEntry> {
    let mut in_main = false;
    let mut raw: HashMap<&str, &str> = HashMap::new();
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if line.starts_with('[') {
            in_main = line == "[Desktop Entry]";
            continue;
        }
        if !in_main {
            continue;
        }
        if let Some((k, v)) = line.split_once('=') {
            raw.entry(k.trim()).or_insert(v.trim());
        }
    }
    if raw.is_empty() {
        return None;
    }
    let localized = |key: &str| -> Option<String> {
        locales
            .iter()
            .find_map(|l| raw.get(format!("{key}[{l}]").as_str()))
            .or_else(|| raw.get(key))
            .map(|v| unescape(v))
    };
    let flag = |key: &str| raw.get(key).is_some_and(|v| v.eq_ignore_ascii_case("true"));
    let list = |key: &str| raw.get(key).map(|v| split_list(&unescape(v))).unwrap_or_default();
    let keywords = locales
        .iter()
        .find_map(|l| raw.get(format!("Keywords[{l}]").as_str()))
        .or_else(|| raw.get("Keywords"))
        .map(|v| split_list(&unescape(v)))
        .unwrap_or_default();
    Some(DesktopEntry {
        entry_type: raw.get("Type").map(|s| s.to_string()).unwrap_or_default(),
        name: localized("Name")?,
        generic_name: localized("GenericName"),
        comment: localized("Comment"),
        keywords,
        categories: list("Categories"),
        icon: raw.get("Icon").map(|v| unescape(v)).filter(|s| !s.is_empty()),
        exec: raw.get("Exec").map(|v| v.to_string()),
        try_exec: raw.get("TryExec").map(|v| unescape(v)),
        no_display: flag("NoDisplay"),
        hidden: flag("Hidden"),
        terminal: flag("Terminal"),
        only_show_in: list("OnlyShowIn"),
        not_show_in: list("NotShowIn"),
    })
}

impl DesktopEntry {
    /// Should this entry appear in an application launcher on `desktops`
    /// (the colon-separated `XDG_CURRENT_DESKTOP` values)?
    pub fn is_visible(&self, desktops: &[String]) -> bool {
        if self.entry_type != "Application" || self.no_display || self.hidden {
            return false;
        }
        let on = |list: &[String]| list.iter().any(|d| desktops.iter().any(|c| c.eq_ignore_ascii_case(d)));
        if !self.only_show_in.is_empty() && !on(&self.only_show_in) {
            return false;
        }
        if on(&self.not_show_in) {
            return false;
        }
        match &self.try_exec {
            Some(program) => find_program(program).is_some(),
            None => true,
        }
    }
}

/// Resolves a program name against `PATH` (absolute paths are checked directly).
pub fn find_program(program: &str) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    let is_exec = |p: &Path| p.metadata().is_ok_and(|m| m.is_file() && m.permissions().mode() & 0o111 != 0);
    if program.contains('/') {
        let p = PathBuf::from(program);
        return is_exec(&p).then_some(p);
    }
    std::env::var_os("PATH")
        .map(|paths| std::env::split_paths(&paths).map(|d| d.join(program)).find(|p| is_exec(p)))
        .unwrap_or(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    const FIREFOX: &str = r#"
[Desktop Entry]
Version=1.0
Name=Firefox Web Browser
Name[de]=Firefox-Webbrowser
GenericName=Web Browser
Comment=Browse the World Wide Web
Keywords=Internet;WWW;Browser;Web;Explorer
Keywords[de]=Internet;WWW;Browser
Exec=firefox %u
Icon=firefox
Terminal=false
Type=Application
Categories=GNOME;GTK;Network;WebBrowser;

[Desktop Action new-window]
Name=Open a New Window
Exec=firefox -new-window
"#;

    #[test]
    fn parses_main_group_only() {
        let e = parse(FIREFOX, &[]).unwrap();
        assert_eq!(e.name, "Firefox Web Browser");
        assert_eq!(e.generic_name.as_deref(), Some("Web Browser"));
        assert_eq!(e.exec.as_deref(), Some("firefox %u"));
        assert_eq!(e.icon.as_deref(), Some("firefox"));
        assert_eq!(e.keywords, vec!["Internet", "WWW", "Browser", "Web", "Explorer"]);
        assert_eq!(e.categories.len(), 4);
        assert!(!e.terminal && !e.no_display);
    }

    #[test]
    fn picks_localized_values() {
        let e = parse(FIREFOX, &locale_candidates("de_DE.UTF-8")).unwrap();
        assert_eq!(e.name, "Firefox-Webbrowser");
        assert_eq!(e.keywords, vec!["Internet", "WWW", "Browser"]);
        assert_eq!(e.comment.as_deref(), Some("Browse the World Wide Web"), "falls back to unlocalized");
    }

    #[test]
    fn locale_fallback_order() {
        assert_eq!(locale_candidates("sr_RS.UTF-8@latin"), vec!["sr_RS@latin", "sr_RS", "sr@latin", "sr"]);
        assert_eq!(locale_candidates("C.UTF-8"), Vec::<String>::new());
        assert_eq!(locale_candidates("fr"), vec!["fr"]);
    }

    #[test]
    fn escapes_and_lists() {
        assert_eq!(unescape(r"a\sb\\c"), r"a b\c");
        assert_eq!(split_list(r"one;two\;three;;"), vec!["one", "two;three"]);
    }

    #[test]
    fn visibility_rules() {
        let base = parse(FIREFOX, &[]).unwrap();
        let desktops = vec!["Pantheon".to_string()];
        assert!(base.is_visible(&desktops));
        assert!(!DesktopEntry { no_display: true, ..base.clone() }.is_visible(&desktops));
        assert!(!DesktopEntry { only_show_in: vec!["KDE".into()], ..base.clone() }.is_visible(&desktops));
        assert!(DesktopEntry { only_show_in: vec!["pantheon".into()], ..base.clone() }.is_visible(&desktops));
        assert!(!DesktopEntry { not_show_in: vec!["Pantheon".into()], ..base.clone() }.is_visible(&desktops));
        assert!(!DesktopEntry { entry_type: "Link".into(), ..base.clone() }.is_visible(&desktops));
        assert!(!DesktopEntry { try_exec: Some("definitely-not-installed-lumio".into()), ..base.clone() }.is_visible(&desktops));
        assert!(DesktopEntry { try_exec: Some("sh".into()), ..base }.is_visible(&desktops));
    }

    #[test]
    fn missing_name_or_group_is_rejected() {
        assert!(parse("[Desktop Entry]\nType=Application\n", &[]).is_none());
        assert!(parse("Name=Orphan\n", &[]).is_none());
    }
}
