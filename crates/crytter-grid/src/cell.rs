use crate::attr::Attr;

/// A single cell in the terminal grid.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    /// The character displayed in this cell.
    pub c: char,
    /// Visual attributes (color, bold, etc.).
    pub attr: Attr,
    /// Width of this cell (1 for normal, 2 for wide chars).
    /// A wide char occupies this cell (width=2) and the next cell (width=0).
    pub width: u8,
    /// Whether this cell has been modified since last render.
    pub dirty: bool,
}

impl Default for Cell {
    fn default() -> Self {
        Self {
            c: ' ',
            attr: Attr::default(),
            width: 1,
            dirty: true,
        }
    }
}

impl Cell {
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}
