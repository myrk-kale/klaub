//! Scrolling bookkeeping for lists and text.

use ratatui::layout::Rect;

/// A cursor inside a scrolled window.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Viewport {
    /// The selected row.
    pub selected: usize,
    /// The first visible row.
    pub offset: usize,
    /// How many rows fit on screen (set by the view each frame).
    pub height: usize,
    /// Where the rows were drawn last, so a mouse click can be turned into a row.
    pub area: Rect,
    /// How many rows the list had when it was drawn last.
    pub len: usize,
}

impl Viewport {
    /// Moves the selection by `delta` rows within `len` rows.
    pub fn move_by(&mut self, delta: isize, len: usize) {
        if len == 0 {
            self.selected = 0;
            return;
        }
        let target = self.selected.saturating_add_signed(delta);
        self.selected = target.min(len - 1);
        self.ensure_visible();
    }

    /// Jumps to a row.
    pub fn select(&mut self, row: usize, len: usize) {
        self.selected = row.min(len.saturating_sub(1));
        self.ensure_visible();
    }

    /// Keeps the selection inside the window.
    pub fn ensure_visible(&mut self) {
        let height = self.height.max(1);
        if self.selected < self.offset {
            self.offset = self.selected;
        } else if self.selected >= self.offset + height {
            self.offset = self.selected + 1 - height;
        }
    }

    /// Clamps after the list length changed.
    pub fn clamp(&mut self, len: usize) {
        self.selected = self.selected.min(len.saturating_sub(1));
        self.offset = self.offset.min(self.selected);
        self.ensure_visible();
    }

    /// The row under a screen position, if the position is on a row of this list.
    #[must_use]
    pub fn row_at(&self, x: u16, y: u16) -> Option<usize> {
        let inside = x >= self.area.x
            && x < self.area.x.saturating_add(self.area.width)
            && y >= self.area.y
            && y < self.area.y.saturating_add(self.area.height);
        if !inside {
            return None;
        }
        let row = self.offset + usize::from(y - self.area.y);
        (row < self.len).then_some(row)
    }

    /// A page of rows (at least one).
    pub fn page(&self) -> isize {
        isize::try_from(self.height.saturating_sub(1).max(1)).unwrap_or(1)
    }

    /// The range of rows to draw out of `len`.
    pub fn window(&self, len: usize) -> std::ops::Range<usize> {
        let start = self.offset.min(len);
        start..(start + self.height.max(1)).min(len)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selection_stays_inside_the_window() {
        let mut view = Viewport {
            height: 3,
            ..Viewport::default()
        };
        for _ in 0..5 {
            view.move_by(1, 10);
        }
        assert_eq!((view.selected, view.offset), (5, 3));
        assert_eq!(view.window(10), 3..6);
        view.move_by(-10, 10);
        assert_eq!((view.selected, view.offset), (0, 0));
        view.move_by(100, 10);
        assert_eq!(view.selected, 9);
    }

    #[test]
    fn a_position_maps_to_the_row_drawn_there() {
        let view = Viewport {
            offset: 10,
            height: 5,
            len: 12,
            area: Rect::new(2, 4, 30, 5),
            ..Viewport::default()
        };
        assert_eq!(view.row_at(2, 4), Some(10));
        assert_eq!(view.row_at(31, 5), Some(11));
        assert_eq!(view.row_at(5, 6), None, "row 12 does not exist");
        assert_eq!(view.row_at(1, 4), None, "left of the list");
        assert_eq!(view.row_at(32, 4), None, "right of the list");
        assert_eq!(view.row_at(5, 3), None, "above the list");
        assert_eq!(view.row_at(5, 9), None, "below the list");
    }

    #[test]
    fn empty_and_shrinking_lists_are_safe() {
        let mut view = Viewport {
            height: 4,
            selected: 7,
            offset: 5,
            ..Viewport::default()
        };
        view.clamp(3);
        assert_eq!((view.selected, view.offset), (2, 2));
        view.move_by(1, 0);
        assert_eq!(view.selected, 0);
        assert_eq!(view.window(0), 0..0);
    }
}
