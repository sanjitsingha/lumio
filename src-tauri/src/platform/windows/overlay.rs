//! Caps Lock overlay on Windows: a per-pixel-alpha layered popup window.
//!
//! `WS_EX_LAYERED | WS_EX_TRANSPARENT` makes it click-through,
//! `WS_EX_NOACTIVATE` keeps it from taking focus, `WS_EX_TOOLWINDOW` keeps it
//! off the taskbar and Alt+Tab, and `WS_EX_TOPMOST` keeps it above normal
//! windows. The window is owned by a dedicated thread that pumps its messages.

use std::sync::mpsc::{self, Receiver, Sender};

use windows::core::w;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC, ReleaseDC, SelectObject, AC_SRC_ALPHA,
    AC_SRC_OVER, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, BLENDFUNCTION, DIB_RGB_COLORS, HBITMAP, HDC, HGDIOBJ,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, PostMessageW, RegisterClassW, SetWindowPos,
    ShowWindow, TranslateMessage, UpdateLayeredWindow, HWND_TOPMOST, MSG, SWP_NOACTIVATE, SWP_NOMOVE, SWP_NOSIZE,
    SW_HIDE, SW_SHOWNOACTIVATE, ULW_ALPHA, WM_APP, WNDCLASSW, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW,
    WS_EX_TOPMOST, WS_EX_TRANSPARENT, WS_POPUP,
};

use crate::capslock::render::Frame;

const WM_OVERLAY_UPDATE: u32 = WM_APP + 0x42;

pub fn support() -> Result<&'static str, String> {
    Ok("Layered window (Win32)")
}

pub fn osd_available() -> bool {
    false
}

pub fn show_osd(_on: bool) -> Result<(), String> {
    Err("Windows uses the cursor overlay".into())
}

enum Cmd {
    Present { frame: Option<Frame>, x: i32, y: i32, opacity: f32 },
    Hide,
}

pub struct Overlay {
    tx: Sender<Cmd>,
    hwnd: isize,
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

/// Holds the current bitmap selected into a memory DC.
struct Surface {
    dc: HDC,
    bitmap: HBITMAP,
    previous: HGDIOBJ,
    width: i32,
    height: i32,
}

impl Surface {
    fn new(frame: &Frame) -> Option<Surface> {
        let (w, h) = (frame.width as i32, frame.height as i32);
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: std::mem::size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: w,
                biHeight: -h,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        unsafe {
            let dc = CreateCompatibleDC(None);
            let bitmap = CreateDIBSection(Some(dc), &info, DIB_RGB_COLORS, &mut bits, None, 0).ok()?;
            // premultiplied RGBA -> premultiplied BGRA
            let dst = std::slice::from_raw_parts_mut(bits as *mut u8, (w * h * 4) as usize);
            for (d, s) in dst.chunks_exact_mut(4).zip(frame.rgba.chunks_exact(4)) {
                d.copy_from_slice(&[s[2], s[1], s[0], s[3]]);
            }
            let previous = SelectObject(dc, HGDIOBJ(bitmap.0));
            Some(Surface { dc, bitmap, previous, width: w, height: h })
        }
    }
}

impl Drop for Surface {
    fn drop(&mut self) {
        unsafe {
            SelectObject(self.dc, self.previous);
            let _ = DeleteObject(HGDIOBJ(self.bitmap.0));
            let _ = DeleteDC(self.dc);
        }
    }
}

fn run(hwnd: HWND, rx: Receiver<Cmd>) {
    let mut surface: Option<Surface> = None;
    let mut visible = false;
    let mut msg = MSG::default();
    while unsafe { GetMessageW(&mut msg, None, 0, 0) }.0 > 0 {
        if msg.message != WM_OVERLAY_UPDATE {
            unsafe {
                let _ = TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }
            continue;
        }
        // apply only the newest command; older positions are already stale
        let Some(cmd) = rx.try_iter().last() else { continue };
        match cmd {
            Cmd::Hide => {
                if visible {
                    unsafe {
                        let _ = ShowWindow(hwnd, SW_HIDE);
                    }
                    visible = false;
                }
            }
            Cmd::Present { frame, x, y, opacity } => {
                if let Some(frame) = frame {
                    surface = Surface::new(&frame);
                }
                let Some(s) = surface.as_ref() else { continue };
                let blend = BLENDFUNCTION {
                    BlendOp: AC_SRC_OVER as u8,
                    BlendFlags: 0,
                    SourceConstantAlpha: (opacity.clamp(0.0, 1.0) * 255.0).round() as u8,
                    AlphaFormat: AC_SRC_ALPHA as u8,
                };
                unsafe {
                    let screen = GetDC(None);
                    let _ = UpdateLayeredWindow(
                        hwnd,
                        Some(screen),
                        Some(&POINT { x, y }),
                        Some(&SIZE { cx: s.width, cy: s.height }),
                        Some(s.dc),
                        Some(&POINT { x: 0, y: 0 }),
                        COLORREF(0),
                        Some(&blend),
                        ULW_ALPHA,
                    );
                    ReleaseDC(None, screen);
                    if !visible {
                        let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
                        visible = true;
                    }
                    let _ = SetWindowPos(hwnd, Some(HWND_TOPMOST), 0, 0, 0, 0, SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE);
                }
            }
        }
    }
}

impl Overlay {
    pub fn new() -> Result<Self, String> {
        let (tx, rx) = mpsc::channel::<Cmd>();
        let (hwnd_tx, hwnd_rx) = mpsc::channel::<Result<isize, String>>();
        std::thread::Builder::new()
            .name("lumio-capslock-overlay".into())
            .spawn(move || unsafe {
                let instance = match GetModuleHandleW(None) {
                    Ok(m) => m,
                    Err(e) => {
                        let _ = hwnd_tx.send(Err(e.to_string()));
                        return;
                    }
                };
                let class = WNDCLASSW {
                    lpfnWndProc: Some(wndproc),
                    hInstance: instance.into(),
                    lpszClassName: w!("LumioCapsLockOverlay"),
                    ..Default::default()
                };
                RegisterClassW(&class);
                let created = CreateWindowExW(
                    WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOPMOST | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                    w!("LumioCapsLockOverlay"),
                    w!("Lumio Caps Lock"),
                    WS_POPUP,
                    0,
                    0,
                    1,
                    1,
                    None,
                    None,
                    Some(instance.into()),
                    None,
                );
                match created {
                    Ok(hwnd) => {
                        let _ = hwnd_tx.send(Ok(hwnd.0 as isize));
                        run(hwnd, rx);
                    }
                    Err(e) => {
                        let _ = hwnd_tx.send(Err(format!("CreateWindowExW failed: {e}")));
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        let hwnd = hwnd_rx.recv().map_err(|e| e.to_string())??;
        Ok(Self { tx, hwnd })
    }

    fn wake(&self) {
        unsafe {
            let _ = PostMessageW(Some(HWND(self.hwnd as *mut _)), WM_OVERLAY_UPDATE, WPARAM(0), LPARAM(0));
        }
    }

    /// `x`/`y`: canvas top-left in physical screen pixels.
    pub fn present(&mut self, frame: Option<&Frame>, x: i32, y: i32, opacity: f32) {
        let _ = self.tx.send(Cmd::Present { frame: frame.cloned(), x, y, opacity });
        self.wake();
    }

    pub fn hide(&mut self) {
        let _ = self.tx.send(Cmd::Hide);
        self.wake();
    }
}
