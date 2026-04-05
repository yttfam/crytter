use crate::grid::Grid;

/// A search match in the terminal grid or scrollback.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchMatch {
    /// Row index. Negative values (represented as offset from scrollback start)
    /// are in scrollback; positive are in the visible grid.
    pub row: i64,
    pub start_col: usize,
    pub end_col: usize,
}

/// Search the grid and scrollback for a needle string.
/// Returns matches in order from top (oldest scrollback) to bottom (latest grid row).
/// `max_results` caps the returned matches to prevent OOM on huge scrollback.
pub fn search(grid: &Grid, needle: &str, max_results: usize) -> Vec<SearchMatch> {
    if needle.is_empty() {
        return Vec::new();
    }

    let needle_lower = needle.to_ascii_lowercase();
    let mut matches = Vec::new();

    // Search scrollback (row indices are negative: -scrollback_len .. -1)
    let sb_len = grid.scrollback_len();
    for i in 0..sb_len {
        if matches.len() >= max_results {
            break;
        }
        if let Some(line) = grid.scrollback_line(i) {
            let text: String = line.iter().map(|c| c.c).collect();
            let text_lower = text.to_ascii_lowercase();
            find_in_line(&text_lower, &needle_lower, -(sb_len as i64) + i as i64, &mut matches, max_results);
        }
    }

    // Search visible grid
    let rows = grid.rows();
    let cols = grid.cols();
    for row in 0..rows {
        if matches.len() >= max_results {
            break;
        }
        let text: String = (0..cols).map(|col| grid.cell(row, col).c).collect();
        let text_lower = text.to_ascii_lowercase();
        find_in_line(&text_lower, &needle_lower, row as i64, &mut matches, max_results);
    }

    matches
}

fn find_in_line(
    text: &str,
    needle: &str,
    row: i64,
    matches: &mut Vec<SearchMatch>,
    max: usize,
) {
    let mut start = 0;
    while let Some(idx) = text[start..].find(needle) {
        if matches.len() >= max {
            break;
        }
        let abs_idx = start + idx;
        // Convert byte offset to char offset
        let start_col = text[..abs_idx].chars().count();
        let end_col = start_col + needle.chars().count();
        matches.push(SearchMatch {
            row,
            start_col,
            end_col,
        });
        start = abs_idx + needle.len();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Grid;

    #[test]
    fn search_visible_grid() {
        let mut grid = Grid::new(80, 24);
        // Write "hello world" into row 0
        for (i, c) in "hello world".chars().enumerate() {
            if let Some(cell) = grid.cell_mut(0, i) {
                cell.c = c;
            }
        }
        let results = search(&grid, "world", 100);
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].row, 0);
        assert_eq!(results[0].start_col, 6);
        assert_eq!(results[0].end_col, 11);
    }

    #[test]
    fn search_case_insensitive() {
        let mut grid = Grid::new(80, 24);
        for (i, c) in "Hello World".chars().enumerate() {
            if let Some(cell) = grid.cell_mut(0, i) {
                cell.c = c;
            }
        }
        let results = search(&grid, "hello", 100);
        assert_eq!(results.len(), 1);
    }

    #[test]
    fn search_no_match() {
        let grid = Grid::new(80, 24);
        let results = search(&grid, "xyz", 100);
        assert!(results.is_empty());
    }

    #[test]
    fn search_empty_needle() {
        let grid = Grid::new(80, 24);
        let results = search(&grid, "", 100);
        assert!(results.is_empty());
    }

    #[test]
    fn search_scrollback() {
        let mut grid = Grid::new(80, 3);
        // Write text then scroll it off
        for (i, c) in "find me in scrollback".chars().enumerate() {
            if let Some(cell) = grid.cell_mut(0, i) {
                cell.c = c;
            }
        }
        grid.scroll_up(0, 3); // push row 0 into scrollback
        let results = search(&grid, "scrollback", 100);
        assert_eq!(results.len(), 1);
        assert!(results[0].row < 0); // in scrollback
    }

    #[test]
    fn search_max_results() {
        let mut grid = Grid::new(80, 24);
        for row in 0..10 {
            for (i, c) in "match".chars().enumerate() {
                if let Some(cell) = grid.cell_mut(row, i) {
                    cell.c = c;
                }
            }
        }
        let results = search(&grid, "match", 3);
        assert_eq!(results.len(), 3);
    }
}
