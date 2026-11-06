//! Text cursor (caret) position on Windows, in physical screen pixels.
//!
//! 1. The system caret of the foreground window's thread
//!    (`GetGUIThreadInfo`), which edit controls, Notepad, Office and most
//!    native apps create.
//! 2. Otherwise the MSAA caret object (`OBJID_CARET`) of the focused window,
//!    which browsers and many UI frameworks expose for screen magnifiers even
//!    without a system caret.
//! 3. Otherwise the caret that UI Automation reports for the focused element
//!    (`TextPattern2` caret range, or the collapsed selection of a text
//!    pattern), as Narrator and Magnifier read it: WinUI and UWP apps
//!    (Settings, the Start menu's search) and Windows Terminal create no
//!    caret of either kind. It comes last because it is less exact where the
//!    others answer: Chromium reports the end of a placeholder as the caret
//!    of an empty field. Text that cannot be edited has no text cursor, even
//!    where UI Automation reports one (a web page you clicked into).
//!
//! Either way the caret must belong to the foreground window and lie inside
//! it: a text field of a window behind it (Notepad next to the browser you
//! type in) never gets the indicator.
//!
//! Whether the focused field has text selected comes from UI Automation,
//! whose text pattern covers edit controls, Office, WinUI and Chromium.
//!
//! Fields that take only numbers get no indicator, as Caps Lock does nothing
//! there: edit controls with `ES_NUMBER`, and what UI Automation calls a
//! spinner (a browser's `<input type="number">`, spin boxes, WinUI's
//! `NumberBox`).

use std::mem::ManuallyDrop;

use windows::core::{Interface, BOOL};
use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::ClientToScreen;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_MULTITHREADED, SAFEARRAY,
};
use windows::Win32::System::Ole::{SafeArrayAccessData, SafeArrayDestroy, SafeArrayGetUBound, SafeArrayUnaccessData};
use windows::Win32::System::Variant::{VariantClear, VARIANT, VARIANT_0, VARIANT_0_0, VARIANT_0_0_0, VT_BOOL, VT_I4};
use windows::Win32::UI::Accessibility::{
    AccessibleObjectFromWindow, CUIAutomation8, IAccessible, IUIAutomation, IUIAutomation2, IUIAutomationElement,
    IUIAutomationTextPattern, IUIAutomationTextPattern2, IUIAutomationTextRange, IUIAutomationValuePattern,
    TextPatternRangeEndpoint_End, TextPatternRangeEndpoint_Start, TextUnit_Character, UIA_DocumentControlTypeId,
    UIA_IsReadOnlyAttributeId, UIA_SpinnerControlTypeId, UIA_TextPattern2Id, UIA_TextPatternId, UIA_ValuePatternId,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetAncestor, GetClassNameW, GetForegroundWindow, GetGUIThreadInfo, GetWindowLongW, GetWindowRect,
    GetWindowThreadProcessId, ES_NUMBER, GA_ROOT, GA_ROOTOWNER, GUITHREADINFO, GWL_STYLE, OBJID_CARET,
};

use crate::capslock::geometry::Rect;

pub fn support() -> Result<&'static str, String> {
    Ok("System caret (Win32, MSAA, UI Automation)")
}

/// How long UI Automation may wait for an app: a hung one must not stall
/// the indicator's animation.
const UIA_TIMEOUT_MS: u32 = 200;

pub struct CaretTracker {
    /// Focus moves (`ime::focus_moves`) when the caret was located.
    located_at: u64,
    /// The foreground window the caret was last located in.
    located_in: HWND,
    /// Created when first needed (`Some(None)`: unavailable).
    automation: Option<Option<IUIAutomation>>,
}

impl CaretTracker {
    pub fn new() -> Result<Self, String> {
        // MSAA needs COM on this thread; S_FALSE (already initialised) is fine
        let _ = unsafe { CoInitializeEx(None, COINIT_MULTITHREADED) };
        Ok(CaretTracker { located_at: 0, located_in: HWND::default(), automation: None })
    }

    /// Whether the focused field has text selected: it then shows no text
    /// cursor. Fields UI Automation cannot read count as without one.
    pub fn has_selection(&mut self) -> bool {
        let Some(automation) = self.automation.get_or_insert_with(ui_automation).as_ref() else { return false };
        unsafe { selection_in_focus(automation) }.unwrap_or(false)
    }

