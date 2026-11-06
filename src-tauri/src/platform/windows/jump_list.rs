//! The tasks an app puts in its taskbar jump list, Windows' counterpart of a
//! desktop entry's actions: Chrome's *New window* and *New Incognito window*,
//! VS Code's *New Window*, Windows Terminal's profiles, Calculator's modes.
//!
//! Windows has no API for reading another app's jump list. It keeps each one
//! in `%APPDATA%\Microsoft\Windows\Recent\CustomDestinations\<AppID>.customDestinations-ms`,
//! named after a CRC-64 of the app's AppUserModelID (its AppsFolder parsing
//! name), and only the app itself writes it (`ICustomDestinationList` or
//! `Windows.UI.StartScreen.JumpList`, usually each time it starts). The format
//! is undocumented but has not changed since Windows 7:
//!
//! ```text
//! u32 version, u32 category count, u32 reserved
//! each category: u32 type
//!     0, named ("Most visited"): u16 name length, UTF-16 name, u32 entry count, entries
//!     1, known (Recent, Frequent): u32 category id
//!     2, tasks:                    u32 entry count, entries
//!   then u32 0xBABFFBAB
//! each entry: a 16-byte class id, then the object as IPersistStream saved it
//! ```
//!
//! Only the tasks are offered. Entries are loaded only as shell links, by the
//! shell's own `ShellLink` class; a file holding anything else is read no further.

use std::path::{Path, PathBuf};

use windows::core::{Interface, GUID, PCWSTR};
use windows::Win32::Foundation::PROPERTYKEY;
use windows::Win32::System::Com::StructuredStorage::{
    PropVariantClear, PropVariantToBooleanWithDefault, PropVariantToStringAlloc, PROPVARIANT,
};
use windows::Win32::System::Com::{
    CoCreateInstance, CoTaskMemFree, IPersistStream, IStream, CLSCTX_INPROC_SERVER, STREAM_SEEK_CUR, STREAM_SEEK_SET,
};
use windows::Win32::UI::Shell::PropertiesSystem::IPropertyStore;
use windows::Win32::UI::Shell::{
    IShellLinkDataList, IShellLinkW, SHCreateMemStream, SHLoadIndirectString, ShellLink, SLDF_RUNAS_USER,
};

const fn key(fmtid: u128, pid: u32) -> PROPERTYKEY {
    PROPERTYKEY { fmtid: GUID::from_u128(fmtid), pid }
}

const APP_USER_MODEL: u128 = 0x9f4c2855_9f79_4b39_a8d0_e1d42de1d5f3;
/// `System.Title`: the task's name as the jump list shows it.
const PKEY_TITLE: PROPERTYKEY = key(0xf29f85e0_4ff9_1068_ab91_08002b27b3d9, 2);
/// `System.AppUserModel.ID`: the app a Store app's task starts.
const PKEY_APP_ID: PROPERTYKEY = key(APP_USER_MODEL, 5);
/// `System.AppUserModel.IsDestListSeparator`: a line between tasks, not a task.
const PKEY_SEPARATOR: PROPERTYKEY = key(APP_USER_MODEL, 6);
/// `System.AppUserModel.ActivationContext`: the argument a Store app's task passes.
const PKEY_ACTIVATION_CONTEXT: PROPERTYKEY = key(APP_USER_MODEL, 20);

const CATEGORY_NAMED: u32 = 0;
const CATEGORY_KNOWN: u32 = 1;
const CATEGORY_TASKS: u32 = 2;
const CATEGORY_END: u32 = 0xBABF_FBAB;

/// Real jump lists are a few KB; anything far larger is not one.
const MAX_FILE_BYTES: u64 = 1 << 20;
/// Windows Terminal lists one task per profile; more than this would bury the results.
const MAX_TASKS: usize = 10;

