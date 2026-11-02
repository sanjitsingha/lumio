//! Application discovery via the shell's AppsFolder (`shell:AppsFolder`),
//! the same list the Start menu shows: classic desktop apps and Store/UWP
//! apps such as Calculator and Settings.

use std::sync::Arc;

use windows::core::{Interface, PWSTR};
use windows::Win32::Foundation::{HANDLE, SIZE};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, DeleteDC, DeleteObject, GetDIBits, GetObjectW, BITMAP, BITMAPINFO, BITMAPINFOHEADER, BI_RGB,
    DIB_RGB_COLORS, HBITMAP, HGDIOBJ,
};
use windows::Win32::System::Com::{CoInitializeEx, CoTaskMemFree, CoUninitialize, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::Shell::{
    BHID_EnumItems, FOLDERID_AppsFolder, IEnumShellItems, IShellItem, IShellItemImageFactory, SHGetKnownFolderItem,
    KF_FLAG_DEFAULT, SIGDN, SIGDN_NORMALDISPLAY, SIGDN_PARENTRELATIVEPARSING, SIIGBF_BIGGERSIZEOK, SIIGBF_ICONONLY,
};

use crate::search::apps::{AppIcon, DiscoveredApp};

/// Non-app entries (help files, web links, readmes) that installers add to the Start menu.
const SKIP_EXTENSIONS: &[&str] = &[".url", ".chm", ".txt", ".pdf", ".htm", ".html", ".rtf", ".hlp", ".ini"];

pub fn discover() -> Vec<DiscoveredApp> {
    let initialized = unsafe { CoInitializeEx(None, COINIT_APARTMENTTHREADED) }.is_ok();
    let apps = match enumerate() {
        Ok(apps) => apps,
        Err(e) => {
            log::error!("could not enumerate installed applications: {e}");
            Vec::new()
        }
    };
    if initialized {
        unsafe { CoUninitialize() };
    }
    apps
}

fn enumerate() -> windows::core::Result<Vec<DiscoveredApp>> {
    let folder: IShellItem = unsafe { SHGetKnownFolderItem(&FOLDERID_AppsFolder, KF_FLAG_DEFAULT, Some(HANDLE::default()))? };
    let items: IEnumShellItems = unsafe { folder.BindToHandler(None, &BHID_EnumItems)? };
    let mut apps = Vec::new();
    loop {
        let mut batch: [Option<IShellItem>; 1] = [None];
        let mut fetched = 0u32;
        let next = unsafe { items.Next(&mut batch, Some(&mut fetched)) };
        if next.is_err() || fetched == 0 {
            break;
        }
        let Some(item) = batch[0].take() else { break };
        let (Some(name), Some(parsing)) = (display_name(&item, SIGDN_NORMALDISPLAY), display_name(&item, SIGDN_PARENTRELATIVEPARSING)) else {
            continue;
        };
        let lower = parsing.to_lowercase();
        if name.is_empty() || SKIP_EXTENSIONS.iter().any(|ext| lower.ends_with(ext)) {
            continue;
        }
        let icon = icon_png(&item).map(|png| AppIcon::Png(Arc::new(png))).unwrap_or(AppIcon::None);
        let keywords = executable_keyword(&parsing).into_iter().collect();
        apps.push(DiscoveredApp {
            key: parsing.clone(),
            name,
            description: String::new(),
            keywords,
            icon,
            target: parsing,
            // jump-list tasks are private to each app; Windows has no API to read them
            actions: Vec::new(),
        });
    }
    Ok(apps)
}

fn display_name(item: &IShellItem, kind: SIGDN) -> Option<String> {
    let raw: PWSTR = unsafe { item.GetDisplayName(kind).ok()? };
    let text = unsafe { raw.to_string().ok() };
    unsafe { CoTaskMemFree(Some(raw.0 as *const _)) };
    text
}

/// `C:\...\Code.exe` -> `code`, so typing the program name finds the app.
fn executable_keyword(parsing: &str) -> Option<String> {
    let file = parsing.rsplit(['\\', '/']).next()?;
    let stem = file.strip_suffix(".exe").or_else(|| file.strip_suffix(".EXE"))?;
    (!stem.is_empty()).then(|| stem.to_lowercase())
}

/// Renders the shell icon at 64 px and encodes it as PNG.
fn icon_png(item: &IShellItem) -> Option<Vec<u8>> {
    let factory: IShellItemImageFactory = item.cast().ok()?;
    let bitmap = unsafe { factory.GetImage(SIZE { cx: 64, cy: 64 }, SIIGBF_ICONONLY | SIIGBF_BIGGERSIZEOK).ok()? };
    let pixels = bitmap_rgba(bitmap);
    unsafe {
        let _ = DeleteObject(HGDIOBJ(bitmap.0));
    }
    let (width, height, rgba) = pixels?;
    encode_png(width, height, &rgba)
}

/// Reads a 32-bit (premultiplied BGRA) bitmap as straight RGBA.
fn bitmap_rgba(bitmap: HBITMAP) -> Option<(u32, u32, Vec<u8>)> {
    let mut info = BITMAP::default();
    let got = unsafe { GetObjectW(HGDIOBJ(bitmap.0), std::mem::size_of::<BITMAP>() as i32, Some((&mut info as *mut BITMAP).cast())) };
    if got == 0 || info.bmWidth <= 0 || info.bmHeight == 0 {
        return None;
    }
    let (w, h) = (info.bmWidth as u32, info.bmHeight.unsigned_abs());
    let mut header = BITMAPINFO {
        bmiHeader: BITMAPINFOHEADER {
            biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
            biWidth: w as i32,
            biHeight: -(h as i32), // top-down rows
            biPlanes: 1,
            biBitCount: 32,
            biCompression: BI_RGB.0,
            ..Default::default()
        },
        ..Default::default()
    };
    let mut bgra = vec![0u8; (w * h * 4) as usize];
    let dc = unsafe { CreateCompatibleDC(None) };
    let lines = unsafe { GetDIBits(dc, bitmap, 0, h, Some(bgra.as_mut_ptr().cast()), &mut header, DIB_RGB_COLORS) };
    unsafe {
        let _ = DeleteDC(dc);
    }
    if lines == 0 {
        return None;
    }
    let has_alpha = bgra.chunks_exact(4).any(|p| p[3] != 0);
    let mut rgba = Vec::with_capacity(bgra.len());
    for p in bgra.chunks_exact(4) {
        let (b, g, r, a) = (p[0], p[1], p[2], if has_alpha { p[3] } else { 255 });
        let unpremultiply = |c: u8| if a == 0 { 0 } else { ((c as u32 * 255 + a as u32 / 2) / a as u32).min(255) as u8 };
        rgba.extend_from_slice(&[unpremultiply(r), unpremultiply(g), unpremultiply(b), a]);
    }
    Some((w, h, rgba))
}

fn encode_png(width: u32, height: u32, rgba: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut out, width, height);
        encoder.set_color(png::ColorType::Rgba);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().ok()?;
        writer.write_image_data(rgba).ok()?;
    }
    Some(out)
}
