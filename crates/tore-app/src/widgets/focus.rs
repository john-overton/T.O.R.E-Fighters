//! Keyboard focus: Tab and Shift+Tab walk the widgets of a screen in the
//! order the screen gives, Enter reaches the focused widget or the default
//! button.

/// What a key means for the screen after [`Focus::key`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Route<Id> {
    /// Tab or Shift+Tab moved the focus.
    Moved,
    /// Give the key to this widget (the focused one).
    Widget(Id),
    /// Enter with nothing focused: press this default button.
    Default(Id),
    /// Not a focus key and nothing is focused (or Esc, the screen's own).
    Ignored,
}

/// The focus order and who has it. Widgets are named by the screen's own ids
/// (an enum or constants). Nothing has the focus until the first Tab or an
/// explicit [`Focus::set`] (a click), so a keyboard that is never touched
/// shows no focus mark ([`Focus::visible`]).
///
/// Rules (*agent decisions*):
/// - Tab goes forward and Shift+Tab back, wrapping round; widgets the
///   screen calls unusable (disabled) are skipped.
/// - Enter goes to the focused widget as a [`Route::Widget`]; a widget that
///   answers `Activated` to Enter (a text field, a list) leaves the screen to
///   press the default button, which [`Focus::default_button`] names. With
///   nothing focused Enter is [`Route::Default`].
/// - Esc is never routed here: it is the screen's.
/// - A click on a widget gives it the focus (`set`), and the mark shows only
///   once the keyboard has been used, as the menu's own buttons do.
#[derive(Clone, Debug)]
pub struct Focus<Id> {
    order: Vec<Id>,
    default: Option<Id>,
    current: Option<Id>,
    keyboard: bool,
}

impl<Id: Copy + PartialEq> Focus<Id> {
    pub fn new(order: Vec<Id>, default: Option<Id>) -> Self {
        Self {
            order,
            default,
            current: None,
            keyboard: false,
        }
    }
    pub fn current(&self) -> Option<Id> {
        self.current
    }
    pub fn default_button(&self) -> Option<Id> {
        self.default
    }
    pub fn is(&self, id: Id) -> bool {
        self.current == Some(id)
    }
    /// True when `id` should draw its focus mark: it has the focus and the
    /// keyboard has been used since the screen opened.
    pub fn marked(&self, id: Id) -> bool {
        self.keyboard && self.is(id)
    }
    /// Gives `id` the focus (a click). The mark stays hidden unless the
    /// keyboard has already been used.
    pub fn set(&mut self, id: Id) {
        if self.order.contains(&id) {
            self.current = Some(id);
        }
    }
    pub fn clear(&mut self) {
        self.current = None;
        self.keyboard = false;
    }

    /// Moves the focus to the next (or previous) widget `usable` accepts,
    /// wrapping. Returns false when none is usable.
    pub fn step(&mut self, forward: bool, usable: impl Fn(Id) -> bool) -> bool {
        self.keyboard = true;
        let n = self.order.len();
        let at = self
            .current
            .and_then(|id| self.order.iter().position(|x| *x == id));
        for k in 1..=n {
            let i = match (at, forward) {
                (Some(i), true) => (i + k) % n,
                (Some(i), false) => (i + n - k % n) % n,
                (None, true) => k - 1,
                (None, false) => n - k,
            };
            if usable(self.order[i]) {
                self.current = Some(self.order[i]);
                return true;
            }
        }
        false
    }

