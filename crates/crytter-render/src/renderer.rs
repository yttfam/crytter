use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d, HtmlCanvasElement};

use crytter_grid::{Cell, Cursor, Terminal};
use crytter_grid::selection::Selection;

use crate::palette::{ColorCache, Theme};

/// Canvas2D terminal renderer.
pub struct Renderer {
    canvas: HtmlCanvasElement,
    ctx: CanvasRenderingContext2d,
    cell_width: f64,
    cell_height: f64,
    baseline_offset: f64,
    font: String,
    font_bold: String,
    font_italic: String,
    font_bold_italic: String,
    theme: Theme,
    dpr: f64,
    scroll_offset: usize,
    colors: ColorCache,
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
        let font_bold = format!("bold {font_size}px {font_family}");
        let font_italic = format!("italic {font_size}px {font_family}");
        let font_bold_italic = format!("bold italic {font_size}px {font_family}");

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
            font_bold,
            font_italic,
            font_bold_italic,
            theme,
            dpr,
            scroll_offset: 0,
            colors: ColorCache::new(),
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

    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    pub fn scroll_up(&mut self, lines: usize, max_scrollback: usize) -> usize {
        self.scroll_offset = (self.scroll_offset + lines).min(max_scrollback);
        self.scroll_offset
    }

    pub fn scroll_down(&mut self, lines: usize) -> usize {
        self.scroll_offset = self.scroll_offset.saturating_sub(lines);
        self.scroll_offset
    }

    pub fn scroll_to_bottom(&mut self) {
        self.scroll_offset = 0;
    }

    pub fn draw(&mut self, terminal: &Terminal) {
        self.draw_full(terminal, true, None);
    }

    pub fn draw_with_cursor(&mut self, terminal: &Terminal, cursor_phase: bool) {
        self.draw_full(terminal, cursor_phase, None);
    }

    pub fn draw_with_selection(
        &mut self,
        terminal: &Terminal,
        cursor_phase: bool,
        selection: Option<&Selection>,
    ) {
        self.draw_full(terminal, cursor_phase, selection);
    }

    fn draw_full(
        &mut self,
        terminal: &Terminal,
        cursor_phase: bool,
        selection: Option<&Selection>,
    ) {
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

        self.scroll_offset = self.scroll_offset.min(scrollback_len);

        // Track current font/color to avoid redundant state changes
        let mut cur_font: u8 = 0; // 0=normal, 1=bold, 2=italic, 3=bold+italic
        let mut cur_fill = String::new();
        self.ctx.set_font(&self.font);

        if self.scroll_offset == 0 {
            for row in 0..rows {
                for col in 0..cols {
                    let cell = grid.cell(row, col);
                    self.draw_cell_fast(cell, row, col, &mut cur_font, &mut cur_fill);
                }
            }

            if cursor.visible && cursor_phase {
                self.draw_cursor(cursor);
            }
        } else {
            let sb_start = scrollback_len.saturating_sub(self.scroll_offset);

            for screen_row in 0..rows {
                let sb_idx = sb_start + screen_row;

                if sb_idx < scrollback_len {
                    if let Some(line) = grid.scrollback_line(sb_idx) {
                        for col in 0..cols {
                            if col < line.len() {
                                self.draw_cell_fast(&line[col], screen_row, col, &mut cur_font, &mut cur_fill);
                            }
                        }
                    }
                } else {
                    let grid_row = sb_idx - scrollback_len;
                    if grid_row < rows {
                        for col in 0..cols {
                            let cell = grid.cell(grid_row, col);
                            self.draw_cell_fast(cell, screen_row, col, &mut cur_font, &mut cur_fill);
                        }
                    }
                }
            }

            self.draw_scroll_indicator(self.scroll_offset, scrollback_len);
        }

        // Selection overlay
        if let Some(sel) = selection {
            self.draw_selection(sel, rows, cols);
        }
    }

    /// Draw selection highlight overlay.
    fn draw_selection(&self, selection: &Selection, rows: usize, cols: usize) {
        if !selection.is_active() {
            return;
        }
        self.ctx.set_fill_style_str(&self.theme.selection_bg);
        for row in 0..rows {
            for col in 0..cols {
                if selection.contains(row, col) {
                    let x = col as f64 * self.cell_width;
                    let y = row as f64 * self.cell_height;
                    self.ctx.fill_rect(x, y, self.cell_width, self.cell_height);
                }
            }
        }
    }

