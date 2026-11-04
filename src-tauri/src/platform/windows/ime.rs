//! Keyboard focus on Windows, reported the way IBus reports it on Linux: a
//! field counts as focused while the foreground window has a text cursor
//! (`caret.rs`, which also gives its position, so none is reported here).
//!
//! WinEvents say when the foreground window, the focused element or a caret
//! changes; the caret is looked up once the change has settled. Nothing is
//! read while Caps Lock is off.

use std::cell::Cell;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{mpsc, Arc};
use std::time::Duration;

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Accessibility::{SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, KillTimer, PeekMessageW, PostThreadMessageW, SetTimer, TranslateMessage,
    EVENT_OBJECT_FOCUS, EVENT_OBJECT_HIDE, EVENT_OBJECT_SHOW, EVENT_SYSTEM_FOREGROUND, MSG, OBJID_CARET, PM_NOREMOVE,
    WINEVENT_OUTOFCONTEXT, WM_QUIT, WM_TIMER,
};

use super::capslock::caps_on;
use super::caret::CaretTracker;
use crate::capslock::InputMethodEvent;

/// When to look for the text cursor after a change: apps create their caret
/// as they take focus, and slow ones get a second look before the field
/// counts as left.
const LOOKS: [Duration; 2] = [Duration::from_millis(50), Duration::from_millis(250)];

thread_local! {
    /// The element that last got focus (window, object, child).
    static FOCUSED: Cell<(isize, i32, i32)> = const { Cell::new((0, 0, 0)) };
    /// The timer of the pending look, and which look it is.
    static TIMER: Cell<usize> = const { Cell::new(0) };
    static LOOK: Cell<usize> = const { Cell::new(0) };
}

/// Counts moves of keyboard focus to another element as they happen, before
/// the new field has its caret (and long before it is reported).
static FOCUS_MOVES: AtomicU64 = AtomicU64::new(0);

pub(super) fn focus_moves() -> u64 {
    FOCUS_MOVES.load(Ordering::SeqCst)
}

pub struct ImeWatch {
    thread: u32,
    alive: Arc<AtomicBool>,
}

impl ImeWatch {
    /// Calls `on_event` (on its own thread) when a text field gets or loses
    /// keyboard focus.
    pub fn start(on_event: impl Fn(InputMethodEvent) + Send + 'static) -> Result<Self, String> {
        let alive = Arc::new(AtomicBool::new(true));
        let running = alive.clone();
        let (ready_tx, ready_rx) = mpsc::channel::<Result<u32, String>>();
        std::thread::Builder::new()
            .name("lummo-focus".into())
            .spawn(move || {
                let ranges = [
                    (EVENT_SYSTEM_FOREGROUND, EVENT_SYSTEM_FOREGROUND),
                    (EVENT_OBJECT_SHOW, EVENT_OBJECT_HIDE),
                    (EVENT_OBJECT_FOCUS, EVENT_OBJECT_FOCUS),
                ];
                let hooks = ranges.map(|(min, max)| unsafe {
                    SetWinEventHook(min, max, None, Some(on_win_event), 0, 0, WINEVENT_OUTOFCONTEXT)
                });
                if hooks.iter().all(|h| !h.is_invalid()) {
                    // creates the message queue before anyone posts to it
                    let mut msg = MSG::default();
                    let _ = unsafe { PeekMessageW(&mut msg, None, 0, 0, PM_NOREMOVE) };
                    let _ = ready_tx.send(Ok(unsafe { GetCurrentThreadId() }));
                    watch(&on_event);
                } else {
                    let _ = ready_tx.send(Err("SetWinEventHook failed".into()));
                }
                for hook in hooks.into_iter().filter(|h| !h.is_invalid()) {
                    let _ = unsafe { UnhookWinEvent(hook) };
                }
                running.store(false, Ordering::SeqCst);
            })
            .map_err(|e| e.to_string())?;
        let thread = ready_rx
            .recv_timeout(Duration::from_secs(3))
            .map_err(|_| "focus watch thread did not start".to_string())??;
        Ok(Self { thread, alive })
    }

    pub fn is_alive(&self) -> bool {
        self.alive.load(Ordering::SeqCst)
    }
}

impl Drop for ImeWatch {
    fn drop(&mut self) {
        let _ = unsafe { PostThreadMessageW(self.thread, WM_QUIT, WPARAM(0), LPARAM(0)) };
    }
}

unsafe extern "system" fn on_win_event(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    object: i32,
    child: i32,
    _thread: u32,
    _time: u32,
) {
    match event {
        // windows and menus come and go all the time; only the caret matters
        EVENT_OBJECT_SHOW | EVENT_OBJECT_HIDE if object != OBJID_CARET.0 => return,
        EVENT_OBJECT_SHOW | EVENT_OBJECT_HIDE => {}
        _ => {
            let element = (hwnd.0 as isize, object, child);
            if FOCUSED.replace(element) != element {
                FOCUS_MOVES.fetch_add(1, Ordering::SeqCst);
            }
        }
    }
    LOOK.set(0);
    look_in(LOOKS[0]);
}

/// Without a window, `SetTimer` replaces the timer whose id it is given.
fn look_in(delay: Duration) {
    TIMER.set(unsafe { SetTimer(None, TIMER.get(), delay.as_millis() as u32, None) });
}

fn watch(on_event: &impl Fn(InputMethodEvent)) {
    let Ok(mut caret) = CaretTracker::new() else { return };
    // the field last reported as focused
    let mut focused: Option<String> = None;
    let mut msg = MSG::default();
    // GetMessageW also delivers the WinEvents; it returns 0 on WM_QUIT and -1 on error
    while unsafe { GetMessageW(&mut msg, None, 0, 0) }.0 > 0 {
        if msg.message == WM_TIMER && msg.wParam.0 == TIMER.get() {
            let n = LOOK.get();
            let last = n + 1 == LOOKS.len();
            if look(&mut caret, &mut focused, last, on_event) {
                let _ = unsafe { KillTimer(None, TIMER.replace(0)) };
            } else {
                LOOK.set(n + 1);
                look_in(LOOKS[n + 1] - LOOKS[n]);
            }
            continue;
        }
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// Reports where focus went, if that changed. `false` while it is too early
/// to tell that no text field has focus.
fn look(caret: &mut CaretTracker, focused: &mut Option<String>, last: bool, on_event: &impl Fn(InputMethodEvent)) -> bool {
    // also when no field was reported: the indicator may have found one itself
    let left = |field: Option<String>| on_event(InputMethodEvent::Focus { context: field.unwrap_or_default(), focused: false });
    // nothing is shown while Caps Lock is off: forget the field rather than read it
    if !caps_on() {
        if focused.is_some() {
            left(focused.take());
        }
        return true;
    }
    if caret.locate().is_none() {
        if last {
            left(focused.take());
        }
        return last;
    }
    let (window, object, child) = FOCUSED.get();
    let field = format!("{window:x}:{object}:{child}");
    if focused.as_ref() != Some(&field) {
        if focused.is_some() {
            left(focused.take());
        }
        on_event(InputMethodEvent::Focus { context: field.clone(), focused: true });
        *focused = Some(field);
    }
    true
}
