//! ANSI color palette → CSS color string conversion.
//!
//! Pure logic, no web-sys dependency. Fully testable.

use crytter_grid::Color;

/// The 16 standard ANSI colors as RGB tuples.
/// Matches xterm defaults.
const ANSI_16: [(u8, u8, u8); 16] = [
    (0, 0, 0),       // 0  black
    (205, 0, 0),     // 1  red
    (0, 205, 0),     // 2  green
    (205, 205, 0),   // 3  yellow
    (0, 0, 238),     // 4  blue
    (205, 0, 205),   // 5  magenta
    (0, 205, 205),   // 6  cyan
    (229, 229, 229), // 7  white
    (127, 127, 127), // 8  bright black
    (255, 0, 0),     // 9  bright red
    (0, 255, 0),     // 10 bright green
    (255, 255, 0),   // 11 bright yellow
    (92, 92, 255),   // 12 bright blue
    (255, 0, 255),   // 13 bright magenta
    (0, 255, 255),   // 14 bright cyan
    (255, 255, 255), // 15 bright white
];

/// Convert an indexed color (0-255) to RGB.
pub fn indexed_to_rgb(idx: u8) -> (u8, u8, u8) {
    if idx < 16 {
        ANSI_16[idx as usize]
    } else if idx < 232 {
        // 6x6x6 color cube (indices 16-231)
        let idx = idx - 16;
        let b = idx % 6;
        let g = (idx / 6) % 6;
        let r = idx / 36;
        let to_val = |c: u8| if c == 0 { 0 } else { 55 + 40 * c };
        (to_val(r), to_val(g), to_val(b))
    } else {
        // Grayscale ramp (indices 232-255)
        let v = 8 + 10 * (idx - 232);
        (v, v, v)
    }
}

/// Convert a terminal `Color` to a CSS color string.
/// `default_color` is used for `Color::Default`.
pub fn color_to_css(color: Color, default_color: &str) -> String {
    match color {
        Color::Default => default_color.to_string(),
        Color::Indexed(idx) => {
            let (r, g, b) = indexed_to_rgb(idx);
            format!("rgb({r},{g},{b})")
        }
        Color::Rgb(r, g, b) => format!("rgb({r},{g},{b})"),
    }
}

/// Theme colors for the terminal.
#[derive(Debug, Clone)]
pub struct Theme {
    pub foreground: String,
    pub background: String,
    pub cursor_color: String,
    pub selection_bg: String,
    pub selection_fg: String,
}

impl Default for Theme {
    fn default() -> Self {
        Self {
            foreground: "#d4d4d4".to_string(),
            background: "#1e1e1e".to_string(),
            cursor_color: "#d4d4d4".to_string(),
            selection_bg: "rgba(68,138,255,0.35)".to_string(),
            selection_fg: "#ffffff".to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ansi_16_colors() {
        assert_eq!(indexed_to_rgb(0), (0, 0, 0));
        assert_eq!(indexed_to_rgb(1), (205, 0, 0));
        assert_eq!(indexed_to_rgb(15), (255, 255, 255));
    }

    #[test]
    fn color_cube() {
        // Index 16 = rgb(0,0,0) in cube
        assert_eq!(indexed_to_rgb(16), (0, 0, 0));
        // Index 196 = bright red in cube: r=5, g=0, b=0
        // 196 - 16 = 180; 180/36=5, (180%36)/6=0, 180%6=0
        assert_eq!(indexed_to_rgb(196), (255, 0, 0));
        // Index 231 = white in cube: r=5, g=5, b=5
        assert_eq!(indexed_to_rgb(231), (255, 255, 255));
    }

    #[test]
    fn grayscale_ramp() {
        assert_eq!(indexed_to_rgb(232), (8, 8, 8));
        assert_eq!(indexed_to_rgb(255), (238, 238, 238));
    }

    #[test]
    fn color_to_css_variants() {
        assert_eq!(color_to_css(Color::Default, "#fff"), "#fff");
        assert_eq!(color_to_css(Color::Indexed(1), "#fff"), "rgb(205,0,0)");
        assert_eq!(color_to_css(Color::Rgb(10, 20, 30), "#fff"), "rgb(10,20,30)");
    }
}
