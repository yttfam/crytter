use std::collections::VecDeque;

use crate::cell::Cell;

/// Maximum allowed terminal dimension to prevent absurd allocations.
const MAX_DIMENSION: usize = 10_000;

/// The terminal cell grid with scrollback.
pub struct Grid {
    /// Number of columns.
    cols: usize,
    /// Number of visible rows.
    rows: usize,
    /// The visible buffer: rows × cols.
    cells: Vec<Vec<Cell>>,
    /// Scrollback buffer (oldest first). VecDeque for O(1) pop_front.
    scrollback: VecDeque<Vec<Cell>>,
    /// Maximum scrollback lines.
    max_scrollback: usize,
}

impl Grid {
    pub fn new(cols: usize, rows: usize) -> Self {
        let cols = cols.min(MAX_DIMENSION).max(1);
        let rows = rows.min(MAX_DIMENSION).max(1);

        let cells = (0..rows)
            .map(|_| (0..cols).map(|_| Cell::default()).collect())
            .collect();

        Self {
            cols,
            rows,
            cells,
            scrollback: VecDeque::new(),
            max_scrollback: 10_000,
        }
    }

    pub fn cols(&self) -> usize {
        self.cols
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    /// Get a reference to a cell. Returns default cell for out-of-bounds.
    pub fn cell(&self, row: usize, col: usize) -> &Cell {
        static DEFAULT_CELL: Cell = Cell {
            c: ' ',
            attr: crate::attr::Attr {
                fg: crate::attr::Color::Default,
                bg: crate::attr::Color::Default,
                bold: false,
                dim: false,
                italic: false,
                underline: false,
                blink: false,
                inverse: false,
                hidden: false,
                strikethrough: false,
            },
            width: 1,
            dirty: false,
        };

        self.cells
            .get(row)
            .and_then(|r| r.get(col))
            .unwrap_or(&DEFAULT_CELL)
    }

    /// Get a mutable reference to a cell. Returns None for out-of-bounds.
    pub fn cell_mut(&mut self, row: usize, col: usize) -> Option<&mut Cell> {
        self.cells.get_mut(row).and_then(|r| r.get_mut(col))
    }

    /// Get a reference to an entire row.
    pub fn row(&self, row: usize) -> &[Cell] {
        self.cells.get(row).map(|v| v.as_slice()).unwrap_or(&[])
    }

    /// Scroll the region [top, bottom) up by one line.
    /// If `to_scrollback` is true and top == 0, the top line goes to scrollback.
    pub fn scroll_up_inner(&mut self, top: usize, bottom: usize, to_scrollback: bool) {
        if top >= bottom || bottom > self.rows {
            return;
        }

        // Save to scrollback if scrolling the whole screen
        if to_scrollback && top == 0 {
            let line = self.cells[0].clone();
            self.scrollback.push_back(line);
            if self.scrollback.len() > self.max_scrollback {
                self.scrollback.pop_front();
            }
        }

        // Shift lines up
        for row in top..bottom - 1 {
            self.cells.swap(row, row + 1);
        }

        // Clear the bottom line
        self.clear_row(bottom - 1);
    }

    /// Scroll up with scrollback saving (normal scroll).
    pub fn scroll_up(&mut self, top: usize, bottom: usize) {
        self.scroll_up_inner(top, bottom, true);
    }

    /// Scroll the region [top, bottom) down by one line.
    /// Bottom line is lost, top gets a blank line.
    pub fn scroll_down(&mut self, top: usize, bottom: usize) {
        if top >= bottom || bottom > self.rows {
            return;
        }

        // Shift lines down
        for row in (top + 1..bottom).rev() {
            self.cells.swap(row, row - 1);
        }

        // Clear the top line
        self.clear_row(top);
    }

    /// Clear a single row to default cells.
    fn clear_row(&mut self, row: usize) {
        if let Some(cells) = self.cells.get_mut(row) {
            for cell in cells {
                cell.reset();
            }
        }
    }

    /// Clear all cells in the grid.
    pub fn clear(&mut self) {
        for row in 0..self.rows {
            self.clear_row(row);
        }
    }

    /// Resize the grid. Tries to preserve content.
    pub fn resize(&mut self, new_cols: usize, new_rows: usize) {
        let new_cols = new_cols.min(MAX_DIMENSION).max(1);
        let new_rows = new_rows.min(MAX_DIMENSION).max(1);

        // Adjust rows
        while self.cells.len() < new_rows {
            self.cells
                .push((0..self.cols).map(|_| Cell::default()).collect());
        }
        self.cells.truncate(new_rows);

        // Adjust columns in each row
        for row in &mut self.cells {
            row.resize_with(new_cols, Cell::default);
            row.truncate(new_cols);
        }

        self.cols = new_cols;
        self.rows = new_rows;

        // Mark everything dirty
        for row in &mut self.cells {
            for cell in row {
                cell.dirty = true;
            }
        }
    }

    /// Clear all dirty flags (call after rendering).
    pub fn clear_dirty(&mut self) {
        for row in &mut self.cells {
            for cell in row {
                cell.dirty = false;
            }
        }
    }

    /// Returns the number of scrollback lines.
    pub fn scrollback_len(&self) -> usize {
        self.scrollback.len()
    }

    /// Get a scrollback line (0 = oldest).
    pub fn scrollback_line(&self, index: usize) -> Option<&[Cell]> {
        self.scrollback.get(index).map(|v| v.as_slice())
    }

    /// Clear the scrollback buffer.
    pub fn clear_scrollback(&mut self) {
        self.scrollback.clear();
    }

    /// Erase cells in a row from `start_col` to `end_col` (exclusive).
    pub fn erase_cells(&mut self, row: usize, start_col: usize, end_col: usize) {
        if row >= self.rows {
            return;
        }
        let end = end_col.min(self.cols);
        let start = start_col.min(end);
        for col in start..end {
            self.cells[row][col].reset();
        }
    }

    /// Delete `count` lines starting at `row`, shifting lines below up.
    /// New blank lines appear at `bottom - 1`. Does NOT save to scrollback.
    pub fn delete_lines(&mut self, row: usize, count: usize, bottom: usize) {
        let count = count.min(bottom.saturating_sub(row));
        for _ in 0..count {
            if row < bottom {
                self.scroll_up_inner(row, bottom, false);
            }
        }
    }

    /// Insert `count` blank lines at `row`, shifting lines below down.
    /// Lines that fall off `bottom` are lost.
    pub fn insert_lines(&mut self, row: usize, count: usize, bottom: usize) {
        let count = count.min(bottom.saturating_sub(row));
        for _ in 0..count {
            if row < bottom {
                self.scroll_down(row, bottom);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_grid_dimensions() {
        let grid = Grid::new(80, 24);
        assert_eq!(grid.cols(), 80);
        assert_eq!(grid.rows(), 24);
    }

    #[test]
    fn new_grid_clamps_zero() {
        let grid = Grid::new(0, 0);
        assert_eq!(grid.cols(), 1);
        assert_eq!(grid.rows(), 1);
    }

    #[test]
    fn cell_default_is_space() {
        let grid = Grid::new(80, 24);
        assert_eq!(grid.cell(0, 0).c, ' ');
    }

    #[test]
    fn cell_oob_returns_default() {
        let grid = Grid::new(80, 24);
        assert_eq!(grid.cell(999, 999).c, ' ');
    }

    #[test]
    fn scroll_up_adds_to_scrollback() {
        let mut grid = Grid::new(80, 24);
        grid.cell_mut(0, 0).unwrap().c = 'X';
        grid.scroll_up(0, 24);
        assert_eq!(grid.scrollback_len(), 1);
        assert_eq!(grid.scrollback_line(0).unwrap()[0].c, 'X');
        assert_eq!(grid.cell(0, 0).c, ' ');
    }

    #[test]
    fn resize_preserves_content() {
        let mut grid = Grid::new(80, 24);
        grid.cell_mut(0, 0).unwrap().c = 'A';
        grid.resize(120, 40);
        assert_eq!(grid.cell(0, 0).c, 'A');
        assert_eq!(grid.cols(), 120);
        assert_eq!(grid.rows(), 40);
    }

    #[test]
    fn delete_lines_does_not_save_scrollback() {
        let mut grid = Grid::new(80, 5);
        grid.cell_mut(0, 0).unwrap().c = 'A';
        grid.delete_lines(0, 1, 5);
        assert_eq!(grid.scrollback_len(), 0);
    }

    #[test]
    fn erase_cells_oob_row_is_noop() {
        let mut grid = Grid::new(80, 24);
        grid.erase_cells(999, 0, 80); // should not panic
    }

    #[test]
    fn scrollback_uses_deque() {
        let mut grid = Grid::new(5, 2);
        // Fill scrollback past max
        for i in 0..10_001u32 {
            grid.cell_mut(0, 0).unwrap().c = char::from_u32(b'A' as u32 + (i % 26)).unwrap();
            grid.scroll_up(0, 2);
        }
        assert_eq!(grid.scrollback_len(), 10_000);
    }
}