#[derive(Debug, Clone, PartialEq)]
pub struct Task {
    /// Stable while the task is unchanged: a hash of what it shows and starts.
    pub id: String,
    pub title: String,
    pub launch: Launch,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Launch {
    /// A program and its arguments (`chrome.exe --incognito`).
    Program {
        path: String,
        arguments: String,
        directory: String,
        /// The link asks to run elevated (PowerShell's *Run as Administrator*).
        run_as_admin: bool,
    },
    /// A Store app started with an argument (Calculator's `1`, its scientific mode).
    StoreApp { app_id: String, arguments: String },
}

/// The jump-list tasks of the app with this AppUserModelID; none when it has
/// not written a jump list (it never ran) or the file cannot be read.
/// COM must be initialized on the calling thread.
pub fn tasks(app_id: &str) -> Vec<Task> {
    let Some(path) = file_for(app_id) else { return Vec::new() };
    let Ok(meta) = std::fs::metadata(&path) else { return Vec::new() };
    if meta.len() > MAX_FILE_BYTES {
        log::warn!("jump list of {app_id} is {} bytes; not read", meta.len());
        return Vec::new();
    }
    let Ok(bytes) = std::fs::read(&path) else { return Vec::new() };
    match parse(&bytes, app_id) {
        Ok(mut tasks) => {
            tasks.truncate(MAX_TASKS);
            tasks
        }
        Err(e) => {
            log::debug!("jump list of {app_id} ({}) not read: {e}", path.display());
            Vec::new()
        }
    }
}

fn file_for(app_id: &str) -> Option<PathBuf> {
    let appdata = PathBuf::from(std::env::var_os("APPDATA")?);
    let name = format!("{}.customDestinations-ms", app_id_hash(app_id));
    Some(appdata.join(r"Microsoft\Windows\Recent\CustomDestinations").join(name))
}

/// The name Windows gives an app's jump-list files: CRC-64 (reflected, poly
/// 0x92C64265D32139A4, initial all ones) of the upper-cased AppUserModelID in
/// UTF-16, in lower-case hex without leading zeros.
pub fn app_id_hash(app_id: &str) -> String {
    const POLY: u64 = 0x92C6_4265_D321_39A4;
    let mut crc = u64::MAX;
    for unit in app_id.to_uppercase().encode_utf16() {
        for byte in unit.to_le_bytes() {
            crc ^= u64::from(byte);
            for _ in 0..8 {
                crc = if crc & 1 != 0 { (crc >> 1) ^ POLY } else { crc >> 1 };
            }
        }
    }
    format!("{crc:x}")
}

struct Reader<'a> {
    bytes: &'a [u8],
    pos: usize,
}

impl Reader<'_> {
    fn take(&mut self, n: usize) -> Result<&[u8], String> {
        let end = self.pos.checked_add(n).filter(|end| *end <= self.bytes.len()).ok_or("the file ends early")?;
        let slice = &self.bytes[self.pos..end];
        self.pos = end;
        Ok(slice)
    }

    fn u16(&mut self) -> Result<u16, String> {
        Ok(u16::from_le_bytes(self.take(2)?.try_into().unwrap()))
    }

    fn u32(&mut self) -> Result<u32, String> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }

    fn guid(&mut self) -> Result<GUID, String> {
        let b = self.take(16)?;
        Ok(GUID::from_values(
            u32::from_le_bytes(b[0..4].try_into().unwrap()),
            u16::from_le_bytes(b[4..6].try_into().unwrap()),
            u16::from_le_bytes(b[6..8].try_into().unwrap()),
            b[8..16].try_into().unwrap(),
        ))
    }
}

/// The tasks in the `.customDestinations-ms` file of `app_id` (COM must be initialized).
fn parse(bytes: &[u8], app_id: &str) -> Result<Vec<Task>, String> {
    let mut r = Reader { bytes, pos: 0 };
    let _version = r.u32()?;
    let categories = r.u32()?;
    let _reserved = r.u32()?;
    let stream = unsafe { SHCreateMemStream(Some(bytes)) }.ok_or("no memory for the file")?;
    let mut tasks: Vec<Task> = Vec::new();
    for _ in 0..categories.min(64) {
        match r.u32()? {
            CATEGORY_NAMED => {
                let name_units = usize::from(r.u16()?);
                r.take(name_units * 2)?;
                // websites and documents, not actions: loaded only to step over them
                for _ in 0..r.u32()? {
                    load_link(&stream, &mut r)?;
                }
            }
            CATEGORY_KNOWN => {
                r.u32()?;
            }
            CATEGORY_TASKS => {
                for _ in 0..r.u32()? {
                    let link = load_link(&stream, &mut r)?;
                    if let Some(task) = task_from(&link, app_id) {
                        if !tasks.iter().any(|t| t.id == task.id) {
                            tasks.push(task);
                        }
                    }
                }
            }
            other => return Err(format!("unknown category type {other}")),
        }
        if r.u32()? != CATEGORY_END {
            return Err("a category does not end where expected".into());
        }
    }
    Ok(tasks)
}

