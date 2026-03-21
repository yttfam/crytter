use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

use crytter_grid::{Cell, Cursor, Terminal};

use crate::palette::{color_to_css, Theme};

/// Canvas2D terminal renderer.
pub struct Renderer {
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    /// Character cell width in pixels.
    cell_width: f64,
    /// Character cell height in pixels.
    cell_height: f64,
    /// Baseline offset within a cell (for fillText y position).
    baseline_offset: f64,
    /// Font string for Canvas2D.
    font: String,
    /// Color theme.
    theme: Theme,
    /// Device pixel ratio for sharp rendering.
    dpr: f64,
    /// Scroll offset into scrollback (0 = bottom / live view).
    scroll_offset: usize,
}

impl Renderer {
    pub fn new(
        canvas: HtmlCanvasElement,
        font_family: &str,
        font_size: f64,
        theme: Theme,
    ) -> Self {
        let ctx = canvas
            .get_context("2d")
            .unwrap()
            .unwrap()
            .dyn_into::<CanvasRenderingContext2d>()
            .unwrap();

        let dpr = web_sys::window()
            .map(|w| w.device_pixel_ratio())
            .unwrap_or(1.0);

        let font = format!("{font_size}px {font_family}");

        ctx.set_font(&font);
        let metrics = ctx.measure_text("M").unwrap();
        let cell_width = metrics.width();
        let cell_height = (font_size * 1.2).ceil();
        let baseline_offset = font_size;

        let mut r = Self {
            canvas,
            ctx,
            cell_width,
            cell_height,
            baseline_offset,
            font,
            theme,
            dpr,
            scroll_offset: 0,
        };
        r.apply_dpr();
        r
    }

    fn apply_dpr(&mut self) {
        let dpr = self.dpr;
        let w = self.canvas.client_width() as f64;
        let h = self.canvas.client_height() as f64;

        self.canvas.set_width((w * dpr) as u32);
        self.canvas.set_height((h * dpr) as u32);

        self.ctx.scale(dpr, dpr).unwrap();
        self.ctx.set_font(&self.font);
    }

    pub fn measure_grid(&self) -> (usize, usize) {
        let w = self.canvas.client_width() as f64;
        let h = self.canvas.client_height() as f64;
        let cw = if self.cell_width > 0.0 { self.cell_width } else { 8.0 };
        let ch = if self.cell_height > 0.0 { self.cell_height } else { 16.0 };
        let cols = (w / cw).floor() as usize;
        let rows = (h / ch).floor() as usize;
        (cols.max(1), rows.max(1))
    }

    pub fn cell_width(&self) -> f64 {
        self.cell_width
    }

    pub fn cell_height(&self) -> f64 {
        self.cell_height
    }

    pub fn resize_canvas(&mut self) {
        self.apply_dpr();
    }

    /// Current scroll offset (0 = live view at bottom).
    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Scroll up by `lines` lines. Returns the new offset.
    pub fn scroll_up(&mut self, lines: usize, max_scrollback: usize) -> usize {
        self.scroll_offset = (self.scroll_offset + lines).min(max_scrollback);
        self.scroll_offset
    }

    /// Scroll down by `lines` lines. Returns the new offset.
    pub fn scroll_down(&mut self, lines: usize) -> usize {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
        self.scroll_offset
    }