    /// Routes a key press by the window's name for it. `usable` says which
    /// widgets can take the focus.
    pub fn key(&mut self, name: &str, shift: bool, usable: impl Fn(Id) -> bool) -> Route<Id> {
        match name {
            "Tab" => {
                self.step(!shift, usable);
                Route::Moved
            }
            "Enter" => match self.current {
                Some(id) => {
                    self.keyboard = true;
                    Route::Widget(id)
                }
                None => self.default.map_or(Route::Ignored, Route::Default),
            },
            "Escape" => Route::Ignored,
            _ => match self.current {
                Some(id) => {
                    self.keyboard = true;
                    Route::Widget(id)
                }
                None => Route::Ignored,
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum W {
        Name,
        Address,
        List,
        Join,
        Cancel,
    }
    const ALL: fn(W) -> bool = |_| true;

    fn focus() -> Focus<W> {
        Focus::new(
            vec![W::Name, W::Address, W::List, W::Join, W::Cancel],
            Some(W::Join),
        )
    }

    #[test]
    fn tab_walks_forward_in_the_screens_order_and_wraps() {
        let mut f = focus();
        assert_eq!(f.current(), None);
        let mut seen = Vec::new();
        for _ in 0..7 {
            assert_eq!(f.key("Tab", false, ALL), Route::Moved);
            seen.push(f.current().unwrap());
        }
        assert_eq!(
            seen,
            [
                W::Name,
                W::Address,
                W::List,
                W::Join,
                W::Cancel,
                W::Name,
                W::Address
            ]
        );
    }

    #[test]
    fn shift_tab_walks_back_and_starts_at_the_last() {
        let mut f = focus();
        f.key("Tab", true, ALL);
        assert_eq!(f.current(), Some(W::Cancel));
        f.key("Tab", true, ALL);
        assert_eq!(f.current(), Some(W::Join));
        f.set(W::Name);
        f.key("Tab", true, ALL);
        assert_eq!(f.current(), Some(W::Cancel), "wraps backward");
    }

    #[test]
    fn disabled_widgets_are_skipped() {
        let mut f = focus();
        let usable = |w: W| w != W::Address && w != W::Join;
        let mut seen = Vec::new();
        for _ in 0..4 {
            f.key("Tab", false, usable);
            seen.push(f.current().unwrap());
        }
        assert_eq!(seen, [W::Name, W::List, W::Cancel, W::Name]);
        assert!(!f.step(true, |_| false), "nothing usable");
    }

    #[test]
    fn enter_goes_to_the_focused_widget_or_the_default_button() {
        let mut f = focus();
        assert_eq!(f.key("Enter", false, ALL), Route::Default(W::Join));
        f.set(W::Name);
        assert_eq!(f.key("Enter", false, ALL), Route::Widget(W::Name));
        let no_default: Focus<W> = Focus::new(vec![W::Name], None);
        let mut no_default = no_default;
        assert_eq!(no_default.key("Enter", false, ALL), Route::Ignored);
    }

    #[test]
    fn escape_is_the_screens_and_other_keys_follow_the_focus() {
        let mut f = focus();
        assert_eq!(f.key("Escape", false, ALL), Route::Ignored);
        assert_eq!(
            f.key("ArrowLeft", false, ALL),
            Route::Ignored,
            "nothing focused"
        );
        f.set(W::Address);
        assert_eq!(
            f.key("Escape", false, ALL),
            Route::Ignored,
            "even with a focus"
        );
        assert_eq!(f.key("ArrowLeft", false, ALL), Route::Widget(W::Address));
        assert_eq!(f.key("Backspace", false, ALL), Route::Widget(W::Address));
    }

    #[test]
    fn the_mark_shows_only_once_the_keyboard_is_used() {
        let mut f = focus();
        f.set(W::List);
        assert!(
            f.is(W::List) && !f.marked(W::List),
            "a click focuses without a mark"
        );
        f.key("Tab", false, ALL);
        assert!(f.marked(W::Join) && !f.marked(W::List));
        f.clear();
        assert_eq!(f.current(), None);
        f.set(W::Name);
        assert!(!f.marked(W::Name));
        f.set(W::Name);
        let mut other = Focus::new(vec![W::Name], None);
        other.set(W::Cancel);
        assert_eq!(
            other.current(),
            None,
            "a widget the screen did not list cannot take it"
        );
    }
}
