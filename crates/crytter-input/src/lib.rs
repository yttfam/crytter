//! Keyboard input → escape sequence mapping for crytter.
//!
//! Maps browser key names and modifiers to terminal escape sequences.

mod keymap;

pub use keymap::encode_key;
