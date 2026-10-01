//! The retail widget kit (slice EF2): the pieces the multiplayer screens
//! (Direct Connection, the lobby) are built from, drawn into the 640 by 480
//! menu canvas from retail art. The measurements behind them are in
//! `docs/formats/menu.md` ("Multiplayer connection screens", EF0); the design
//! is `docs/ARCHITECTURE.md` "Lobby and hosting".
//!
//! # How a screen uses the kit
//!
//! 1. Build one [`Kit`] when the screen opens: [`Kit::new`] decodes the pieces
//!    in the palette of the picture the screen is named after and the two
//!    connection backgrounds in their own.
//! 2. Make the widgets, each placed by rectangle: [`Button`], [`TextField`],
//!    [`List`], [`MessageBox`], [`CheckBox`], plus a [`Focus`] with the order
//!    Tab visits them in. A widget holds only its own state (text, caret,
//!    selection, page, lines, lamp frame) and knows nothing of the screen.
//! 3. Draw back to front: [`Background::draw`], [`draw_panel`], the labels and
//!    frame lines the screen owns, then each widget's `draw(canvas, kit,
//!    focused)`. Widgets blend over what is under them and stay inside their
//!    own rectangle (and the pieces' own shadows), so the order among them
//!    does not matter. Pass `focus.marked(id)` for `focused`: the dotted focus
//!    mark and the text caret show for the widget the keyboard has.
//! 4. Route events by hand, as every screen here does. The window gives a key
//!    name ("Enter", "Tab", "ArrowLeft", "Backspace"...), the text of the key
//!    press, a pointer position in canvas pixels and a wheel step; a widget
//!    takes the ones it understands and answers an [`Outcome`]: `Changed`
//!    (text edited, selection moved, box ticked), `Activated` (clicked,
//!    double-clicked, Enter) or `None`. Tab, Shift+Tab and Enter go to
//!    [`Focus::key`]; Esc is the screen's.
//!
//! ```ignore
//! match focus.key(name, shift, |id| usable(id)) {
//!     Route::Moved => {}                                  // redraw
//!     Route::Widget(Id::Callsign) => { callsign.key(name); }
//!     Route::Widget(Id::Games) => { games.key(name); }
//!     Route::Default(id) => press(id),                    // Enter, nothing focused
//!     Route::Ignored => {}                                // Esc leaves the screen
//!     ...
//! }
//! // Typed text goes to the focused field, from the window's text event:
//! if let Some(Id::Callsign) = focus.current() { callsign.text_input(text); }
//! // A click: give the widget the focus and hand it the press.
//! if games.hit(p) { focus.set(Id::Games); games.press(p, now); }
//! ```
//!
//! # Look
//!
//! Everything is retail pieces except the focus mark, the row icons and the
//! message box's scroll bar, which are authored (*agent decisions*, in the
//! modules that draw them). No retail bytes are in the code: the kit decodes
//! the imported pictures at runtime, and the tests use synthetic pieces.
//!
//! The kit is not used by a screen yet: EF7 (Direct Connection) and EF8 (the
//! lobby) place it. The tests build every widget from synthetic pieces; the
//! mock NETWORK CONNECTION screen made only of kit widgets, and the sheets
//! of every widget in each state, are ignored tests in `mock_screen.rs` that
//! render from the imported pieces.
mod button;
mod check_box;
mod draw;
mod focus;
mod icons;
mod kit;
mod list;
mod message_box;
mod panel;
mod text_field;

#[cfg(test)]
mod mock_screen;
#[cfg(test)]
pub(crate) mod test_kit;

pub use button::Button;
pub use check_box::CheckBox;
pub use draw::fit;
pub use focus::{Focus, Route};
pub use icons::Icon;
pub use kit::{Kit, KitSource};
pub use list::{Align, Cell, Column, List, Pager, Row};
pub use message_box::{MessageBox, tone};
pub use panel::{Background, draw_panel};
pub use text_field::{Filter, TextField, parse_address};

/// A rectangle on the 640 by 480 canvas: x, y, width, height.
pub type Rect = (i32, i32, i32, i32);
/// A pointer position in canvas pixels.
pub type Point = (i32, i32);

/// What a widget tells its screen after an event. Plain data, in the style of
/// [`crate::menu::Action`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    /// Nothing for the screen to do (the widget may still have changed what
    /// it draws; screens redraw every frame while the menu is up).
    None,
    /// The widget's value changed: text edited, selection or page moved,
    /// check box turned.
    Changed,
    /// The player chose the widget: a button clicked or Entered, a list row
    /// double-clicked or Entered, Enter in a text field. A screen treats a
    /// field's `Activated` as a press of its default button.
    Activated,
}

pub(crate) fn inside((x, y, w, h): Rect, (px, py): Point) -> bool {
    px >= x && py >= y && px < x + w && py < y + h
}

/// What every widget answers for the screen's hit and focus tests.
pub trait Widget {
    /// The rectangle a click counts in.
    fn bounds(&self) -> Rect;
    /// A disabled widget ignores events and is skipped by Tab.
    fn enabled(&self) -> bool {
        true
    }
    fn hit(&self, point: Point) -> bool {
        self.enabled() && inside(self.bounds(), point)
    }
}