    /// Snap to bottom (live view).
    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
    }

    /// Full render of the terminal grid with scrollback support.
    pub fn draw(&mut self, terminal: &Terminal) {
        let grid = terminal.grid();
        let cursor = terminal.cursor();
        let rows = grid.rows();
        let cols = grid.cols();
        let scrollback_len = grid.scrollback_len();

        // Clear background
        self.ctx.set_fill_style_str(&self.theme.background);
        let w = self.canvas.client_width() as f64;
        let h = self.canvas.client_height() as f64;
        self.ctx.fill_rect(0.0, 0.0, w, h);

        // Clamp scroll offset
        self.scroll_offset = self.scroll_offset.min(scrollback_len);

        if self.scroll_offset == 0 {
            // Live view — draw grid directly
            for row in 0..rows {
                for col in 0..cols {
                    let cell = grid.cell(row, col);
                    self.draw_cell_at(cell, row, col);
                }
            }

            // Draw cursor only in live view
            if cursor.visible {
                self.draw_cursor(cursor);
            }
        } else {
            // Scrolled up — mix scrollback lines and grid lines.
            // scroll_offset=N means we're viewing N lines above the bottom.
            //
            // The visible window shows:
            // - Scrollback lines from (scrollback_len - scroll_offset) onward
            // - Then grid lines from the top, filling the rest
            let sb_start = scrollback_len.saturating_sub(self.scroll_offset);

            for screen_row in 0..rows {
                let sb_idx = sb_start + screen_row;

                if sb_idx < scrollback_len {
                    // This row comes from scrollback
                    if let Some(line) = grid.scrollback_line(sb_idx) {
                        for col in 0..cols {
                            if col < line.len() {
                                self.draw_cell_at(&line[col], screen_row, col);
                            }
                        }
                    }
                } else {
                    // This row comes from the live grid
                    let grid_row = sb_idx - scrollback_len;
                    if grid_row < rows {
                        for col in 0..cols {
                            let cell = grid.cell(grid_row, col);
                            self.draw_cell_at(cell, screen_row, col);
                        }
                    }
                }
            }

            // Draw scroll indicator
            self.draw_scroll_indicator(self.scroll_offset, scrollback_len);
        }
    }

    /// Draw a cell at screen position (row, col).
    fn draw_cell_at(&self, cell: &Cell, row: usize, col: usize) {
        let x = col as f64 * self.cell_width;
        let y = row as f64 * self.cell_height;

        let (fg, bg) = if cell.attr.inverse {
            (cell.attr.bg, cell.attr.fg)
        } else {
            (cell.attr.fg, cell.attr.bg)
        };

        // Draw background
        let bg_css = color_to_css(bg, &self.theme.background);
        self.ctx.set_fill_style_str(&bg_css);
        self.ctx
            .fill_rect(x, y, self.cell_width, self.cell_height);

        // Draw character
        if cell.c != ' ' && cell.c != '\0' && !cell.attr.hidden {
            let fg_css = color_to_css(fg, &self.theme.foreground);
            self.ctx.set_fill_style_str(&fg_css);

            let font = if cell.attr.bold && cell.attr.italic {
                format!("bold italic {}", self.font)
            } else if cell.attr.bold {
                format!("bold {}", self.font)
            } else if cell.attr.italic {
                format!("italic {}", self.font)
            } else {
                self.font.clone()
            };
            self.ctx.set_font(&font);

            let text_y = y + self.baseline_offset;
            let mut buf = [0u8; 4];
            let s = cell.c.encode_utf8(&mut buf);
            self.ctx.fill_text(s, x, text_y).unwrap();

            if cell.attr.bold || cell.attr.italic {
                self.ctx.set_font(&self.font);
            }
        }

        // Underline
        if cell.attr.underline {
            let fg_css = color_to_css(fg, &self.theme.foreground);
            self.ctx.set_stroke_style_str(&fg_css);
            self.ctx.set_line_width(1.0);
            self.ctx.begin_path();
            let underline_y = y + self.cell_height - 1.0;
            self.ctx.move_to(x, underline_y);
            self.ctx.line_to(x + self.cell_width, underline_y);
            self.ctx.stroke();
        }

        // Strikethrough
        if cell.attr.strikethrough {
            let fg_css = color_to_css(fg, &self.theme.foreground);
            self.ctx.set_stroke_style_str(&fg_css);
            self.ctx.set_line_width(1.0);
            self.ctx.begin_path();
            let strike_y = y + self.cell_height / 2.0;
            self.ctx.move_to(x, strike_y);
            self.ctx.line_to(x + self.cell_width, strike_y);
            self.ctx.stroke();
        }
    }

    fn draw_cursor(&self, cursor: &Cursor) {
        let x = cursor.col as f64 * self.cell_width;
        let y = cursor.row as f64 * self.cell_height;

        self.ctx.set_fill_style_str(&self.theme.cursor_color);

        match cursor.shape {
            crytter_grid::CursorShape::Block => {
                self.ctx.set_global_alpha(0.5);
                self.ctx
                    .fill_rect(x, y, self.cell_width, self.cell_height);
                self.ctx.set_global_alpha(1.0);
            }
            crytter_grid::CursorShape::Underline => {
                let underline_y = y + self.cell_height - 2.0;
                self.ctx.fill_rect(x, underline_y, self.cell_width, 2.0);
            }
            crytter_grid::CursorShape::Bar => {
                self.ctx.fill_rect(x, y, 2.0, self.cell_height);
            }
        }
    }

    /// Draw a small scroll position indicator in the top-right.
    fn draw_scroll_indicator(&self, offset: usize, total: usize) {
        let text = format!("↑{offset}/{total}");
        let w = self.canvas.client_width() as f64;

        self.ctx.set_font(&self.font);
        let metrics = self.ctx.measure_text(&text).unwrap();
        let text_w = metrics.width();
        let padding = 6.0;
        let x = w - text_w - padding * 2.0;

        // Background pill
        self.ctx.set_fill_style_str("rgba(255,255,255,0.15)");
        self.ctx.fill_rect(x, 2.0, text_w + padding * 2.0, self.cell_height);

        // Text
        self.ctx.set_fill_style_str("#d4d4d4");
        self.ctx
            .fill_text(&text, x + padding, self.baseline_offset)
            .unwrap();
    }

    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }
}
