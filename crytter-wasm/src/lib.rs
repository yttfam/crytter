use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use crytter_grid::Terminal as GridTerminal;
#[cfg(feature = "links")]
use crytter_grid::links;
#[cfg(feature = "search")]
use crytter_grid::search;
use crytter_grid::selection::Selection;
use crytter_input::encode_key;
use crytter_render::{Renderer, Theme};
use crytter_vte::Parser;

/// xterm.js-compatible terminal API.
#[wasm_bindgen]
pub struct Terminal {
    grid: GridTerminal,
    parser: Parser,
    renderer: Option<Renderer>,
    on_title: Option<js_sys::Function>,
    dirty: bool,
    user_scrolled: bool,
    cursor_visible_phase: bool,
    font_family: String,
    font_size: f64,
    selection: Selection,
}

#[wasm_bindgen]
impl Terminal {
    #[wasm_bindgen(constructor)]
    pub fn new(options: Option<js_sys::Object>) -> Self {
        let (cols, rows, font_family, font_size) = parse_options(&options);

        Self {
            grid: GridTerminal::new(cols, rows),
            parser: Parser::new(),
            renderer: None,
            on_title: None,
            dirty: false,
            user_scrolled: false,
            cursor_visible_phase: true,
            font_family,
            font_size,
            selection: Selection::new(),
        }
    }

    /// Mount the terminal into a DOM container element.
    pub fn open(&mut self, container: web_sys::HtmlElement) {
        if self.renderer.is_some() {
            return;
        }

        let document = web_sys::window().unwrap().document().unwrap();
        let canvas = document
            .create_element("canvas")
            .unwrap()
            .dyn_into::<HtmlCanvasElement>()
            .unwrap();

        let style = canvas.style();
        style.set_property("width", "100%").unwrap();
        style.set_property("height", "100%").unwrap();
        style.set_property("display", "block").unwrap();

        container.append_child(&canvas).unwrap();

        container.set_tab_index(0);
        let container_style = container.style();
        container_style.set_property("outline", "none").unwrap();

        let mut renderer = Renderer::new(
            canvas,
            &self.font_family,
            self.font_size,
            Theme::default(),
        );

        // Canvas is now in the DOM — set buffer dimensions from actual layout size
        renderer.resize_canvas();
        let (cols, rows) = renderer.measure_grid();
        self.grid.resize(cols, rows);
        self.renderer = Some(renderer);
        self.dirty = true;
    }

    /// Write PTY output data to the terminal. Does NOT render —
    /// call `render()` from a rAF callback to batch multiple writes.
    /// Returns response bytes to send back to the PTY (DA1, CPR, etc), or null.
    pub fn write(&mut self, data: &str) -> Option<String> {
        let actions = self.parser.parse(data.as_bytes());
        let old_title = self.grid.title().to_string();
        self.grid.process(&actions);

        if self.grid.title() != old_title {
            if let Some(ref cb) = self.on_title {
                let title = JsValue::from_str(self.grid.title());
                let _ = cb.call1(&JsValue::NULL, &title);
            }
        }

        self.dirty = true;
        self.collect_responses()
    }

    /// Write raw bytes to the terminal.
    #[wasm_bindgen(js_name = "writeBytes")]
    pub fn write_bytes(&mut self, data: &[u8]) -> Option<String> {
        let actions = self.parser.parse(data);
        let old_title = self.grid.title().to_string();
        self.grid.process(&actions);

        if self.grid.title() != old_title {
            if let Some(ref cb) = self.on_title {
                let title = JsValue::from_str(self.grid.title());
                let _ = cb.call1(&JsValue::NULL, &title);
            }
        }

        self.dirty = true;
        self.collect_responses()
    }

    /// Collect any queued terminal responses into a single string.
    fn collect_responses(&mut self) -> Option<String> {
        let responses = self.grid.drain_responses();
        if responses.is_empty() {
            return None;
        }
        let combined: Vec<u8> = responses.into_iter().flatten().collect();
        Some(combined.iter().map(|&b| char::from(b)).collect())
    }

    /// Register a callback for title changes.
    #[wasm_bindgen(js_name = "onTitleChange")]
    pub fn on_title_change(&mut self, callback: js_sys::Function) {
        self.on_title = Some(callback);
    }

    /// Handle a keyboard event. Returns escape sequence or null.
    #[wasm_bindgen(js_name = "handleKeyEvent")]
    pub fn handle_key_event(&mut self, event: &web_sys::KeyboardEvent) -> Option<String> {
        let key = event.key();
        let ctrl = event.ctrl_key();
        let alt = event.alt_key();
        let shift = event.shift_key();

        if event.meta_key() {
            return None;
        }

        let app_cursor = self.grid.modes().app_cursor;

        let result = encode_key(&key, ctrl, alt, shift, app_cursor)
            .map(|bytes| bytes.iter().map(|&b| char::from(b)).collect());

        if result.is_some() {
            // User is typing — snap to live view and show cursor
            if self.user_scrolled {
                self.scroll_to_bottom();
            }
            self.cursor_visible_phase = true;
            self.dirty = true;
        }

        result
    }

