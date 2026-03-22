/// A point in grid coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GridPoint {
    pub row: usize,
    pub col: usize,
}

/// Text selection state.
#[derive(Debug, Clone)]
pub struct Selection {
    /// Anchor point (where mousedown happened).
    anchor: Option<GridPoint>,
    /// Current end point (where mouse is now).
    end: Option<GridPoint>,
}

impl Selection {
    pub fn new() -> Self {
        Self {
            anchor: None,
            end: None,
        }
    }

    /// Start a new selection at the given point.
    pub fn start(&mut self, row: usize, col: usize) {
        self.anchor = Some(GridPoint { row, col });
        self.end = Some(GridPoint { row, col });
    }

    /// Update the selection endpoint (mouse drag).
    pub fn update(&mut self, row: usize, col: usize) {
        self.end = Some(GridPoint { row, col });
    }

    /// Clear the selection.
    pub fn clear(&mut self) {
        self.anchor = None;
        self.end = None;
    }

    /// Whether a selection is active.
    pub fn is_active(&self) -> bool {
        if let (Some(a), Some(e)) = (self.anchor, self.end) {
            a != e
        } else {
            false
        }
    }

    /// Get the normalized selection range (start <= end).
    /// Returns (start_row, start_col, end_row, end_col).
    pub fn range(&self) -> Option<(usize, usize, usize, usize)> {
        let (a, e) = (self.anchor?, self.end?);
        if a == e {
            return None;
        }

        let (start, end) = if a.row < e.row || (a.row == e.row && a.col <= e.col) {
            (a, e)
        } else {
            (e, a)
        };

        Some((start.row, start.col, end.row, end.col))
    }

    /// Check if a cell at (row, col) is within the selection.
    pub fn contains(&self, row: usize, col: usize) -> bool {
        let Some((sr, sc, er, ec)) = self.range() else {
            return false;
        };

        if row < sr || row > er {
            return false;
        }
        if row == sr && row == er {
            return col >= sc && col < ec;
        }
        if row == sr {
            return col >= sc;
        }
        if row == er {
            return col < ec;
        }
        true // middle rows are fully selected
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_selection() {
        let sel = Selection::new();
        assert!(!sel.is_active());
        assert_eq!(sel.range(), None);
    }

    #[test]
    fn single_line_selection() {
        let mut sel = Selection::new();
        sel.start(5, 10);
        sel.update(5, 20);
        assert!(sel.is_active());
        assert_eq!(sel.range(), Some((5, 10, 5, 20)));
        assert!(sel.contains(5, 10));
        assert!(sel.contains(5, 15));
        assert!(!sel.contains(5, 20)); // end is exclusive
        assert!(!sel.contains(4, 15));
    }

    #[test]
    fn multiline_selection() {
        let mut sel = Selection::new();
        sel.start(2, 5);
        sel.update(4, 10);
        assert!(sel.contains(2, 5)); // start row, from col 5
        assert!(sel.contains(2, 79)); // start row, rest of line
        assert!(sel.contains(3, 0)); // middle row
        assert!(sel.contains(3, 79)); // middle row
        assert!(sel.contains(4, 0)); // end row
        assert!(sel.contains(4, 9)); // end row
        assert!(!sel.contains(4, 10)); // past end col
    }

    #[test]
    fn reverse_selection() {
        let mut sel = Selection::new();
        sel.start(5, 20);
        sel.update(5, 10);
        // Should normalize to (5,10) → (5,20)
        assert_eq!(sel.range(), Some((5, 10, 5, 20)));
    }
}