/// Loads the shell link at the reader's position and moves past it.
fn load_link(stream: &IStream, r: &mut Reader<'_>) -> Result<IShellLinkW, String> {
    let class = r.guid()?;
    if class != ShellLink {
        return Err(format!("an entry is not a shell link ({class:?})"));
    }
    let start = r.pos as u64;
    let mut end = 0u64;
    let link = unsafe {
        stream.Seek(start as i64, STREAM_SEEK_SET, None).map_err(|e| e.to_string())?;
        let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).map_err(|e| e.to_string())?;
        link.cast::<IPersistStream>().and_then(|p| p.Load(stream)).map_err(|e| format!("a link does not load: {e}"))?;
        stream.Seek(0, STREAM_SEEK_CUR, Some(&mut end)).map_err(|e| e.to_string())?;
        link
    };
    if end <= start || end > r.bytes.len() as u64 {
        return Err("a link has an impossible length".into());
    }
    r.pos = end as usize;
    Ok(link)
}

/// The task a link describes, if Lummo can start it.
fn task_from(link: &IShellLinkW, app_id: &str) -> Option<Task> {
    let store: IPropertyStore = link.cast().ok()?;
    if bool_property(&store, &PKEY_SEPARATOR) {
        return None;
    }
    let title = string_property(&store, &PKEY_TITLE)
        .or_else(|| Some(read_text(|buf| unsafe { link.GetDescription(buf) })))
        .map(|t| resolve_indirect(&t).trim().to_string())
        .filter(|t| !t.is_empty())?;
    let path = read_text(|buf| unsafe { link.GetPath(buf, std::ptr::null_mut(), 0) });
    let arguments = read_text(|buf| unsafe { link.GetArguments(buf) });
    let launch = if !path.is_empty() && Path::new(&path).exists() {
        if names_missing_profile(&arguments) {
            return None;
        }
        let directory = read_text(|buf| unsafe { link.GetWorkingDirectory(buf) });
        let flags = link.cast::<IShellLinkDataList>().ok().and_then(|d| unsafe { d.GetFlags() }.ok()).unwrap_or(0);
        Launch::Program { path, arguments, directory, run_as_admin: flags & SLDF_RUNAS_USER.0 as u32 != 0 }
    } else {
        // a Store app's task names no program, only the app (`Family_id!App`) and an argument for it;
        // a jump list only ever starts its own app
        let target = string_property(&store, &PKEY_APP_ID).filter(|t| t.contains('!') && t.eq_ignore_ascii_case(app_id))?;
        let arguments = string_property(&store, &PKEY_ACTIVATION_CONTEXT).unwrap_or_default();
        Launch::StoreApp { app_id: target, arguments }
    };
    Some(Task { id: task_id(&title, &launch), title, launch })
}

/// `--user-data-dir=<folder>` naming a profile that no longer exists: browsers
/// started by test tools leave jump lists for their temporary profiles.
fn names_missing_profile(arguments: &str) -> bool {
    const FLAG: &str = "--user-data-dir=";
    let Some(start) = arguments.find(FLAG) else { return false };
    let rest = &arguments[start + FLAG.len()..];
    let dir = match rest.strip_prefix('"') {
        Some(quoted) => quoted.split('"').next().unwrap_or_default(),
        None => rest.split_whitespace().next().unwrap_or_default(),
    };
    !dir.is_empty() && !Path::new(dir).is_dir()
}

/// `task-<FNV-1a of the title and what it starts>`; never contains the `#`
/// that joins app keys and action ids.
fn task_id(title: &str, launch: &Launch) -> String {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;
    for byte in format!("{title}\0{launch:?}").bytes() {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(0x0100_0000_01b3);
    }
    format!("task-{hash:016x}")
}