    /// Fast cell drawing with state change minimization.
    fn draw_cell_fast(
        &self,
        cell: &Cell,
        row: usize,
        col: usize,
        cur_font: &mut u8,
        cur_fill: &mut String,
    ) {
        // Skip spacer cells (second half of wide chars)
        if cell.width == 0 {
            return;
        }

        let x = col as f64 * self.cell_width;
        let y = row as f64 * self.cell_height;
        let draw_width = if cell.width == 2 {
            self.cell_width * 2.0
        } else {
            self.cell_width
        };

        let (fg, bg) = if cell.attr.inverse {
            (cell.attr.bg, cell.attr.fg)
        } else {
            (cell.attr.fg, cell.attr.bg)
        };

        // Background — only draw if non-default
        let bg_css = self.colors.resolve(bg, &self.theme.background);
        if bg_css.as_ref() != &self.theme.background {
            self.ctx.set_fill_style_str(&bg_css);
            self.ctx.fill_rect(x, y, draw_width, self.cell_height);
            *cur_fill = String::new();
        }

        // Character
        if cell.c != ' ' && cell.c != '\0' && !cell.attr.hidden {
            let fg_css = self.colors.resolve(fg, &self.theme.foreground);

            // Minimize fill style changes
            if fg_css.as_ref() != cur_fill.as_str() {
                self.ctx.set_fill_style_str(&fg_css);
                *cur_fill = fg_css.into_owned();
            }

            // Minimize font changes
            let want_font = match (cell.attr.bold, cell.attr.italic) {
                (true, true) => 3,
                (true, false) => 1,
                (false, true) => 2,
                (false, false) => 0,
            };
            if want_font != *cur_font {
                let f = match want_font {
                    1 => &self.font_bold,
                    2 => &self.font_italic,
                    3 => &self.font_bold_italic,
                    _ => &self.font,
                };
                self.ctx.set_font(f);
                *cur_font = want_font;
            }

            let text_y = y + self.baseline_offset;
            let mut buf = [0u8; 4];
            let s = cell.c.encode_utf8(&mut buf);
            self.ctx.fill_text(s, x, text_y).unwrap();
        }

        // Decorations (underline / strikethrough) — relatively rare
        if cell.attr.underline {
            let fg_css = self.colors.resolve(fg, &self.theme.foreground);
            self.ctx.set_stroke_style_str(&fg_css);
            self.ctx.set_line_width(1.0);
            self.ctx.begin_path();
            let uy = y + self.cell_height - 1.0;
            self.ctx.move_to(x, uy);
            self.ctx.line_to(x + self.cell_width, uy);
            self.ctx.stroke();
        }

        if cell.attr.strikethrough {
            let fg_css = self.colors.resolve(fg, &self.theme.foreground);
            self.ctx.set_stroke_style_str(&fg_css);
            self.ctx.set_line_width(1.0);
            self.ctx.begin_path();
            let sy = y + self.cell_height / 2.0;
            self.ctx.move_to(x, sy);
            self.ctx.line_to(x + self.cell_width, sy);
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
                self.ctx.fill_rect(x, y, self.cell_width, self.cell_height);
                self.ctx.set_global_alpha(1.0);
            }
            crytter_grid::CursorShape::Underline => {
                let uy = y + self.cell_height - 2.0;
                self.ctx.fill_rect(x, uy, self.cell_width, 2.0);
            }
            crytter_grid::CursorShape::Bar => {
                self.ctx.fill_rect(x, y, 2.0, self.cell_height);
            }
        }
    }

    fn draw_scroll_indicator(&self, offset: usize, total: usize) {
        let text = format!("↑{offset}/{total}");
        let w = self.canvas.client_width() as f64;

        self.ctx.set_font(&self.font);
        let metrics = self.ctx.measure_text(&text).unwrap();
        let text_w = metrics.width();
        let padding = 6.0;
        let x = w - text_w - padding * 2.0;

        self.ctx.set_fill_style_str("rgba(255,255,255,0.15)");
        self.ctx.fill_rect(x, 2.0, text_w + padding * 2.0, self.cell_height);

        self.ctx.set_fill_style_str("#d4d4d4");
        self.ctx.fill_text(&text, x + padding, self.baseline_offset).unwrap();
    }

    /// Convert pixel coordinates to grid (row, col).
    pub fn pixel_to_grid(&self, px_x: f64, px_y: f64) -> (usize, usize) {
        let cw = if self.cell_width > 0.0 { self.cell_width } else { 8.0 };
        let ch = if self.cell_height > 0.0 { self.cell_height } else { 16.0 };
        let col = (px_x / cw).floor().max(0.0) as usize;
        let row = (px_y / ch).floor().max(0.0) as usize;
        (row, col)
    }

    pub fn set_theme(&mut self, theme: Theme) {
        self.theme = theme;
    }
}