    /// Render if dirty. Call this from requestAnimationFrame.
    /// Returns true if a frame was actually drawn.
    pub fn render(&mut self) -> bool {
        if !self.dirty {
            return false;
        }
        self.dirty = false;

        if let Some(ref mut renderer) = self.renderer {
            if !self.user_scrolled {
                renderer.scroll_to_bottom();
            }
            let sel = if self.selection.is_active() {
                Some(&self.selection)
            } else {
                None
            };
            renderer.draw_with_selection(&self.grid, self.cursor_visible_phase, sel);
        }
        true
    }

    /// Whether the terminal has pending changes to render.
    #[wasm_bindgen(getter, js_name = "needsRender")]
    pub fn needs_render(&self) -> bool {
        self.dirty
    }

    pub fn fit(&mut self) {
        if let Some(ref mut renderer) = self.renderer {
            renderer.resize_canvas();
            let (cols, rows) = renderer.measure_grid();
            if cols != self.grid.cols() || rows != self.grid.rows() {
                self.grid.resize(cols, rows);
            }
            self.dirty = true;
        }
    }

    #[wasm_bindgen(getter)]
    pub fn cols(&self) -> usize {
        self.grid.cols()
    }

    #[wasm_bindgen(getter)]
    pub fn rows(&self) -> usize {
        self.grid.rows()
    }

    pub fn refresh(&mut self) {
        self.dirty = true;
    }

    /// Toggle cursor blink phase. Call from a JS setInterval(~530ms).
    #[wasm_bindgen(js_name = "blinkCursor")]
    pub fn blink_cursor(&mut self) {
        if self.grid.cursor().blinking {
            self.cursor_visible_phase = !self.cursor_visible_phase;
            self.dirty = true;
        }
    }

    /// Dump the grid content as text lines (for debugging).
    #[wasm_bindgen(js_name = "dumpGrid")]
    pub fn dump_grid(&self) -> String {
        let grid = self.grid.grid();
        let rows = self.grid.rows();
        let cols = self.grid.cols();
        let mut out = String::new();
        for r in 0..rows {
            for c in 0..cols {
                let cell = grid.cell(r, c);
                let ch = if cell.c == '\0' { ' ' } else { cell.c };
                out.push(ch);
            }
            // Trim trailing spaces
            let trimmed = out.trim_end();
            out.truncate(trimmed.len());
            out.push('\n');
        }
        out
    }

    pub fn resize(&mut self, cols: usize, rows: usize) {
        self.grid.resize(cols, rows);
        self.dirty = true;
    }

    pub fn reset(&mut self) {
        self.grid.reset();
        self.dirty = true;
    }

    #[wasm_bindgen(js_name = "scrollUp")]
    pub fn scroll_up(&mut self, lines: usize) {
        if let Some(ref mut renderer) = self.renderer {
            let max = self.grid.grid().scrollback_len();
            renderer.scroll_up(lines, max);
            self.user_scrolled = true;
            self.dirty = true;
        }
    }

    #[wasm_bindgen(js_name = "scrollDown")]
    pub fn scroll_down(&mut self, lines: usize) {
        if let Some(ref mut renderer) = self.renderer {
            let offset = renderer.scroll_down(lines);
            if offset == 0 {
                self.user_scrolled = false;
            }
            self.dirty = true;
        }
    }

    #[wasm_bindgen(js_name = "scrollToBottom")]
    pub fn scroll_to_bottom(&mut self) {
        if let Some(ref mut renderer) = self.renderer {
            renderer.scroll_to_bottom();
        }
        self.user_scrolled = false;
        self.dirty = true;
    }

    /// Handle mousedown — start selection.
    #[wasm_bindgen(js_name = "mouseDown")]
    pub fn mouse_down(&mut self, x: f64, y: f64) {
        if let Some(ref renderer) = self.renderer {
            let (row, col) = renderer.pixel_to_grid(x, y);
            self.selection.start(row, col);
            self.dirty = true;
        }
    }

    /// Handle mousemove while button held — extend selection.
    #[wasm_bindgen(js_name = "mouseMove")]
    pub fn mouse_move(&mut self, x: f64, y: f64) {
        if let Some(ref renderer) = self.renderer {
            let (row, col) = renderer.pixel_to_grid(x, y);
            self.selection.update(row, col);
            self.dirty = true;
        }
    }