    /// Whether the focused field takes only numbers.
    pub fn numeric_field(&mut self) -> bool {
        if gui_thread_info().is_some_and(|(info, _)| number_edit(info.hwndFocus)) {
            return true;
        }
        let Some(automation) = self.automation.get_or_insert_with(ui_automation).as_ref() else { return false };
        unsafe { spinner_in_focus(automation) }.unwrap_or(false)
    }

    pub fn locate(&mut self) -> Option<Rect> {
        self.located_at = super::ime::focus_moves();
        let (caret, window) = self.current_caret()?;
        self.located_in = window;
        Some(caret)
    }

    /// The sources are cheap (UI Automation is asked only where the others
    /// have no caret), so following simply reads the caret again. `None` once
    /// focus has moved on: the next field's caret is another field's, found
    /// by `locate` when that field is reported.
    pub fn refresh(&mut self) -> Option<Rect> {
        if super::ime::focus_moves() != self.located_at {
            return None;
        }
        let located_in = self.located_in;
        self.current_caret().filter(|(_, window)| *window == located_in).map(|(caret, _)| caret)
    }

    /// The caret of the foreground window, and that window.
    fn current_caret(&mut self) -> Option<(Rect, HWND)> {
        let (info, foreground) = gui_thread_info()?;
        let focus = if info.hwndFocus.is_invalid() { foreground } else { info.hwndFocus };
        let caret = system_caret(&info)
            .filter(|caret| shown_by(info.hwndCaret, *caret, foreground))
            .or_else(|| msaa_caret(focus).filter(|caret| shown_by(focus, *caret, foreground)))
            .or_else(|| {
                let automation = self.automation.get_or_insert_with(ui_automation).as_ref()?;
                unsafe { uia_caret(automation, focus) }.filter(|caret| shown_by(focus, *caret, foreground))
            })?;
        Some((caret, foreground))
    }

