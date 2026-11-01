//! Text cursor (caret) position for the Caps Lock indicator, read through the
//! AT-SPI accessibility bus that GTK, Qt, LibreOffice, Firefox and Chromium
//! apps (the latter only when started with `--force-renderer-accessibility`)
//! report their text fields on.
//!
//! 1. The focused window is the active AT-SPI window whose title matches the
//!    window Gala reports as focused (on other desktops: any active window).
//! 2. Its focused descendant must be an editable text field or a terminal;
//!    the caret is the left edge of the character at the caret offset.
//! 3. On **X11**, and for XWayland apps, these are screen coordinates. Native
//!    **Wayland** apps do not know where their window is and report
//!    coordinates relative to their window surface. Gala exposes its window
//!    actors on the same bus (Clutter accessibility) with their real screen
//!    rectangles, and the focused window's actor supplies the origin.
//!
//! Nothing is registered with the accessibility registry, so no app starts
//! emitting events because of Lumio; the bus is only read while the
//! indicator is about to show or is visible.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use gtk::gio;
use gtk::glib::{self, ToVariant};

use super::gala::{self, FocusedWindow};
use super::session::{self, SessionType};
use crate::capslock::geometry::Rect;

const REGISTRY: &str = "org.a11y.atspi.Registry";
const ROOT_PATH: &str = "/org/a11y/atspi/accessible/root";
const NULL_PATH: &str = "/org/a11y/atspi/null";
const ACCESSIBLE: &str = "org.a11y.atspi.Accessible";
const COMPONENT: &str = "org.a11y.atspi.Component";
const TEXT: &str = "org.a11y.atspi.Text";
const COLLECTION: &str = "org.a11y.atspi.Collection";
const COORD_SCREEN: u32 = 0;
const MATCH_ALL: i32 = 1;
const SORT_CANONICAL: u32 = 1;

// AtspiStateType bit numbers
const ACTIVE: u32 = 1;
const EDITABLE: u32 = 7;
const FOCUSED: u32 = 12;
const SHOWING: u32 = 25;
const VISIBLE: u32 = 30;
const MANAGES_DESCENDANTS: u32 = 31;

/// Per call: an app that hangs must not stall the indicator.
const CALL_TIMEOUT_MS: i32 = 150;
/// Bounds for the search of the focused object inside one window.
const MAX_NODES: usize = 3000;
const MAX_DEPTH: usize = 60;
const MAX_SEARCH_TIME: Duration = Duration::from_millis(300);
/// Objects that manage their descendants (documents, lists, tables) are
/// searched only up to this many children: a LibreOffice document holds its
/// visible paragraphs, a large list could hold thousands of rows.
const MAX_MANAGED_CHILDREN: i32 = 500;
/// Gala's window actors sit three levels below its stage window.
const GALA_MAX_DEPTH: usize = 4;
/// Largest client-side shadow plus resize border around a window frame.
const MAX_SHADOW: i32 = 200;

pub fn support() -> Result<&'static str, String> {
    let session = session::session_type();
    if session == SessionType::Unknown {
        return Err("No graphical session was detected.".into());
    }
    connect().map_err(|e| format!("The accessibility bus (AT-SPI) is not available ({e})."))?;
    if session == SessionType::Wayland && !session::is_pantheon() {
        return Err("On Wayland, Lumio can place text cursor positions on screen only with elementary OS's window \
                    manager (Gala)."
            .into());
    }
    Ok("Accessibility (AT-SPI)")
}

