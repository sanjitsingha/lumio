//! Text cursor (caret) position for the Caps Lock indicator, read through the
//! AT-SPI accessibility bus that GTK, Qt, LibreOffice, Firefox and Chromium
//! apps (the latter only when started with `--force-renderer-accessibility`)
//! report their text fields on.
//!
//! 1. The focused window is the AT-SPI window whose title matches the window
//!    Gala reports as focused (on other desktops: any active window).
//! 2. Its focused descendant must be an editable text field or a terminal;
//!    the caret is the left edge of the character at the caret offset.
//! 3. On **X11**, and for XWayland apps, these are screen coordinates. Native
//!    **Wayland** apps do not know where their window is and report
//!    coordinates relative to their window surface. Gala exposes its window
//!    actors on the same bus (Clutter accessibility) with their real screen
//!    rectangles, and the focused window's actor supplies the origin.
//!
//! Nothing is registered with the accessibility registry, so no app starts
//! emitting events because of Lummo; the bus is only read while the
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
const HYPERTEXT: &str = "org.a11y.atspi.Hypertext";
const HYPERLINK: &str = "org.a11y.atspi.Hyperlink";
/// The character standing for an embedded object (a child) in a text.
const EMBEDDED_OBJECT: &str = "\u{FFFC}";
/// How deep rich editors nest the object that holds the caret.
const MAX_EMBEDDING: usize = 12;
const COORD_SCREEN: u32 = 0;
const COORD_WINDOW: u32 = 1;
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
/// How long the focused window's place is reused for input method positions.
const GEOMETRY_REUSE: Duration = Duration::from_secs(4);
/// Bounds for the search of the focused object inside one window.
const MAX_NODES: usize = 3000;
const MAX_DEPTH: usize = 60;
const MAX_SEARCH_TIME: Duration = Duration::from_millis(700);
/// Objects that manage their descendants (documents, lists, tables) are
/// searched only up to this many children: a LibreOffice document holds its
/// visible paragraphs, a large list could hold thousands of rows.
const MAX_MANAGED_CHILDREN: i32 = 500;
/// Gala's window actors sit three levels below its stage window.
const GALA_MAX_DEPTH: usize = 4;
/// Largest client-side shadow plus resize border around a window frame.
const MAX_SHADOW: i32 = 200;
/// Fields up to this height (logical px) are single-line.
const SINGLE_LINE_MAX: i32 = 48;

