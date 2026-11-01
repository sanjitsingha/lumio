//! Caps Lock detection on Windows: a low-level keyboard hook wakes a worker
//! thread whenever the Caps Lock key is released, and the worker reads the
//! toggle state with `GetKeyState`. No polling.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::Arc;
use std::time::Duration;

use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyState, VK_CAPITAL};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, DispatchMessageW, GetMessageW, PostThreadMessageW, SetWindowsHookExW, TranslateMessage,
    UnhookWindowsHookEx, HC_ACTION, KBDLLHOOKSTRUCT, MSG, WH_KEYBOARD_LL, WM_APP, WM_KEYUP, WM_SYSKEYUP,
};

use crate::platform::{DetectionSupport, StateCallback};

const WM_CAPS_CHANGED: u32 = WM_APP + 0x41;
static WORKER_THREAD: AtomicU32 = AtomicU32::new(0);

pub fn support() -> DetectionSupport {
    DetectionSupport {
        available: true,
        method: "Keyboard hook + GetKeyState",
        detail: "Reacts to the Caps Lock key without polling.".into(),
    }
}

fn caps_on() -> bool {
    (unsafe { GetKeyState(VK_CAPITAL.0 as i32) } & 1) != 0
}

unsafe extern "system" fn hook_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code == HC_ACTION as i32 && (wparam.0 as u32 == WM_KEYUP || wparam.0 as u32 == WM_SYSKEYUP) {
        // SAFETY: for WH_KEYBOARD_LL with HC_ACTION, lparam points to a KBDLLHOOKSTRUCT.
        let info = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        if info.vkCode == VK_CAPITAL.0 as u32 {
            let thread = WORKER_THREAD.load(Ordering::Relaxed);
            if thread != 0 {
                let _ = unsafe { PostThreadMessageW(thread, WM_CAPS_CHANGED, WPARAM(0), LPARAM(0)) };
            }
        }
    }
    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}

pub fn start_monitor(on_change: StateCallback, _active: Arc<AtomicBool>) -> Result<&'static str, String> {
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<(), String>>();
    std::thread::Builder::new()
        .name("lumio-capslock-hook".into())
        .spawn(move || {
            WORKER_THREAD.store(unsafe { GetCurrentThreadId() }, Ordering::SeqCst);
            let module = unsafe { GetModuleHandleW(None) }.ok().map(|m| m.into());
            let hook = match unsafe { SetWindowsHookExW(WH_KEYBOARD_LL, Some(hook_proc), module, 0) } {
                Ok(h) => h,
                Err(e) => {
                    let _ = ready_tx.send(Err(format!("SetWindowsHookExW failed: {e}")));
                    return;
                }
            };
            let mut last = caps_on();
            on_change(last);
            let _ = ready_tx.send(Ok(()));
            let mut msg = MSG::default();
            // GetMessageW also pumps the hook; it returns 0 on WM_QUIT and -1 on error.
            while unsafe { GetMessageW(&mut msg, None, 0, 0) }.0 > 0 {
                if msg.message == WM_CAPS_CHANGED {
                    // The toggle bit flips when the key goes down; the key-up we were
                    // woken by guarantees it has been processed. Re-check shortly in
                    // case the keyboard state lags behind the hook.
                    for delay in [0u64, 40] {
                        std::thread::sleep(Duration::from_millis(delay));
                        let now = caps_on();
                        if now != last {
                            last = now;
                            on_change(now);
                            break;
                        }
                    }
                    continue;
                }
                unsafe {
                    let _ = TranslateMessage(&msg);
                    DispatchMessageW(&msg);
                }
            }
            unsafe {
                let _ = UnhookWindowsHookEx(hook);
            }
        })
        .map_err(|e| e.to_string())?;
    ready_rx.recv_timeout(Duration::from_secs(3)).map_err(|_| "keyboard hook thread did not start".to_string())??;
    Ok("Keyboard hook + GetKeyState")
}
