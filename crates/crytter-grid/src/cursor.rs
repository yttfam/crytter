use crate::attr::Attr;

/// Cursor shape for rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CursorShape {
    Block,
    Underline,
    Bar,
}

impl Default for CursorShape {
    fn default() -> Self {
        CursorShape::Block
    }
}

/// Terminal cursor state.
#[derive(Debug, Clone)]
pub struct Cursor {
    /// Column (0-indexed).
    pub col: usize,
    /// Row (0-indexed).
    pub row: usize,
    /// Current text attributes applied to new characters.
    pub attr: Attr,
    /// Cursor shape.
    pub shape: CursorShape,
    /// Whether the cursor is visible.
    pub visible: bool,
    /// Saved cursor state (for ESC 7 / ESC 8).
    saved: Option<SavedCursor>,
}

#[derive(Debug, Clone)]
struct SavedCursor {
    col: usize,
    row: usize,
    attr: Attr,
}

impl Default for Cursor {
    fn default() -> Self {
        Self {
            col: 0,
            row: 0,
            attr: Attr::default(),
            shape: CursorShape::default(),
            visible: true,
            saved: None,
        }
    }
}

impl Cursor {
    /// Save current cursor position and attributes.
    pub fn save(&mut self) {
        self.saved = Some(SavedCursor {
            col: self.col,
            row: self.row,
            attr: self.attr,
        });
    }

    /// Restore previously saved cursor position and attributes.
    pub fn restore(&mut self) {
        if let Some(saved) = self.saved.take() {
            self.col = saved.col;
            self.row = saved.row;
            self.attr = saved.attr;
        }
    }
}