    /// Whether another window came to the front since the caret was located:
    /// its field is then behind it.
    pub fn window_changed(&self) -> bool {
        !self.located_in.is_invalid() && unsafe { GetForegroundWindow() } != self.located_in
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
        .map_err(|e| log::info!("UI Automation is unavailable, so selections and number fields are not detected: {e}"))
        .ok()?;
    if let Ok(timeouts) = automation.cast::<IUIAutomation2>() {
        let _ = unsafe { timeouts.SetConnectionTimeout(UIA_TIMEOUT_MS) };
        let _ = unsafe { timeouts.SetTransactionTimeout(UIA_TIMEOUT_MS) };
    }
    Some(automation)
}

/// A classic edit control that accepts only digits.
fn number_edit(window: HWND) -> bool {
    if window.is_invalid() {
        return false;
    }
    let mut class = [0u16; 16];
    let len = unsafe { GetClassNameW(window, &mut class) }.max(0) as usize;
    String::from_utf16_lossy(&class[..len]).eq_ignore_ascii_case("Edit")
        && unsafe { GetWindowLongW(window, GWL_STYLE) } & ES_NUMBER != 0
}

/// Whether the focused element is a spinner, or the text field of one.
unsafe fn spinner_in_focus(automation: &IUIAutomation) -> windows::core::Result<bool> {
    let focused = automation.GetFocusedElement()?;
    if is_spinner(&focused) {
        return Ok(true);
    }
    let parent = automation.ControlViewWalker()?.GetParentElement(&focused)?;
    Ok(is_spinner(&parent))
}

unsafe fn is_spinner(element: &IUIAutomationElement) -> bool {
    element.CurrentControlType().is_ok_and(|t| t == UIA_SpinnerControlTypeId)
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

/// The caret UI Automation reports for the focused element, if that element
/// belongs to the process owning `focus` (the keyboard focus of the
/// foreground window) and its text can be edited.
unsafe fn uia_caret(automation: &IUIAutomation, focus: HWND) -> Option<Rect> {
    let focused = automation.GetFocusedElement().ok()?;
    let mut owner = 0u32;
    windows::Win32::UI::WindowsAndMessaging::GetWindowThreadProcessId(focus, Some(&mut owner));
    if focused.CurrentProcessId().ok()? as u32 != owner {
        return None;
    }
    let range = caret_range(&focused)?;
    if !editable(&focused, &range) {
        log::debug!("UI Automation reports a caret in text that cannot be edited");
        return None;
    }
    caret_rect(&range)
}

/// The collapsed range at the caret: from `TextPattern2` while the element
/// has the keyboard focus, else the selection of its text pattern (Windows
/// Terminal), which is collapsed at the caret unless text is selected.
unsafe fn caret_range(element: &IUIAutomationElement) -> Option<IUIAutomationTextRange> {
    if let Ok(text) = element.GetCurrentPatternAs::<IUIAutomationTextPattern2>(UIA_TextPattern2Id) {
        let mut active = BOOL(0);
        let range = text.GetCaretRange(&mut active).ok()?;
        return active.as_bool().then_some(range);
    }
    let text: IUIAutomationTextPattern = element.GetCurrentPatternAs(UIA_TextPatternId).ok()?;
    let selection = text.GetSelection().ok()?;
    if selection.Length().ok()? < 1 {
        return None;
    }
    selection.GetElement(0).ok()
}

/// Whether the text at the caret can be edited. Where the app reports it for
/// the text itself (Chromium, XAML) that decides; otherwise the element's
/// value must not be read-only, and a document counts only with a value that
/// can be edited (a web page is a read-only document).
unsafe fn editable(element: &IUIAutomationElement, range: &IUIAutomationTextRange) -> bool {
    // a collapsed range may not answer for the text around it; the character after it does
    let at_caret = read_only(range).or_else(|| {
        let next = range.Clone().ok()?;
        next.ExpandToEnclosingUnit(TextUnit_Character).ok()?;
        read_only(&next)
    });
    if let Some(read_only) = at_caret {
        return !read_only;
    }
    let value = element.GetCurrentPatternAs::<IUIAutomationValuePattern>(UIA_ValuePatternId).ok();
    match value.and_then(|v| v.CurrentIsReadOnly().ok()) {
        Some(read_only) => !read_only.as_bool(),
        None => element.CurrentControlType().is_ok_and(|t| t != UIA_DocumentControlTypeId),
    }
}

/// The range's `IsReadOnly` text attribute; `None` where the app does not
/// report it or the range mixes both.
unsafe fn read_only(range: &IUIAutomationTextRange) -> Option<bool> {
    let mut value = range.GetAttributeValue(UIA_IsReadOnlyAttributeId).ok()?;
    let inner = &value.Anonymous.Anonymous;
    let answer = (inner.vt == VT_BOOL).then(|| inner.Anonymous.boolVal.0 != 0);
    // the "not supported" and "mixed" answers are objects
    let _ = VariantClear(&mut value);
    answer
}

/// The caret of a collapsed range. Most apps give it no rectangle of its
/// own; then it is the left edge of the character after it, or at the end
/// of the text the right edge of the character before it.
unsafe fn caret_rect(range: &IUIAutomationTextRange) -> Option<Rect> {
    if let Some(own) = bounding_rects(range).first() {
        return Some(Rect { width: 1, ..*own });
    }
    let after = range.Clone().ok().and_then(|next| {
        next.ExpandToEnclosingUnit(TextUnit_Character).ok()?;
        bounding_rects(&next).first().copied()
    });
    let before = || {
        let previous = range.Clone().ok()?;
        previous.MoveEndpointByUnit(TextPatternRangeEndpoint_Start, TextUnit_Character, -1).ok()?;
        bounding_rects(&previous).last().copied()
    };
    caret_beside(after, after.is_none().then(before).flatten())
}

/// The caret from the character after it, else the one before it.
fn caret_beside(after: Option<Rect>, before: Option<Rect>) -> Option<Rect> {
    match (after, before) {
        (Some(next), _) => Some(Rect { width: 1, ..next }),
        (None, Some(previous)) => Some(Rect { x: previous.x + previous.width, width: 1, ..previous }),
        (None, None) => None,
    }
}

/// The screen rectangles of a range (one per line it covers).
unsafe fn bounding_rects(range: &IUIAutomationTextRange) -> Vec<Rect> {
    let Ok(array) = range.GetBoundingRectangles() else { return Vec::new() };
    let rects = rects_from(&doubles(array));
    if !array.is_null() {
        let _ = SafeArrayDestroy(array);
    }
    rects
}

/// UI Automation's rectangles: left, top, width and height after each
/// other. Empty ones (text scrolled out of view) are left out.
fn rects_from(values: &[f64]) -> Vec<Rect> {
    values
        .chunks_exact(4)
        .filter(|r| r[3] > 0.0)
        .map(|r| Rect { x: r[0].round() as i32, y: r[1].round() as i32, width: r[2].round() as i32, height: r[3].round() as i32 })
        .collect()
}

unsafe fn doubles(array: *mut SAFEARRAY) -> Vec<f64> {
    if array.is_null() {
        return Vec::new();
    }
    let Ok(upper) = SafeArrayGetUBound(array, 1) else { return Vec::new() };
    let mut data = std::ptr::null_mut();
    if SafeArrayAccessData(array, &mut data).is_err() || data.is_null() {
        return Vec::new();
    }
    let values = std::slice::from_raw_parts(data as *const f64, (upper + 1).max(0) as usize).to_vec();
    let _ = SafeArrayUnaccessData(array);
    values
}

/// Whether `caret`, in `window`, is a text cursor of the foreground window:
/// `window` is part of it (a child, or a popup it owns) and the caret lies
/// inside the top-level window holding it. Anything else is a field of a
/// window behind it, or a stale position.
fn shown_by(window: HWND, caret: Rect, foreground: HWND) -> bool {
    let root = unsafe { GetAncestor(window, GA_ROOT) };
    if root.is_invalid() || unsafe { GetAncestor(root, GA_ROOTOWNER) != GetAncestor(foreground, GA_ROOTOWNER) } {
        log::debug!("the text cursor belongs to a window behind the foreground one");
        return false;
    }
    let mut frame = RECT::default();
    let inside = unsafe { GetWindowRect(root, &mut frame) }.is_ok() && within(frame, caret);
    if !inside {
        log::debug!("the text cursor {caret:?} lies outside its window {frame:?}");
    }
    inside
}

/// The caret's top-left inside `frame`, give or take a few pixels (a caret
/// right at the edge).
fn within(frame: RECT, caret: Rect) -> bool {
    let m = 8;
    caret.x >= frame.left - m && caret.x <= frame.right + m && caret.y >= frame.top - m && caret.y <= frame.bottom + m
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn carets_outside_the_window_are_not_its_own() {
        let frame = RECT { left: 960, top: 0, right: 1920, bottom: 1040 };
        let caret = |x, y| Rect { x, y, width: 1, height: 18 };
        assert!(within(frame, caret(1200, 300)));
        assert!(within(frame, caret(955, 1040)), "at the very edge");
        // the left half of a split screen, where the window behind it is
        assert!(!within(frame, caret(400, 300)));
        assert!(!within(frame, caret(1200, 1100)));
    }

    #[test]
    fn ui_automation_rectangles_are_read_in_fours() {
        // a range over two lines, and one scrolled out of view (no height)
        let values = [100.4, 200.0, 7.6, 19.0, 12.0, 219.0, 30.0, 19.0, 50.0, 60.0, 0.0, 0.0];
        assert_eq!(rects_from(&values), vec![
            Rect { x: 100, y: 200, width: 8, height: 19 },
            Rect { x: 12, y: 219, width: 30, height: 19 },
        ]);
        assert!(rects_from(&[1.0, 2.0, 3.0]).is_empty(), "an incomplete rectangle");
    }

    #[test]
    fn carets_sit_beside_their_character() {
        let after = Rect { x: 743, y: 14, width: 8, height: 19 };
        let before = Rect { x: 720, y: 14, width: 9, height: 19 };
        // the left edge of the character after the caret
        assert_eq!(caret_beside(Some(after), Some(before)), Some(Rect { x: 743, y: 14, width: 1, height: 19 }));
        // at the end of the text: the right edge of the last character
        assert_eq!(caret_beside(None, Some(before)), Some(Rect { x: 729, y: 14, width: 1, height: 19 }));
        assert_eq!(caret_beside(None, None), None);
    }

    #[test]
    fn edit_controls_for_digits_are_number_fields() {
        use windows::core::w;
        use windows::Win32::UI::WindowsAndMessaging::{CreateWindowExW, DestroyWindow, WINDOW_EX_STYLE, WINDOW_STYLE, WS_POPUP};
        let edit = |style: i32| unsafe {
            CreateWindowExW(WINDOW_EX_STYLE(0), w!("EDIT"), w!(""), WS_POPUP | WINDOW_STYLE(style as u32), 0, 0, 80, 20, None, None, None, None)
        }
        .unwrap();
        let (digits, text) = (edit(ES_NUMBER), edit(0));
        assert!(number_edit(digits));
        assert!(!number_edit(text));
        assert!(!number_edit(HWND::default()));
        unsafe {
            let _ = DestroyWindow(digits);
            let _ = DestroyWindow(text);
        }
    }
}
