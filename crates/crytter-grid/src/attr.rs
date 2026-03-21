/// Terminal text color.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Color {
    /// Default foreground/background.
    Default,
    /// One of the 16 ANSI colors (0-15).
    Indexed(u8),
    /// 24-bit RGB color.
    Rgb(u8, u8, u8),
}

impl Default for Color {
    fn default() -> Self {
        Color::Default
    }
}

/// Cell attributes (style + color).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Attr {
    pub fg: Color,
    pub bg: Color,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub blink: bool,
    pub inverse: bool,
    pub hidden: bool,
    pub strikethrough: bool,
}

impl Default for Attr {
    fn default() -> Self {
        Self {
            fg: Color::Default,
            bg: Color::Default,
            bold: false,
            dim: false,
            italic: false,
            underline: false,
            blink: false,
            inverse: false,
            hidden: false,
            strikethrough: false,
        }
    }
}

impl Attr {
    /// Reset all attributes to default.
    pub fn reset(&mut self) {
        *self = Self::default();
    }
}