fn connect() -> Result<gio::DBusConnection, String> {
    let reply = gala::session_bus()?
        .call_sync(
            Some("org.a11y.Bus"),
            "/org/a11y/bus",
            "org.a11y.Bus",
            "GetAddress",
            None,
            Some(glib::VariantTy::new("(s)").expect("valid type")),
            gio::DBusCallFlags::NONE,
            1000,
            gio::Cancellable::NONE,
        )
        .map_err(|e| e.to_string())?;
    let (address,) = reply.get::<(String,)>().ok_or("unexpected GetAddress reply")?;
    gio::DBusConnection::for_address_sync(
        &address,
        gio::DBusConnectionFlags::AUTHENTICATION_CLIENT | gio::DBusConnectionFlags::MESSAGE_BUS_CONNECTION,
        None,
        gio::Cancellable::NONE,
    )
    .map_err(|e| e.to_string())
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct Obj {
    bus: String,
    path: String,
}

impl Obj {
    fn root() -> Self {
        Obj { bus: REGISTRY.into(), path: ROOT_PATH.into() }
    }
}

fn has(state: u64, bit: u32) -> bool {
    state & (1 << bit) != 0
}

struct Target {
    field: Obj,
    /// Added to the field's caret coordinates to get screen coordinates.
    offset: (i32, i32),
}

pub struct CaretTracker {
    a11y: gio::DBusConnection,
    session_bus: Option<gio::DBusConnection>,
    wayland: bool,
    gala: Option<Obj>,
    target: Option<Target>,
}

impl CaretTracker {
    pub fn new() -> Result<Self, String> {
        support()?;
        Ok(Self {
            a11y: connect()?,
            session_bus: gala::session_bus().ok(),
            wayland: session::is_wayland(),
            gala: None,
            target: None,
        })
    }

    /// Finds the focused text field and returns its caret in screen
    /// coordinates (X root pixels, logical on Wayland).
    pub fn locate(&mut self) -> Option<Rect> {
        let started = Instant::now();
        self.target = self.find_target();
        let caret = self.refresh();
        log::debug!("text cursor lookup took {:?}: {caret:?}", started.elapsed());
        caret
    }

    /// Re-reads the caret of the field `locate` found (a few cheap calls).
    /// `None` once that field has lost focus.
    pub fn refresh(&mut self) -> Option<Rect> {
        let target = self.target.as_ref()?;
        let caret = has(self.state(&target.field), FOCUSED)
            .then(|| self.caret(&target.field))
            .flatten()
            .map(|r| Rect { x: r.x + target.offset.0, y: r.y + target.offset.1, ..r });
        if caret.is_none() {
            self.target = None;
        }
        caret
    }

    pub fn release(&mut self) {
        self.target = None;
    }

    fn find_target(&mut self) -> Option<Target> {
        let focus = self.session_bus.as_ref().and_then(gala::focused_window);
        let mut windows = Vec::new();
        for app in self.children(&Obj::root()) {
            for window in self.children(&app) {
                if !has(self.state(&window), ACTIVE) {
                    continue;
                }
                // Several apps can claim an active window (Chromium keeps the
                // flag on background windows); Gala knows the real one.
                if let Some(f) = &focus {
                    if !same_window(&self.name(&window), &f.title) {
                        continue;
                    }
                }
                windows.push(window);
            }
        }
        windows.into_iter().find_map(|window| {
            let path = self.focused_by_collection(&window).or_else(|| self.find_focused(&window))?;
            let field = path.last()?.clone();
            if !self.is_text_field(&field) {
                log::debug!("the focused object is not a text field");
                return None;
            }
            let (x, y) = self.screen_offset(&window, focus.as_ref())?;
            let extents: Vec<Option<Rect>> = path.iter().map(|o| self.extents(o)).collect();
            let (dx, dy) = embedded_toolkit_shift(&extents);
            Some(Target { field, offset: (x + dx, y + dy) })
        })
    }

    /// What to add to the toolkit's coordinates to get screen coordinates.
    fn screen_offset(&mut self, window: &Obj, focus: Option<&FocusedWindow>) -> Option<(i32, i32)> {
        let x11_client = focus.is_some_and(|f| f.x11);
        if !self.wayland || x11_client {
            return Some((0, 0));
        }
        let surface = self.extents(window)?;
        if focus.is_none() && (surface.x, surface.y) != (0, 0) {
            return Some((0, 0)); // reports real positions, so an XWayland client
        }
        let actors = self.gala_actors()?;
        let frame = focus.map(|f| (f.width, f.height));
        let Some(actor) = pick_window_actor(&actors, (surface.width, surface.height), frame) else {
            log::debug!("no Gala window matches the {}x{} surface (frame {frame:?})", surface.width, surface.height);
            return None;
        };
        Some((actor.x - surface.x, actor.y - surface.y))
    }

    /// The focused descendant in one `Collection.GetMatches` call, which GTK 3,
    /// LibreOffice, Firefox and Chromium implement (6 ms instead of 200 in
    /// LibreOffice). Returns the path from `window` to it.
    fn focused_by_collection(&self, window: &Obj) -> Option<Vec<Obj>> {
        let rule = (
            vec![1i32 << FOCUSED, 0],
            MATCH_ALL,
            HashMap::<String, String>::new(),
            MATCH_ALL,
            vec![0i32; 4],
            MATCH_ALL,
            Vec::<String>::new(),
            MATCH_ALL,
            false,
        );
        let args = (rule, SORT_CANONICAL, 1i32, true).to_variant();
        let reply = self.call(window, COLLECTION, "GetMatches", Some(args), "(a(so))")?;
        let field = refs(&reply.child_value(0)).into_iter().next()?;
        let mut path = vec![field];
        while path.last() != Some(window) {
            let parent = self.property(path.last()?, ACCESSIBLE, "Parent").and_then(|v| reference(&v))?;
            if path.len() > MAX_DEPTH || parent.path == ROOT_PATH {
                break; // the app's root: the window was passed under another name
            }
            path.push(parent);
        }
        path.reverse();
        Some(path)
    }

    /// Depth-first search for the focused descendant, through the visible
    /// part of the tree only (toolkits without `Collection`, such as GTK 4).
    /// Returns the path from `window` to it.
    fn find_focused(&self, window: &Obj) -> Option<Vec<Obj>> {
        let deadline = Instant::now() + MAX_SEARCH_TIME;
        // every object seen, with the index of its parent
        let mut seen: Vec<(Obj, Option<usize>)> = vec![(window.clone(), None)];
        let mut stack = vec![(0usize, 0usize)];
        let mut visited = 0;
        while let Some((index, depth)) = stack.pop() {
            visited += 1;
            if visited > MAX_NODES || Instant::now() > deadline {
                log::debug!("gave up looking for the focused object after {visited} objects");
                return None;
            }
            let obj = seen[index].0.clone();
            let state = self.state(&obj);
            if has(state, FOCUSED) {
                let mut path = Vec::with_capacity(depth + 1);
                let mut at = Some(index);
                while let Some(i) = at {
                    path.push(seen[i].0.clone());
                    at = seen[i].1;
                }
                path.reverse();
                return Some(path);
            }
            let showing = depth == 0 || has(state, SHOWING);
            let searchable = !has(state, MANAGES_DESCENDANTS) || self.child_count(&obj) <= MAX_MANAGED_CHILDREN;
            if showing && depth < MAX_DEPTH && searchable {
                for child in self.children(&obj).into_iter().rev() {
                    seen.push((child, Some(index)));
                    stack.push((seen.len() - 1, depth + 1));
                }
            }
        }
        None
    }

    fn is_text_field(&self, obj: &Obj) -> bool {
        has(self.state(obj), EDITABLE) || self.role_name(obj) == "terminal"
    }

    /// The caret rectangle in the toolkit's coordinates.
    fn caret(&self, field: &Obj) -> Option<Rect> {
        let offset = self.property(field, TEXT, "CaretOffset")?.get::<i32>()?;
        if offset < 0 {
            return None;
        }
        let count = self.property(field, TEXT, "CharacterCount").and_then(|v| v.get::<i32>()).unwrap_or(0);
        if offset < count {
            if let Some(c) = self.char_extents(field, offset) {
                return Some(Rect { width: 1, ..c });
            }
        }
        if offset > 0 {
            // at the end of the text: right after the last character
            if let Some(c) = self.char_extents(field, offset - 1) {
                return Some(Rect { x: c.x + c.width, y: c.y, width: 1, height: c.height });
            }
        }
        empty_field_caret(self.extents(field)?)
    }

    fn char_extents(&self, field: &Obj, offset: i32) -> Option<Rect> {
        let reply = self.call(field, TEXT, "GetCharacterExtents", Some((offset, COORD_SCREEN).to_variant()), "(iiii)")?;
        let (x, y, width, height) = reply.get::<(i32, i32, i32, i32)>()?;
        (height > 0 && height < 500).then_some(Rect { x, y, width, height })
    }

    fn extents(&self, obj: &Obj) -> Option<Rect> {
        let reply = self.call(obj, COMPONENT, "GetExtents", Some((COORD_SCREEN,).to_variant()), "((iiii))")?;
        let ((x, y, width, height),) = reply.get::<((i32, i32, i32, i32),)>()?;
        Some(Rect { x, y, width, height })
    }

    /// Visible actors below Gala's stage with their depth, in stacking order.
    fn gala_actors(&mut self) -> Option<Vec<(usize, Rect)>> {
        let gala = match self.gala.clone() {
            Some(g) => g,
            None => {
                let g = self.children(&Obj::root()).into_iter().find(|app| self.name(app) == "gala")?;
                self.gala = Some(g.clone());
                g
            }
        };
        let mut actors = Vec::new();
        self.collect_actors(&gala, 0, &mut actors);
        if actors.is_empty() {
            self.gala = None; // Gala restarted: look it up again next time
            return None;
        }
        Some(actors)
    }

    fn collect_actors(&self, obj: &Obj, depth: usize, out: &mut Vec<(usize, Rect)>) {
        for child in self.children(obj) {
            let state = self.state(&child);
            if !(has(state, VISIBLE) && has(state, SHOWING)) {
                continue;
            }
            if let Some(r) = self.extents(&child).filter(|r| r.width > 0 && r.height > 0) {
                out.push((depth, r));
            }
            if depth < GALA_MAX_DEPTH {
                self.collect_actors(&child, depth + 1, out);
            }
        }
    }

    fn call(&self, obj: &Obj, iface: &str, method: &str, args: Option<glib::Variant>, reply: &str) -> Option<glib::Variant> {
        self.a11y
            .call_sync(
                Some(&obj.bus),
                &obj.path,
                iface,
                method,
                args.as_ref(),
                Some(glib::VariantTy::new(reply).ok()?),
                gio::DBusCallFlags::NO_AUTO_START,
                CALL_TIMEOUT_MS,
                gio::Cancellable::NONE,
            )
            .ok()
    }

    fn children(&self, obj: &Obj) -> Vec<Obj> {
        self.call(obj, ACCESSIBLE, "GetChildren", None, "(a(so))").map(|r| refs(&r.child_value(0))).unwrap_or_default()
    }

    fn state(&self, obj: &Obj) -> u64 {
        self.call(obj, ACCESSIBLE, "GetState", None, "(au)")
            .and_then(|v| v.child_value(0).get::<Vec<u32>>())
            .map(|words| words.iter().take(2).enumerate().fold(0u64, |acc, (i, w)| acc | (*w as u64) << (32 * i)))
            .unwrap_or(0)
    }

    fn property(&self, obj: &Obj, iface: &str, name: &str) -> Option<glib::Variant> {
        let reply = self.call(obj, "org.freedesktop.DBus.Properties", "Get", Some((iface, name).to_variant()), "(v)")?;
        reply.child_value(0).as_variant()
    }

    fn child_count(&self, obj: &Obj) -> i32 {
        self.property(obj, ACCESSIBLE, "ChildCount").and_then(|v| v.get::<i32>()).unwrap_or(i32::MAX)
    }

    fn name(&self, obj: &Obj) -> String {
        self.property(obj, ACCESSIBLE, "Name").and_then(|v| v.get::<String>()).unwrap_or_default()
    }

    fn role_name(&self, obj: &Obj) -> String {
        self.call(obj, ACCESSIBLE, "GetRoleName", None, "(s)")
            .and_then(|v| v.get::<(String,)>())
            .map(|(s,)| s)
            .unwrap_or_default()
    }
}

/// An object reference `(so)`; `None` for the null object.
fn reference(v: &glib::Variant) -> Option<Obj> {
    let bus = v.try_child_value(0)?.str()?.to_string();
    let path = v.try_child_value(1)?.str()?.to_string();
    (!bus.is_empty() && path != NULL_PATH).then_some(Obj { bus, path })
}

/// A list of object references `a(so)`.
fn refs(list: &glib::Variant) -> Vec<Obj> {
    (0..list.n_children()).filter_map(|i| reference(&list.child_value(i))).collect()
}

/// AT-SPI window names usually equal the title; Chromium appends the profile
/// name ("New Tab - Google Chrome - Work").
fn same_window(accessible_name: &str, title: &str) -> bool {
    !accessible_name.is_empty()
        && !title.is_empty()
        && (accessible_name.starts_with(title) || title.starts_with(accessible_name))
}

/// A toolkit embedded in another one's window (LibreOffice's VCL inside a GTK
/// window) reports positions relative to its own top-level pane, and that
/// pane claims to sit at (0, 0) although its parent (same size) is further
/// down. `extents` runs from the window to the focused object; the result is
/// the sum of those hidden origins.
fn embedded_toolkit_shift(extents: &[Option<Rect>]) -> (i32, i32) {
    extents.windows(2).fold((0, 0), |(dx, dy), pair| match pair {
        [Some(parent), Some(child)]
            if (child.x, child.y) == (0, 0)
                && (parent.x, parent.y) != (0, 0)
                && (child.width, child.height) == (parent.width, parent.height) =>
        {
            (dx + parent.x, dy + parent.y)
        }
        _ => (dx, dy),
    })
}

/// The caret of an empty field sits at its start: top-left in a multi-line
/// field, vertically centred in a single-line one.
fn empty_field_caret(field: Rect) -> Option<Rect> {
    if field.width <= 0 || field.height <= 0 {
        return None;
    }
    let height = field.height.min(20);
    let y = if field.height > 48 { field.y + 4 } else { field.y + (field.height - height) / 2 };
    Some(Rect { x: field.x + 4, y, width: 1, height })
}

/// Picks the actor of the focused window among Gala's visible actors
/// (`(depth, rect)` in stacking order, bottom first): the topmost one at the
/// shallowest depth whose size equals the window surface, or else (toolkits
/// that scale their window size, like Chromium with text scaling) one that
/// surrounds the frame Gala reports with the surface's aspect ratio.
fn pick_window_actor(actors: &[(usize, Rect)], surface: (i32, i32), frame: Option<(i32, i32)>) -> Option<Rect> {
    let exact = |r: &Rect| (r.width, r.height) == surface;
    let around_frame = |r: &Rect| {
        let Some((fw, fh)) = frame else { return false };
        let aspect = |w: i32, h: i32| w as f64 / h.max(1) as f64;
        (fw..=fw + MAX_SHADOW).contains(&r.width)
            && (fh..=fh + MAX_SHADOW).contains(&r.height)
            && (aspect(r.width, r.height) / aspect(surface.0, surface.1) - 1.0).abs() < 0.005
    };
    topmost_shallowest(actors, exact).or_else(|| topmost_shallowest(actors, around_frame))
}

fn topmost_shallowest(actors: &[(usize, Rect)], test: impl Fn(&Rect) -> bool) -> Option<Rect> {
    let depth = actors.iter().filter(|(_, r)| test(r)).map(|(d, _)| *d).min()?;
    actors.iter().filter(|(d, r)| *d == depth && test(r)).last().map(|(_, r)| *r)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(x: i32, y: i32, width: i32, height: i32) -> Rect {
        Rect { x, y, width, height }
    }

    /// Visible actors of Gala on elementary OS 8, as read during development:
    /// a terminal, two maximized Chromium windows, a GTK window with shadows.
    fn desktop() -> Vec<(usize, Rect)> {
        vec![
            (1, r(0, 0, 1920, 1080)),
            (3, r(0, 0, 1920, 1080)),
            (3, r(743, 117, 851, 832)),
            (4, r(743, 117, 851, 832)),
            (3, r(0, 30, 1921, 1051)),
            (4, r(0, 30, 1921, 1051)),
            (3, r(-22, 11, 1967, 1097)),
            (4, r(-22, 11, 1967, 1097)),
            (3, r(661, 355, 598, 426)),
            (4, r(661, 355, 598, 426)),
        ]
    }

    #[test]
    fn gtk_surface_matches_its_actor_exactly() {
        assert_eq!(pick_window_actor(&desktop(), (598, 426), Some((544, 372))), Some(r(661, 355, 598, 426)));
        assert_eq!(pick_window_actor(&desktop(), (851, 832), Some((797, 778))), Some(r(743, 117, 851, 832)));
    }

    #[test]
    fn scaled_chromium_window_is_found_through_its_frame() {
        // Chrome reported a 2375x1324 window for a 1923x1053 frame
        assert_eq!(pick_window_actor(&desktop(), (2375, 1324), Some((1923, 1053))), Some(r(-22, 11, 1967, 1097)));
    }

    #[test]
    fn fullscreen_panels_do_not_pass_for_a_window() {
        // a 1900x1000 frame: the 1920x1080 stage panels are in range but have another aspect ratio
        assert_eq!(pick_window_actor(&desktop(), (2280, 1200), Some((1900, 1000))), None);
        assert_eq!(pick_window_actor(&desktop(), (1000, 700), None), None);
    }

    #[test]
    fn same_size_windows_resolve_to_the_topmost() {
        let mut actors = desktop();
        actors.push((3, r(100, 100, 598, 426)));
        assert_eq!(pick_window_actor(&actors, (598, 426), None), Some(r(100, 100, 598, 426)));
    }

    #[test]
    fn libreoffice_panes_inside_gtk_are_shifted() {
        // LibreOffice Writer on Wayland: frame, GTK panels, VCL root pane, document, paragraph
        let path = [
            Some(r(0, 0, 1920, 1050)),
            Some(r(0, 37, 1920, 1013)),
            Some(r(0, 61, 1920, 989)),
            Some(r(0, 0, 1920, 989)),
            Some(r(0, 78, 1920, 887)),
            Some(r(343, 238, 1161, 26)),
        ];
        assert_eq!(embedded_toolkit_shift(&path), (0, 61));
    }

    #[test]
    fn ordinary_trees_are_not_shifted() {
        // GTK and Chromium: children report positions in the window's space
        let gtk = [Some(r(0, 0, 598, 426)), Some(r(26, 26, 546, 374)), Some(r(26, 26, 546, 374)), Some(r(30, 60, 500, 20))];
        assert_eq!(embedded_toolkit_shift(&gtk), (0, 0));
        let unknown = [Some(r(0, 0, 800, 600)), None, Some(r(0, 0, 800, 600))];
        assert_eq!(embedded_toolkit_shift(&unknown), (0, 0));
    }

    #[test]
    fn window_names_match_titles() {
        assert!(same_window("New Tab - Google Chrome - Sanjit", "New Tab - Google Chrome"));
        assert!(same_window("Lumio caret test GTK3", "Lumio caret test GTK3"));
        assert!(!same_window("Terminal", "Spotlight - Visual Studio Code"));
        assert!(!same_window("", "Files"));
    }

    #[test]
    fn empty_fields_put_the_caret_at_their_start() {
        assert_eq!(empty_field_caret(r(10, 10, 200, 30)), Some(r(14, 15, 1, 20)));
        assert_eq!(empty_field_caret(r(10, 10, 400, 300)), Some(r(14, 14, 1, 20)));
        assert_eq!(empty_field_caret(r(10, 10, 0, 0)), None);
    }
}
