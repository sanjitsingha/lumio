//! Text cursor (caret) position on Windows, in physical screen pixels.
//!
//! 1. The system caret of the foreground window's thread
//!    (`GetGUIThreadInfo`), which edit controls, Notepad, Office and most
//!    native apps create.
//! 2. Otherwise the MSAA caret object (`OBJID_CARET`) of the focused window,
//!    which browsers and many UI frameworks expose for screen magnifiers even
//!    without a system caret.
//!
//! Whether the focused field has text selected comes from UI Automation,
//! whose text pattern covers edit controls, Office, WinUI and Chromium.

use std::mem::ManuallyDrop;

use windows::core::Interface;
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::System::Com::{CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED};
use windows::Win32::System::Variant::{VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VT_I4};
use windows::Win32::UI::Accessibility::{
    AccessibleObjectFromWindow, CUIAutomation8, IAccessible, IUIAutomation, IUIAutomation2, IUIAutomationTextPattern,
    TextPatternRangeEndpoint_End, TextPatternRangeEndpoint_Start, UIA_TextPatternId,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetGUIThreadInfo, GetWindowThreadProcessId, GUITHREADINFO, OBJID_CARET,
};

use crate::capslock::geometry::Rect;

pub fn support() -> Result<&'static str, String> {
    Ok("System caret (Win32, MSAA)")
}

/// How long UI Automation may wait for an app: a hung one must not stall
/// the indicator's animation.
const UIA_TIMEOUT_MS: u32 = 200;

pub struct CaretTracker {
    /// Focus moves (`ime::focus_moves`) when the caret was located.
    located_at: u64,
    /// Created when first needed (`Some(None)`: unavailable).
    automation: Option<Option<IUIAutomation>>,
}

impl CaretTracker {
    pub fn new() -> Result<Self, String> {
        // MSAA needs COM on this thread; S_FALSE (already initialised) is fine
        let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        Ok(CaretTracker { located_at: 0, automation: None })
    }

    /// Whether the focused field has text selected: it then shows no text
    /// cursor. Fields UI Automation cannot read count as without one.
    pub fn has_selection(&mut self) -> bool {
        let Some(automation) = self.automation.get_or_insert_with(ui_automation).as_ref() else { return false };
        unsafe { selection_in_focus(automation) }.unwrap_or(false)
    }

    pub fn locate(&mut self) -> Option<Rect> {
        self.located_at = super::ime::focus_moves();
        current_caret()
    }

    /// Both sources are cheap, so following simply reads the caret again.
    /// `None` once focus has moved on: the next field's caret is another
    /// field's, found by `locate` when that field is reported.
    pub fn refresh(&mut self) -> Option<Rect> {
        if super::ime::focus_moves() != self.located_at {
            return None;
        }
        current_caret()
    }

    pub fn release(&mut self) {}

    /// The system caret needs no cross-check against its field.
    pub fn focused_field(&self) -> Option<Rect> {
        None
    }

    /// Input method positions are not watched on Windows.
    pub fn map_input_method(&mut self, _rect: Rect, _relative: bool, _fresh: bool) -> Option<Rect> {
        None
    }

    /// Input method positions are not watched on Windows.
    pub fn take_window_switch(&mut self) -> bool {
        false
    }
}

fn ui_automation() -> Option<IUIAutomation> {
    let automation: IUIAutomation = unsafe { CoCreateInstance(&CUIAutomation8, None, CLSCTX_INPROC_SERVER) }
        .map_err(|e| log::info!("UI Automation is unavailable, so selections are not detected: {e}"))
        .ok()?;
    if let Ok(timeouts) = automation.cast::<IUIAutomation2>() {
        let _ = unsafe { timeouts.SetConnectionTimeout(UIA_TIMEOUT_MS) };
        let _ = unsafe { timeouts.SetTransactionTimeout(UIA_TIMEOUT_MS) };
    }
    Some(automation)
}

