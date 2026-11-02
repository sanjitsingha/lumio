//! Installed web browsers, as registered for Windows' "Default apps"
//! (`Clients\StartMenuInternet`), and opening links in one of them.

use std::path::Path;

use windows::core::{w, PWSTR};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use windows::Win32::UI::Shell::{
    AssocQueryStringW, ASSOCF_IS_PROTOCOL, ASSOCSTR, ASSOCSTR_EXECUTABLE, ASSOCSTR_FRIENDLYAPPNAME,
};

use super::{launch, registry};
use crate::platform::Browser;

const CLIENTS: &str = r"SOFTWARE\Clients\StartMenuInternet";

/// What is registered for `https` links, i.e. about the default browser.
fn https_association(kind: ASSOCSTR) -> Option<String> {
    let mut len: u32 = 0;
    unsafe {
        let _ = AssocQueryStringW(ASSOCF_IS_PROTOCOL, kind, w!("https"), None, None, &mut len);
    }
    if len == 0 {
        return None;
    }
    let mut buf = vec![0u16; len as usize];
    let hr = unsafe { AssocQueryStringW(ASSOCF_IS_PROTOCOL, kind, w!("https"), None, Some(PWSTR(buf.as_mut_ptr())), &mut len) };
    if hr.is_err() {
        return None;
    }
    let end = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..end])).filter(|s| !s.is_empty())
}

/// Program of the default browser.
pub fn default_executable() -> Option<String> {
    https_association(ASSOCSTR_EXECUTABLE)
}

/// The browser web searches open in: `id`, or the default browser when `id`
/// is empty or no longer installed. The default has no id here; its name
/// ("Google Chrome") is what finds its icon.
pub fn resolve(id: &str) -> Option<Browser> {
    if !id.is_empty() {
        if let Some(browser) = installed().into_iter().find(|b| b.id == id) {
            return Some(browser);
        }
    }
    https_association(ASSOCSTR_FRIENDLYAPPNAME).map(|name| Browser { id: String::new(), name })
}

/// The program in a `shell\open\command` value: `"C:\…\chrome.exe"` or `C:\…\x.exe -arg`.
fn program(command: &str) -> Option<&str> {
    let command = command.trim();
    let exe = match command.strip_prefix('"') {
        Some(rest) => rest.split('"').next()?,
        None => {
            // unquoted paths may contain spaces; ASCII lowercasing keeps byte offsets
            let end = command.to_ascii_lowercase().find(".exe").map_or(command.len(), |i| i + 4);
            &command[..end]
        }
    };
    (!exe.is_empty()).then_some(exe)
}

/// `(id, name, program)` of each registered browser; per-user registrations win.
fn registered() -> Vec<(String, String, String)> {
    let mut found: Vec<(String, String, String)> = Vec::new();
    for root in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        for id in registry::subkeys(root, CLIENTS) {
            // retired: Windows 11 sends it to Edge, and Default Apps no longer lists it
            if id.eq_ignore_ascii_case("IEXPLORE.EXE") || found.iter().any(|(known, ..)| known.eq_ignore_ascii_case(&id)) {
                continue;
            }
            let key = format!(r"{CLIENTS}\{id}");
            let Some(command) = registry::read_string(root, &format!(r"{key}\shell\open\command"), "") else { continue };
            let Some(exe) = program(&command).filter(|p| Path::new(p).is_file()) else { continue };
            let name = registry::read_string(root, &key, "").filter(|n| !n.is_empty()).unwrap_or_else(|| id.clone());
            found.push((id.clone(), name, exe.to_string()));
        }
    }
    found
}

pub fn installed() -> Vec<Browser> {
    let mut browsers: Vec<Browser> = registered().into_iter().map(|(id, name, _)| Browser { id, name }).collect();
    browsers.sort_by_key(|b| b.name.to_lowercase());
    browsers
}

/// Opens an `https://` link in the browser `id` (from [`installed`]), or in
/// the default browser when `id` is empty or no longer installed.
pub fn open_url(url: &str, id: &str) -> Result<(), String> {
    if !url.starts_with("https://") {
        return Err("only https links can be opened".into());
    }
    if !id.is_empty() {
        match registered().into_iter().find(|(known, ..)| known == id) {
            Some((_, name, exe)) => {
                return std::process::Command::new(&exe)
                    .arg(url)
                    .spawn()
                    .map(|_| ())
                    .map_err(|e| format!("could not start {name}: {e}"));
            }
            None => log::warn!("browser “{id}” is no longer installed; using the default browser"),
        }
    }
    launch::open_url(url)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_the_program_from_open_commands() {
        let chrome = r#""C:\Program Files\Google\Chrome\Application\chrome.exe""#;
        assert_eq!(program(chrome), Some(r"C:\Program Files\Google\Chrome\Application\chrome.exe"));
        let firefox = r#""C:\Program Files\Mozilla Firefox\firefox.exe" -osint -url "%1""#;
        assert_eq!(program(firefox), Some(r"C:\Program Files\Mozilla Firefox\firefox.exe"));
        assert_eq!(program(r"C:\Program Files\Browser\Browser.EXE --new"), Some(r"C:\Program Files\Browser\Browser.EXE"));
        assert_eq!(program(r"C:\Tools\browser"), Some(r"C:\Tools\browser"));
        assert_eq!(program("  "), None);
        assert_eq!(program(r#""""#), None);
    }

    #[test]
    fn only_https_links_open() {
        assert!(open_url("file:///C:/Windows/System32/calc.exe", "").is_err());
    }
}