    /// Handle mouseup — finalize selection.
    #[wasm_bindgen(js_name = "mouseUp")]
    pub fn mouse_up(&mut self) {
        // Selection stays active until cleared by click or new selection
    }

    /// Get the selected text content.
    #[wasm_bindgen(js_name = "getSelection")]
    pub fn get_selection(&self) -> Option<String> {
        let (sr, sc, er, ec) = self.selection.range()?;
        let grid = self.grid.grid();
        let cols = self.grid.cols();
        let mut text = String::new();

        for row in sr..=er {
            let start_col = if row == sr { sc } else { 0 };
            let end_col = if row == er { ec } else { cols };

            for col in start_col..end_col {
                let cell = grid.cell(row, col);
                if cell.width > 0 {
                    text.push(cell.c);
                }
            }

            // Trim trailing spaces on each line
            let trimmed_len = text.trim_end_matches(' ').len();
            text.truncate(trimmed_len);

            if row < er {
                text.push('\n');
            }
        }

        if text.is_empty() { None } else { Some(text) }
    }

    /// Copy selection to clipboard. Call from JS.
    #[wasm_bindgen(js_name = "copySelection")]
    pub fn copy_selection(&self) -> Option<String> {
        self.get_selection()
    }

    /// Clear selection.
    #[wasm_bindgen(js_name = "clearSelection")]
    pub fn clear_selection(&mut self) {
        self.selection.clear();
        self.dirty = true;
    }

    /// Whether there's an active selection.
    #[wasm_bindgen(getter, js_name = "hasSelection")]
    pub fn has_selection(&self) -> bool {
        self.selection.is_active()
    }

    /// Get the URL at pixel position (x, y), if any.
    #[cfg(feature = "links")]
    #[wasm_bindgen(js_name = "getUrlAt")]
    pub fn get_url_at(&self, x: f64, y: f64) -> Option<String> {
        let renderer = self.renderer.as_ref()?;
        let (row, col) = renderer.pixel_to_grid(x, y);
        let grid = self.grid.grid();
        let cols = self.grid.cols();

        let chars: Vec<char> = (0..cols).map(|c| grid.cell(row, c).c).collect();
        let row_links = links::detect_urls(row, &chars);

        row_links
            .into_iter()
            .find(|link| col >= link.start_col && col < link.end_col)
            .map(|link| link.url)
    }

    /// Search grid + scrollback for text. Returns JSON array of matches.
    #[cfg(feature = "search")]
    #[wasm_bindgen(js_name = "search")]
    pub fn search(&self, needle: &str) -> String {
        let grid = self.grid.grid();
        let matches = search::search(grid, needle, 1000);

        let mut json = String::from("[");
        for (i, m) in matches.iter().enumerate() {
            if i > 0 {
                json.push(',');
            }
            json.push_str(&format!(
                "{{\"row\":{},\"startCol\":{},\"endCol\":{}}}",
                m.row, m.start_col, m.end_col
            ));
        }
        json.push(']');
        json
    }

    #[wasm_bindgen(getter, js_name = "isScrolled")]
    pub fn is_scrolled(&self) -> bool {
        self.renderer
            .as_ref()
            .map(|r| r.scroll_offset() > 0)
            .unwrap_or(false)
    }
}

use wasm_bindgen::JsCast;

fn parse_options(options: &Option<js_sys::Object>) -> (usize, usize, String, f64) {
    let mut cols = 80usize;
    let mut rows = 24usize;
    let mut font_family = "Menlo, Monaco, 'Courier New', monospace".to_string();
    let mut font_size = 14.0f64;

    if let Some(opts) = options {
        let obj: &JsValue = opts.as_ref();

        if let Ok(val) = js_sys::Reflect::get(obj, &JsValue::from_str("cols")) {
            if let Some(n) = val.as_f64() {
                if n > 0.0 && n <= 10_000.0 {
                    cols = n as usize;
                }
            }
        }
        if let Ok(val) = js_sys::Reflect::get(obj, &JsValue::from_str("rows")) {
            if let Some(n) = val.as_f64() {
                if n > 0.0 && n <= 10_000.0 {
                    rows = n as usize;
                }
            }
        }
        if let Ok(val) = js_sys::Reflect::get(obj, &JsValue::from_str("fontFamily")) {
            if let Some(s) = val.as_string() {
                font_family = s;
            }
        }
        if let Ok(val) = js_sys::Reflect::get(obj, &JsValue::from_str("fontSize")) {
            if let Some(n) = val.as_f64() {
                if n > 0.0 && n <= 200.0 {
                    font_size = n;
                }
            }
        }
    }

    (cols, rows, font_family, font_size)
}