pub fn support() -> Result<&'static str, String> {
    let session = session::session_type();
    if session == SessionType::Unknown {
        return Err("No graphical session was detected.".into());
    }
    connect().map_err(|e| format!("The accessibility bus (AT-SPI) is not available ({e})."))?;
    if session == SessionType::Wayland && !session::is_pantheon() {
        return Err("On Wayland, Lummo can place text cursor positions on screen only with elementary OS's window \
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
    /// The field's caret coordinates are divided by this (Chromium's own
    /// controls are reported scaled; its web pages are not)…
    scale: f64,
    /// …then this is added to get screen coordinates.
    offset: (i32, i32),
}

pub struct CaretTracker {
    a11y: gio::DBusConnection,
    session_bus: Option<gio::DBusConnection>,
    wayland: bool,
    gala: Option<Obj>,
    target: Option<Target>,
    /// The last focused field and its path from its window: checked first,
    /// so toggling again in the same field needs no search.
    last_path: Option<Vec<Obj>>,
    /// Where the focused window was last found, for input method positions:
    /// (when, Gala's focused window it belongs to, geometry).
    geometry: Option<(Instant, FocusedWindow, WindowGeometry)>,
}

/// Where the focused window is on screen, for positions an app reports
/// relative to it.
#[derive(Debug, Clone, Copy, PartialEq)]
struct WindowGeometry {
    /// Gala's actor of the window: its surface, shadows included.
    actor: Rect,
    /// How much the toolkit scales its positions (Chromium with a text
    /// scaling factor), 1 otherwise.
    scale: f64,
    /// An X11 client (or session): positions are screen positions already.
    x11: bool,
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
            last_path: None,
            geometry: None,
        })
    }

    /// Converts a text cursor reported to the input method into screen
    /// coordinates. `relative` positions are relative to the focused window
    /// (GTK, Qt); the others are screen positions (Wayland text-input clients
    /// through Gala, X11 apps). `None` when the focused window cannot be
    /// placed or the position lies outside it (a field of another window
    /// that kept its focus).
    pub fn map_input_method(&mut self, rect: Rect, relative: bool, fresh: bool) -> Option<Rect> {
        let geometry = self.window_geometry(fresh)?;
        let caret = map_reported(rect, relative, &geometry);
        let inside = geometry.x11 || within(geometry.actor, caret, 8);
        if !inside {
            log::debug!("input method cursor {caret:?} lies outside the focused window {:?}", geometry.actor);
        }
        inside.then_some(caret)
    }

    /// The focused window's geometry, reused for a few seconds unless `fresh`
    /// (Caps Lock pressed: the window may have moved since).
    fn window_geometry(&mut self, fresh: bool) -> Option<WindowGeometry> {
        let focus = self.focused_window();
        if !self.wayland || focus.as_ref().is_some_and(|f| f.x11) {
            return Some(WindowGeometry { actor: Rect { x: 0, y: 0, width: 0, height: 0 }, scale: 1.0, x11: true });
        }
        let focus = focus?;
        if let Some((at, of, g)) = &self.geometry {
            if !fresh && *of == focus && at.elapsed() < GEOMETRY_REUSE {
                return Some(*g);
            }
        }
        let started = Instant::now();
        let actors = self.gala_actors()?;
        let frame = (focus.width, focus.height);
        let surface = self.focused_windows(Some(&focus)).first().and_then(|w| self.extents(w));
        let geometry = surface
            .and_then(|s| {
                let actor = pick_window_actor(&actors, (s.width, s.height), Some(frame))?;
                Some(WindowGeometry { actor, scale: toolkit_scale(s, actor), x11: false })
            })
            // GTK 4 reports only its frame; its positions are surface positions
            .or_else(|| actor_around_frame(&actors, frame).map(|actor| WindowGeometry { actor, scale: 1.0, x11: false }));
        log::debug!("focused window geometry {geometry:?} for {surface:?} (frame {frame:?}) in {:?}", started.elapsed());
        let geometry = geometry?;
        self.geometry = Some((Instant::now(), focus, geometry));
        Some(geometry)
    }

    fn focused_window(&self) -> Option<FocusedWindow> {
        let focused = || self.session_bus.as_ref().and_then(gala::focused_window);
        // right after a focus change Gala can briefly report no focused window
        focused().or_else(|| {
            session::is_pantheon().then(|| std::thread::sleep(Duration::from_millis(60)))?;
            focused()
        })
    }

    /// AT-SPI windows that can be the focused one, most likely first. Gala
    /// knows the focused window, so its title is matched: Chromium sets the
    /// active flag late after a window is focused and keeps it on background
    /// windows. Active windows are tried first.
    fn focused_windows(&self, focus: Option<&FocusedWindow>) -> Vec<Obj> {
        let all: Vec<Obj> = self.children(&Obj::root()).iter().flat_map(|app| self.children(app)).collect();
        let (active, inactive): (Vec<Obj>, Vec<Obj>) = all.into_iter().partition(|w| has(self.state(w), ACTIVE));
        match focus {
            Some(f) => {
                // titles are read only where needed: some apps answer slowly
                let matches = |w: &Obj| same_window(&self.name(w), &f.title);
                let found: Vec<Obj> = active.into_iter().filter(|w| matches(w)).collect();
                if found.is_empty() { inactive.into_iter().filter(|w| matches(w)).collect() } else { found }
            }
            None => active,
        }
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
            .map(|r| to_screen(r, target.scale, target.offset));
        if caret.is_none() {
            self.target = None;
        }
        caret
    }

    pub fn release(&mut self) {
        self.target = None;
    }

    /// The text field `locate` found, on screen (where the caret must be).
    pub fn focused_field(&self) -> Option<Rect> {
        let target = self.target.as_ref()?;
        let field = self.field_extents(&target.field)?;
        Some(to_screen(field, target.scale, target.offset))
    }

    fn find_target(&mut self) -> Option<Target> {
        let started = Instant::now();
        let focus = self.focused_window();
        let windows = self.focused_windows(focus.as_ref());
        let windows_found = started.elapsed();
        windows.into_iter().find_map(|window| {
            // Collection names the focused object reliably and fast; the last
            // path only saves walking the tree again (Chromium keeps the
            // focused flag on its address bar after focus moved into a page,
            // so a remembered field is never trusted where Collection works)
            let path = match self.focused_by_collection(&window) {
                Some(field) => self.remembered_path_to(&window, &field).unwrap_or_else(|| self.path_to(&window, field)),
                None => self.remembered_path(&window).or_else(|| self.find_focused(&window))?,
            };
            let path_found = started.elapsed();
            self.last_path = Some(path.clone());
            let field = path.last()?.clone();
            if !self.is_text_field(&field) {
                log::debug!("the focused object is not a text field");
                return None;
            }
            let (x, y, window_scale) = self.screen_offset(&window, focus.as_ref())?;
            let origin_found = started.elapsed();
            let extents: Vec<Option<Rect>> = path.iter().map(|o| self.extents(o)).collect();
            let (dx, dy) = embedded_toolkit_shift(&extents);
            // Chromium scales its window and its own controls (the address bar)
            // with the text scaling factor, but not its web pages
            let in_web_page = || path.iter().any(|o| self.role_name(o) == "document web");
            let scale = if window_scale != 1.0 && !in_web_page() { window_scale } else { 1.0 };
            log::debug!(
                "text cursor lookup: window {windows_found:?}, field {path_found:?} (depth {}), origin {origin_found:?}, \
                 layout {:?}",
                path.len(),
                started.elapsed()
            );
            Some(Target { field, scale, offset: (x + dx, y + dy) })
        })
    }

    /// What to add to the toolkit's coordinates to get screen coordinates.
    /// Also returns the factor by which the toolkit scales its window size
    /// (1 except for Chromium with a text scaling factor).
    fn screen_offset(&mut self, window: &Obj, focus: Option<&FocusedWindow>) -> Option<(i32, i32, f64)> {
        let x11_client = focus.is_some_and(|f| f.x11);
        if !self.wayland || x11_client {
            return Some((0, 0, 1.0));
        }
        let surface = self.extents(window)?;
        if focus.is_none() && (surface.x, surface.y) != (0, 0) {
            return Some((0, 0, 1.0)); // reports real positions, so an XWayland client
        }
        let actors = self.gala_actors()?;
        let frame = focus.map(|f| (f.width, f.height));
        let Some(actor) = pick_window_actor(&actors, (surface.width, surface.height), frame) else {
            log::debug!("no Gala window matches the {}x{} surface (frame {frame:?})", surface.width, surface.height);
            return None;
        };
        Some((actor.x - surface.x, actor.y - surface.y, toolkit_scale(surface, actor)))
    }

    /// The path found last time, if its field in this window still has focus
    /// (toolkits without `Collection`, where finding it again is slow).
    fn remembered_path(&self, window: &Obj) -> Option<Vec<Obj>> {
        let path = self.last_path.as_ref()?;
        let still_focused = path.first() == Some(window) && has(self.state(path.last()?), FOCUSED);
        still_focused.then(|| path.clone())
    }

    /// The path found last time, if it leads to `field` in this window.
    fn remembered_path_to(&self, window: &Obj, field: &Obj) -> Option<Vec<Obj>> {
        let path = self.last_path.as_ref()?;
        (path.first() == Some(window) && path.last() == Some(field)).then(|| path.clone())
    }

    /// The focused descendant in one `Collection.GetMatches` call, which GTK 3,
    /// LibreOffice, Firefox and Chromium implement (6 ms instead of 200 in
    /// LibreOffice).
    fn focused_by_collection(&self, window: &Obj) -> Option<Obj> {
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
        refs(&reply.child_value(0)).into_iter().next()
    }

    /// The path from `window` down to `field`, through the parents.
    fn path_to(&self, window: &Obj, field: Obj) -> Vec<Obj> {
        let mut path = vec![field];
        while path.last() != Some(window) {
            let Some(parent) = path.last().and_then(|o| self.property(o, ACCESSIBLE, "Parent")).and_then(|v| reference(&v))
            else {
                break;
            };
            if path.len() > MAX_DEPTH || parent.path == ROOT_PATH {
                break; // the app's root: the window was passed under another name
            }
            path.push(parent);
        }
        path.reverse();
        path
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
        let field = &self.caret_holder(field);
        let offset = self.property(field, TEXT, "CaretOffset")?.get::<i32>()?;
        if offset < 0 {
            return None;
        }
        let count = self.property(field, TEXT, "CharacterCount").and_then(|v| v.get::<i32>()).unwrap_or(0);
        // GTK and LibreOffice also answer for the offset right after the last
        // character, which is where the caret of an empty field sits
        if offset <= count {
            // past the end only a caret-sized answer is a caret position
            if let Some(c) = self.char_extents(field, offset).filter(|c| offset < count || c.width <= 4) {
                return Some(Rect { width: 1, ..c });
            }
        }
        if offset > 0 {
            // at the end of the text: right after the last character
            if let Some(c) = self.char_extents(field, offset - 1) {
                return Some(Rect { x: c.x + c.width, y: c.y, width: 1, height: c.height });
            }
        }
        field_caret(self.field_extents(field)?, count == 0 && offset == 0)
    }

    /// Rich editors (Chromium `contenteditable`: ChatGPT, WhatsApp, Gmail…)
    /// keep their text in child objects. The field's own text then holds one
    /// object replacement character per child, and its caret offset points
    /// at the child that holds the caret; that child's caret is the real one.
    fn caret_holder(&self, field: &Obj) -> Obj {
        let mut holder = field.clone();
        for _ in 0..MAX_EMBEDDING {
            let Some(offset) = self.property(&holder, TEXT, "CaretOffset").and_then(|v| v.get::<i32>()) else { break };
            if offset < 0 || self.text(&holder, offset, offset + 1).as_deref() != Some(EMBEDDED_OBJECT) {
                break;
            }
            // a child without a caret of its own (the page has not placed
            // one yet) leaves the field's own position for that child
            let child_offset = |c: &Obj| self.property(c, TEXT, "CaretOffset").and_then(|v| v.get::<i32>());
            match self.embedded_child(&holder, offset) {
                Some(child) if child_offset(&child).is_some_and(|o| o >= 0) => holder = child,
                _ => break,
            }
        }
        holder
    }

    /// The child object at `offset` in `obj`'s text.
    fn embedded_child(&self, obj: &Obj, offset: i32) -> Option<Obj> {
        let by_link = || {
            let (index,) = self.call(obj, HYPERTEXT, "GetLinkIndex", Some((offset,).to_variant()), "(i)")?.get::<(i32,)>()?;
            let link = self.call(obj, HYPERTEXT, "GetLink", Some((index,).to_variant()), "((so))")?;
            let link = reference(&link.child_value(0))?;
            let child = self.call(&link, HYPERLINK, "GetObject", Some((0i32,).to_variant()), "((so))")?;
            reference(&child.child_value(0))
        };
        // without Hypertext: the child whose caret offset is valid (Chromium
        // reports -1 for the others)
        by_link().or_else(|| {
            self.children(obj).into_iter().take(MAX_MANAGED_CHILDREN as usize).find(|c| {
                self.property(c, TEXT, "CaretOffset").and_then(|v| v.get::<i32>()).is_some_and(|o| o >= 0)
            })
        })
    }

    fn text(&self, obj: &Obj, start: i32, end: i32) -> Option<String> {
        self.call(obj, TEXT, "GetText", Some((start, end).to_variant()), "(s)")?.get::<(String,)>().map(|(t,)| t)
    }

    /// GTK 4.14 reports (0, 0) for every screen position but correct window
    /// positions, which are what the other toolkits' screen positions are on
    /// Wayland too.
    fn field_extents(&self, field: &Obj) -> Option<Rect> {
        let screen = self.extents(field)?;
        if (screen.x, screen.y) != (0, 0) {
            return Some(screen);
        }
        Some(self.extents_in(field, COORD_WINDOW).filter(|w| (w.x, w.y) != (0, 0)).unwrap_or(screen))
    }

    fn char_extents(&self, field: &Obj, offset: i32) -> Option<Rect> {
        let reply = self.call(field, TEXT, "GetCharacterExtents", Some((offset, COORD_SCREEN).to_variant()), "(iiii)")?;
        let (x, y, width, height) = reply.get::<(i32, i32, i32, i32)>()?;
        (height > 0 && height < 500).then_some(Rect { x, y, width, height })
    }

    fn extents(&self, obj: &Obj) -> Option<Rect> {
        self.extents_in(obj, COORD_SCREEN)
    }

    fn extents_in(&self, obj: &Obj, coords: u32) -> Option<Rect> {
        let reply = self.call(obj, COMPONENT, "GetExtents", Some((coords,).to_variant()), "((iiii))")?;
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

/// How much a toolkit scales its window size relative to Gala's actor: 1.2
/// for Chromium with a 0.83 text scaling factor, otherwise 1.
fn toolkit_scale(surface: Rect, actor: Rect) -> f64 {
    let scale = if actor.width > 0 { surface.width as f64 / actor.width as f64 } else { 1.0 };
    if (scale - 1.0).abs() < 0.01 {
        1.0
    } else {
        scale
    }
}

/// The topmost actor that can hold a window frame of this size with its
/// shadows (for toolkits that report only the frame).
fn actor_around_frame(actors: &[(usize, Rect)], (fw, fh): (i32, i32)) -> Option<Rect> {
    let fits = |r: &Rect| (fw..=fw + MAX_SHADOW).contains(&r.width) && (fh..=fh + MAX_SHADOW).contains(&r.height);
    // deepest level first: window actors sit below the stage's full-screen panels
    let depth = actors.iter().filter(|(_, r)| fits(r)).map(|(d, _)| *d).max()?;
    actors.iter().rev().find(|(d, r)| *d == depth && fits(r)).map(|(_, r)| *r)
}

/// An input method position to screen coordinates. Chromium reports its
/// text-input positions scaled like its window, so they are unscaled around
/// the window's origin.
fn map_reported(r: Rect, relative: bool, g: &WindowGeometry) -> Rect {
    if g.x11 {
        return Rect { width: r.width.max(1), ..r };
    }
    let (ax, ay) = (g.actor.x as f64, g.actor.y as f64);
    let s = g.scale;
    let (x, y) = if relative {
        (ax + r.x as f64 / s, ay + r.y as f64 / s)
    } else {
        (ax + (r.x as f64 - ax) / s, ay + (r.y as f64 - ay) / s)
    };
    let size = |v: i32| ((v as f64 / s).round() as i32).max(1);
    Rect { x: x.round() as i32, y: y.round() as i32, width: size(r.width), height: size(r.height) }
}

/// Whether `inner`'s top-left lies within `outer`, give or take `margin`.
fn within(outer: Rect, inner: Rect, margin: i32) -> bool {
    inner.x >= outer.x - margin
        && inner.y >= outer.y - margin
        && inner.x <= outer.x + outer.width + margin
        && inner.y <= outer.y + outer.height + margin
}

/// Toolkit coordinates to screen coordinates.
fn to_screen(r: Rect, scale: f64, (dx, dy): (i32, i32)) -> Rect {
    let s = |v: i32| (v as f64 / scale).round() as i32;
    Rect { x: s(r.x) + dx, y: s(r.y) + dy, width: s(r.width).max(1), height: s(r.height).max(1) }
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

/// Without character positions (an empty field, or GTK 4.14, which does not
/// report them) only the field is known. A single-line field is the caret's
/// line, so the caret spans it at its start; in a multi-line field only the
/// caret of an empty field is known (top-left).
fn field_caret(field: Rect, empty: bool) -> Option<Rect> {
    if field.width <= 0 || field.height <= 0 {
        return None;
    }
    if field.height <= SINGLE_LINE_MAX {
        return Some(Rect { x: field.x + 4, y: field.y + 2, width: 1, height: (field.height - 4).max(1) });
    }
    empty.then_some(Rect { x: field.x + 4, y: field.y + 4, width: 1, height: 20 })
}

/// Picks the actor of the focused window among Gala's visible actors
/// (`(depth, rect)` in stacking order, bottom first): the topmost one at the
/// shallowest depth whose size equals the window surface, or else (toolkits
/// that scale their window size, like Chromium with text scaling) one that
/// surrounds the frame Gala reports with the surface's aspect ratio.
///
/// A toolkit that reports exactly the frame (GTK 4) in a window with a
/// shadow has no matching actor: its frame sits inside the actor at an
/// unknown, uneven inset, so no position is better than a wrong one.
fn pick_window_actor(actors: &[(usize, Rect)], surface: (i32, i32), frame: Option<(i32, i32)>) -> Option<Rect> {
    let frame = frame.filter(|f| *f != surface);
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
    fn chromium_controls_are_unscaled_web_pages_are_not() {
        // address bar reported at (249, 70) in a window scaled by 1.2, actor at (-22, 8)
        assert_eq!(to_screen(r(253, 72, 1, 20), 1.2, (-22, 8)), r(189, 68, 1, 17));
        // web page caret: unscaled
        assert_eq!(to_screen(r(128, 197, 1, 20), 1.0, (-22, 8)), r(106, 205, 1, 20));
    }

    #[test]
    fn input_method_positions_map_to_the_screen() {
        // GTK: relative to the window surface (Gala's actor), unscaled
        let gtk = WindowGeometry { actor: r(661, 355, 598, 426), scale: 1.0, x11: false };
        assert_eq!(map_reported(r(84, 77, 0, 13), true, &gtk), r(745, 432, 1, 13));
        // Chromium through Gala: screen positions scaled by 1.2 around the window's origin
        let chrome = WindowGeometry { actor: r(0, 30, 1921, 1051), scale: 1.2, x11: false };
        assert_eq!(map_reported(r(931, 1163, 0, 18), false, &chrome), r(776, 974, 1, 15));
        // X11 clients: screen positions as they are
        let x11 = WindowGeometry { actor: r(0, 0, 0, 0), scale: 1.0, x11: true };
        assert_eq!(map_reported(r(300, 400, 2, 18), false, &x11), r(300, 400, 2, 18));
    }

    #[test]
    fn frames_without_their_shadow_find_their_window() {
        // a floating GTK 4 window: 797x778 frame inside an 851x832 actor
        assert_eq!(actor_around_frame(&desktop(), (797, 778)), Some(r(743, 117, 851, 832)));
        assert_eq!(actor_around_frame(&desktop(), (300, 200)), None);
    }

    #[test]
    fn positions_outside_the_focused_window_are_rejected() {
        let window = r(100, 100, 800, 600);
        assert!(within(window, r(500, 300, 1, 18), 8));
        assert!(!within(window, r(1500, 300, 1, 18), 8));
    }

    #[test]
    fn window_names_match_titles() {
        assert!(same_window("New Tab - Google Chrome - Sanjit", "New Tab - Google Chrome"));
        assert!(same_window("Lummo caret test GTK3", "Lummo caret test GTK3"));
        assert!(!same_window("Terminal", "Spotlight - Visual Studio Code"));
        assert!(!same_window("", "Files"));
    }

    #[test]
    fn empty_fields_put_the_caret_at_their_start() {
        assert_eq!(field_caret(r(10, 10, 200, 30), true), Some(r(14, 12, 1, 26)));
        assert_eq!(field_caret(r(10, 10, 400, 300), true), Some(r(14, 14, 1, 20)));
        assert_eq!(field_caret(r(10, 10, 0, 0), true), None);
    }

    #[test]
    fn fields_without_character_positions() {
        // GTK 4.14 search entry with text: the caret's line is the field
        assert_eq!(field_caret(r(629, 7, 665, 24), false), Some(r(633, 9, 1, 20)));
        // multi-line text with an unknown caret position
        assert_eq!(field_caret(r(10, 10, 400, 300), false), None);
    }

    #[test]
    fn gtk4_frames_need_an_exact_actor() {
        // maximized GTK 4 window: the frame is the actor
        let mut actors = desktop();
        actors.push((3, r(0, 30, 1920, 1050)));
        assert_eq!(pick_window_actor(&actors, (1920, 1050), Some((1920, 1050))), Some(r(0, 30, 1920, 1050)));
        // floating GTK 4 window with a shadow: 797x778 frame in an 851x832 actor
        assert_eq!(pick_window_actor(&desktop(), (797, 778), Some((797, 778))), None);
    }
}
