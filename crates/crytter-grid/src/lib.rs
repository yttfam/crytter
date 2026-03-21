//! Terminal grid state for crytter.
//!
//! Manages the cell grid, cursor, attributes, scrollback buffer, and
//! processes VTE actions into grid mutations.

mod attr;
mod cell;
mod cursor;
mod grid;
mod term;

pub use attr::{Attr, Color};
pub use cell::Cell;
pub use cursor::{Cursor, CursorShape};
pub use grid::Grid;
pub use term::{Modes, Terminal};