/// Whether the focused element's text pattern has a selection that is more
/// than a caret.
unsafe fn selection_in_focus(automation: &IUIAutomation) -> windows::core::Result<bool> {
    let focused = automation.GetFocusedElement()?;
    let text: IUIAutomationTextPattern = focused.GetCurrentPatternAs(UIA_TextPatternId)?;
    let ranges = text.GetSelection()?;
    for i in 0..ranges.Length()? {
        let range = ranges.GetElement(i)?;
        if range.CompareEndpoints(TextPatternRangeEndpoint_Start, &range, TextPatternRangeEndpoint_End)? != 0 {
            return Ok(true);
        }
    }
    Ok(false)
}

fn current_caret() -> Option<Rect> {
    let (info, foreground) = gui_thread_info()?;
    system_caret(&info).or_else(|| {
        let focus = if info.hwndFocus.is_invalid() { foreground } else { info.hwndFocus };
        msaa_caret(focus)
    })
}

fn gui_thread_info() -> Option<(GUITHREADINFO, HWND)> {
    let foreground = unsafe { GetForegroundWindow() };
    if foreground.is_invalid() {
        return None;
    }
    let thread = unsafe { GetWindowThreadProcessId(foreground, None) };
    let mut info = GUITHREADINFO { cbSize: std::mem::size_of::<GUITHREADINFO>() as u32, ..Default::default() };
    unsafe { GetGUIThreadInfo(thread, &mut info) }.ok()?;
    Some((info, foreground))
}

fn system_caret(info: &GUITHREADINFO) -> Option<Rect> {
    let rc = info.rcCaret;
    if info.hwndCaret.is_invalid() || rc.bottom <= rc.top {
        return None;
    }
    let mut origin = POINT { x: rc.left, y: rc.top };
    if !unsafe { ClientToScreen(info.hwndCaret, &mut origin) }.as_bool() {
        return None;
    }
    Some(Rect { x: origin.x, y: origin.y, width: (rc.right - rc.left).max(1), height: rc.bottom - rc.top })
}

fn msaa_caret(window: HWND) -> Option<Rect> {
    let mut raw = std::ptr::null_mut();
    unsafe { AccessibleObjectFromWindow(window, OBJID_CARET.0 as u32, &IAccessible::IID, &mut raw) }.ok()?;
    if raw.is_null() {
        return None;
    }
    let caret = unsafe { IAccessible::from_raw(raw) };
    // a field that loses focus only hides its caret, which keeps answering
    // with its last place (Chromium's address bar after a click into the page)
    if unsafe { caret.get_accState(&child_self()) }.is_ok_and(|state| invisible(&state)) {
        return None;
    }
    let (mut x, mut y, mut width, mut height) = (0, 0, 0, 0);
    unsafe { caret.accLocation(&mut x, &mut y, &mut width, &mut height, &child_self()) }.ok()?;
    // windows without a caret answer with an empty rectangle
    (height > 0 && (x, y) != (0, 0)).then_some(Rect { x, y, width: width.max(1), height })
}

/// `STATE_SYSTEM_INVISIBLE` from oleacc.h.
const STATE_SYSTEM_INVISIBLE: i32 = 0x8000;

fn invisible(state: &VARIANT) -> bool {
    let v = unsafe { &state.Anonymous.Anonymous };
    v.vt == VT_I4 && unsafe { v.Anonymous.lVal } & STATE_SYSTEM_INVISIBLE != 0
}

/// `CHILDID_SELF`: the caret object itself rather than one of its children.
fn child_self() -> VARIANT {
    VARIANT {
        Anonymous: VARIANT_0 {
            Anonymous: ManuallyDrop::new(VARIANT_0_0 {
                vt: VT_I4,
                Anonymous: VARIANT_0_0_0 { lVal: 0 },
                ..Default::default()
            }),
        },
    }
}