/// Calls a `Get…(&mut [u16])` method and returns its text (empty on failure).
fn read_text(get: impl FnOnce(&mut [u16]) -> windows::core::Result<()>) -> String {
    let mut buf = vec![0u16; 4096];
    if get(&mut buf).is_err() {
        return String::new();
    }
    let len = buf.iter().position(|c| *c == 0).unwrap_or(buf.len());
    String::from_utf16_lossy(&buf[..len])
}

fn string_property(store: &IPropertyStore, key: &PROPERTYKEY) -> Option<String> {
    unsafe {
        let mut value: PROPVARIANT = store.GetValue(key).ok()?;
        let text = PropVariantToStringAlloc(&value).ok().map(|raw| {
            let text = raw.to_string().unwrap_or_default();
            CoTaskMemFree(Some(raw.0 as *const _));
            text
        });
        let _ = PropVariantClear(&mut value);
        text.filter(|t| !t.is_empty())
    }
}

fn bool_property(store: &IPropertyStore, key: &PROPERTYKEY) -> bool {
    unsafe {
        let Ok(mut value) = store.GetValue(key) else { return false };
        let set = PropVariantToBooleanWithDefault(&value, false).as_bool();
        let _ = PropVariantClear(&mut value);
        set
    }
}

/// Titles may name a localized resource (`@shell32.dll,-1234`, or Store apps'
/// `@{Package?ms-resource:///Resources/ScientificModeText}`).
fn resolve_indirect(text: &str) -> String {
    if !text.starts_with('@') {
        return text.to_string();
    }
    let source: Vec<u16> = text.encode_utf16().chain([0]).collect();
    let resolved = read_text(|buf| unsafe { SHLoadIndirectString(PCWSTR(source.as_ptr()), buf, None) });
    if resolved.is_empty() {
        text.to_string()
    } else {
        resolved
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use windows::core::HSTRING;
    use windows::Win32::Foundation::VARIANT_TRUE;
    use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
    use windows::Win32::System::Variant::{VT_BOOL, VT_LPWSTR};
    use windows::Win32::UI::Shell::SHStrDupW;

    const CALCULATOR: &str = "Microsoft.WindowsCalculator_8wekyb3d8bbwe!App";

    fn com() {
        let _ = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) };
    }

    /// A program that exists wherever the tests run.
    fn program() -> String {
        std::env::current_exe().unwrap().to_string_lossy().into_owned()
    }

    #[test]
    fn file_names_match_the_ones_windows_writes() {
        // names of real jump-list files, as Windows 11 wrote them for these apps
        assert_eq!(app_id_hash("Chrome"), "5d696d521de238c3");
        assert_eq!(app_id_hash("MSEdge"), "ccba5a5986c77e43");
        assert_eq!(app_id_hash("Microsoft.VisualStudioCode"), "1ced32d74a95c7bc");
        assert_eq!(app_id_hash("Microsoft.WindowsTerminal_8wekyb3d8bbwe!App"), "16f2f0042ddbe0e8");
        assert_eq!(
            app_id_hash(r"{1AC14E77-02E7-4E5D-B744-2EB1AE5198B7}\WindowsPowerShell\v1.0\powershell.exe"),
            "590aee7bdd69b59b"
        );
        assert_eq!(app_id_hash("chrome"), app_id_hash("CHROME"), "case does not matter");
    }

    #[derive(Default)]
    struct TestLink<'a> {
        title: Option<&'a str>,
        path: &'a str,
        arguments: &'a str,
        separator: bool,
        app_id: Option<&'a str>,
        context: Option<&'a str>,
    }

    unsafe fn set_text(store: &IPropertyStore, key: &PROPERTYKEY, text: &str) {
        // a PROPVARIANT's string belongs to the COM allocator, as InitPropVariantFromString makes it
        let mut value = PROPVARIANT::default();
        let inner = &mut *value.Anonymous.Anonymous;
        inner.vt = VT_LPWSTR;
        inner.Anonymous.pwszVal = SHStrDupW(&HSTRING::from(text)).unwrap();
        store.SetValue(key, &value).unwrap();
        PropVariantClear(&mut value).unwrap();
    }

    /// A link saved the way `ICustomDestinationList` saves a task, after its class id.
    fn entry(spec: TestLink<'_>) -> Vec<u8> {
        unsafe {
            let link: IShellLinkW = CoCreateInstance(&ShellLink, None, CLSCTX_INPROC_SERVER).unwrap();
            link.SetPath(&HSTRING::from(spec.path)).unwrap();
            link.SetArguments(&HSTRING::from(spec.arguments)).unwrap();
            let store: IPropertyStore = link.cast().unwrap();
            if let Some(title) = spec.title {
                set_text(&store, &PKEY_TITLE, title);
            }
            if let Some(app_id) = spec.app_id {
                set_text(&store, &PKEY_APP_ID, app_id);
            }
            if let Some(context) = spec.context {
                set_text(&store, &PKEY_ACTIVATION_CONTEXT, context);
            }
            if spec.separator {
                let mut value = PROPVARIANT::default();
                let inner = &mut *value.Anonymous.Anonymous;
                inner.vt = VT_BOOL;
                inner.Anonymous.boolVal = VARIANT_TRUE;
                store.SetValue(&PKEY_SEPARATOR, &value).unwrap();
            }
            store.Commit().unwrap();
            let stream = SHCreateMemStream(None).unwrap();
            link.cast::<IPersistStream>().unwrap().Save(&stream, true).unwrap();
            let mut size = 0u64;
            stream.Seek(0, STREAM_SEEK_CUR, Some(&mut size)).unwrap();
            stream.Seek(0, STREAM_SEEK_SET, None).unwrap();
            let mut saved = vec![0u8; size as usize];
            let mut read = 0u32;
            stream.Read(saved.as_mut_ptr().cast(), size as u32, Some(&mut read)).ok().unwrap();
            saved.truncate(read as usize);
            // the class id in its in-memory (mixed-endian) byte order, then the link
            let mut out = ShellLink.data1.to_le_bytes().to_vec();
            out.extend_from_slice(&ShellLink.data2.to_le_bytes());
            out.extend_from_slice(&ShellLink.data3.to_le_bytes());
            out.extend_from_slice(&ShellLink.data4);
            out.extend(saved);
            out
        }
    }

    fn file(categories: &[Vec<u8>]) -> Vec<u8> {
        let mut out = Vec::new();
        out.extend(2u32.to_le_bytes());
        out.extend((categories.len() as u32).to_le_bytes());
        out.extend(0u32.to_le_bytes());
        for c in categories {
            out.extend(c);
            out.extend(CATEGORY_END.to_le_bytes());
        }
        out
    }

    fn category(kind: u32, name: Option<&str>, entries: &[Vec<u8>]) -> Vec<u8> {
        let mut out = kind.to_le_bytes().to_vec();
        if let Some(name) = name {
            let units: Vec<u16> = name.encode_utf16().collect();
            out.extend((units.len() as u16).to_le_bytes());
            units.iter().for_each(|u| out.extend(u.to_le_bytes()));
        }
        out.extend((entries.len() as u32).to_le_bytes());
        entries.iter().for_each(|e| out.extend(e));
        out
    }

    fn known_category() -> Vec<u8> {
        let mut out = CATEGORY_KNOWN.to_le_bytes().to_vec();
        out.extend(2u32.to_le_bytes());
        out
    }

    fn arguments(task: &Task) -> &str {
        match &task.launch {
            Launch::Program { arguments, .. } | Launch::StoreApp { arguments, .. } => arguments,
        }
    }

    #[test]
    fn reads_the_tasks_and_skips_the_rest() {
        com();
        let exe = program();
        let link = |title, arguments| entry(TestLink { title, path: &exe, arguments, ..Default::default() });
        let bytes = file(&[
            category(CATEGORY_NAMED, Some("Most visited"), &[link(Some("Example"), "https://example.com")]),
            known_category(),
            category(
                CATEGORY_TASKS,
                None,
                &[
                    link(Some("New window"), ""),
                    entry(TestLink { path: &exe, separator: true, ..Default::default() }),
                    link(Some("New Incognito window"), "--incognito"),
                ],
            ),
        ]);
        let tasks = parse(&bytes, "Chrome").unwrap();
        let shown: Vec<_> = tasks.iter().map(|t| (t.title.as_str(), arguments(t))).collect();
        assert_eq!(shown, [("New window", ""), ("New Incognito window", "--incognito")], "no website, no separator");
        let Launch::Program { path, run_as_admin, .. } = &tasks[1].launch else { panic!("{:?}", tasks[1]) };
        assert!(path.eq_ignore_ascii_case(&exe));
        assert!(!run_as_admin);
        assert!(tasks.iter().all(|t| t.id.starts_with("task-") && !t.id.contains('#')));
        assert_ne!(tasks[0].id, tasks[1].id);
        assert_eq!(parse(&bytes, "Chrome").unwrap()[1].id, tasks[1].id, "ids are stable");
    }

    #[test]
    fn store_app_tasks_start_their_own_app_with_its_argument() {
        com();
        let missing = r"C:\Users\Public\Desktop\Microsoft.WindowsCalculator_8wekyb3d8bbwe!App";
        let task = |title, app_id, context| entry(TestLink { title: Some(title), path: missing, app_id, context, ..Default::default() });
        let bytes = file(&[
            known_category(),
            category(
                CATEGORY_TASKS,
                None,
                &[
                    task("Scientific", Some(CALCULATOR), Some("1")),
                    task("Standard", Some(CALCULATOR), None),
                    task("Someone else's", Some("Contoso.Other_1234567890abc!App"), Some("x")),
                    task("Nothing to start", None, Some("x")),
                ],
            ),
        ]);
        let tasks = parse(&bytes, CALCULATOR).unwrap();
        assert_eq!(
            tasks.iter().map(|t| (t.title.as_str(), &t.launch)).collect::<Vec<_>>(),
            [
                ("Scientific", &Launch::StoreApp { app_id: CALCULATOR.into(), arguments: "1".into() }),
                ("Standard", &Launch::StoreApp { app_id: CALCULATOR.into(), arguments: String::new() }),
            ]
        );
        assert!(parse(&bytes, "Chrome").unwrap().is_empty(), "a desktop app's jump list starts no Store app");
    }

    #[test]
    fn tasks_for_a_deleted_profile_or_program_are_dropped() {
        com();
        let exe = program();
        let profile = tempfile::tempdir().unwrap();
        let live = format!("--user-data-dir=\"{}\" --inprivate", profile.path().display());
        let link = |title, path: &str, arguments: &str| entry(TestLink { title: Some(title), path, arguments, ..Default::default() });
        let bytes = file(&[category(
            CATEGORY_TASKS,
            None,
            &[
                link("Live profile", &exe, &live),
                link("Deleted profile", &exe, r#"--user-data-dir="C:\Temp\scoped_dir_gone_123" --inprivate"#),
                link("Deleted unquoted", &exe, r"--user-data-dir=C:\Temp\scoped_dir_gone_456 --inprivate"),
                link("Uninstalled", r"C:\Program Files\Gone\gone.exe", ""),
            ],
        )]);
        let titles: Vec<_> = parse(&bytes, "MSEdge").unwrap().into_iter().map(|t| t.title).collect();
        assert_eq!(titles, ["Live profile"]);
    }

    #[test]
    fn damaged_files_are_rejected_without_panicking() {
        com();
        let exe = program();
        let good = file(&[category(CATEGORY_TASKS, None, &[entry(TestLink { title: Some("New window"), path: &exe, ..Default::default() })])]);
        assert_eq!(parse(&good, "Chrome").unwrap().len(), 1);
        for len in [0, 5, 12, 20, 40, good.len() - 4, good.len() - 1] {
            assert!(parse(&good[..len], "Chrome").is_err(), "truncated to {len}");
        }
        let mut foreign = good.clone();
        foreign[20] ^= 0xff; // the entry's class id
        assert!(parse(&foreign, "Chrome").unwrap_err().contains("not a shell link"));
        let mut odd = good.clone();
        odd[12] = 7; // category type
        assert!(parse(&odd, "Chrome").is_err());
    }

    /// Prints the tasks of the installed apps: `cargo test show_installed_jump_lists -- --ignored --nocapture`.
    #[test]
    #[ignore]
    fn show_installed_jump_lists() {
        com();
        for app in crate::platform::windows::apps::discover() {
            if !app.actions.is_empty() {
                println!("{} ({})", app.name, app.key);
                for task in tasks(&app.key) {
                    println!("    {:?} -> {:?}", task.title, task.launch);
                }
            }
        }
    }
}
