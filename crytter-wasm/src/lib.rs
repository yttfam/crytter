use wasm_bindgen::prelude::*;
use web_sys::HtmlCanvasElement;

use crytter_grid::Terminal as GridTerminal;
use crytter_input::encode_key;
use crytter_render::{Renderer, Theme};
use crytter_vte::Parser;

/// xterm.js-compatible terminal API.
#[wasm_bindgen]
pub struct Terminal {
    grid: GridTerminal,
    parser: Parser,
    renderer: Option<Renderer>,
    on_data: Option<js_sys::Function>,
    on_title: Option<js_sys::Function>,
}

#[wasm_bindgen]
impl Terminal {
    /// Create a new terminal instance.
    /// Options: `cols` (default 80), `rows` (default 24),
    /// `fontFamily`, `fontSize`, `theme`.
    #[wasm_bindgen(constructor)]
    pub fn new(options: Option<js_sys::Object>) -> Self {
        let (cols, rows, _font_family, _font_size) = parse_options(&options);

        Self {
            grid: GridTerminal::new(cols, rows),
            parser: Parser::new(),
            renderer: None,
            on_data: None,
            on_title: None,
        }
    }

    /// Mount the terminal into a DOM container element.
    /// Creates a canvas and starts rendering. Can only be called once.
    pub fn open(&mut self, container: web_sys::HtmlElement) {
        // Prevent double-open leaking canvas elements
        if self.renderer.is_some() {
            return;
        }

        let document = web_sys::window().unwrap().document().unwrap();
        let canvas = document
            .create_element("canvas")
            .unwrap()
            .dyn_into::<HtmlCanvasElement>()
            .unwrap();

        // Style canvas to fill container
        let style = canvas.style();
        style.set_property("width", "100%").unwrap();
        style.set_property("height", "100%").unwrap();
        style.set_property("display", "block").unwrap();

        container.append_child(&canvas).unwrap();

        // Make container focusable for keyboard events
        container.set_tab_index(0);
        let container_style = container.style();
        container_style.set_property("outline", "none").unwrap();

        let renderer = Renderer::new(
            canvas,
            "Menlo, Monaco, 'Courier New', monospace",
            14.0,
            Theme::default(),
        );

        // Auto-fit to canvas size
        let (cols, rows) = renderer.measure_grid();
        self.grid.resize(cols, rows);
        self.renderer = Some(renderer);

        // Initial render
        self.render();
    }

    /// Write PTY output data to the terminal.
    /// Accepts a string or Uint8Array from JavaScript.
    pub fn write(&mut self, data: &str) {
        let actions = self.parser.parse(data.as_bytes());
        let old_title = self.grid.title().to_string();
        self.grid.process(&actions);

        // Fire title change callback
        if self.grid.title() != old_title {
            if let Some(ref cb) = self.on_title {
                let title = JsValue::from_str(self.grid.title());
                let _ = cb.call1(&JsValue::NULL, &title);
            }
        }

        self.render();
    }

    /// Write raw bytes to the terminal.
    #[wasm_bindgen(js_name = "writeBytes")]
    pub fn write_bytes(&mut self, data: &[u8]) {
        let actions = self.parser.parse(data);
        let old_title = self.grid.title().to_string();
        self.grid.process(&actions);

        if self.grid.title() != old_title {
            if let Some(ref cb) = self.on_title {
                let title = JsValue::from_str(self.grid.title());
                let _ = cb.call1(&JsValue::NULL, &title);
            }
        }

        self.render();
    }

    /// Register a callback for user input data.
    /// The callback receives a string of escape sequence bytes.
    #[wasm_bindgen(js_name = "onData")]
    pub fn on_data(&mut self, callback: js_sys::Function) {
        self.on_data = Some(callback);
    }

    /// Register a callback for title changes.
    #[wasm_bindgen(js_name = "onTitleChange")]
    pub fn on_title_change(&mut self, callback: js_sys::Function) {
        self.on_title = Some(callback);
    }

    /// Handle a keyboard event. Returns the escape sequence string if the
    /// key was handled, or null if it should be ignored.
    /// The caller is responsible for forwarding the data (avoids re-entrant borrow).
    #[wasm_bindgen(js_name = "handleKeyEvent")]
    pub fn handle_key_event(&self, event: &web_sys::KeyboardEvent) -> Option<String> {
        let key = event.key();
        // Don't intercept Meta (Cmd on Mac) — let browser handle Cmd+C/V/etc.
        let ctrl = event.ctrl_key();
        let alt = event.alt_key();
        let shift = event.shift_key();

        if event.meta_key() {
            return None;
        }

        let app_cursor = self.grid.modes().app_cursor;

        encode_key(&key, ctrl, alt, shift, app_cursor)
            .map(|bytes| bytes.iter().map(|&b| char::from(b)).collect())
    }

    /// Resize the terminal to fit its container.
    pub fn fit(&mut self) {
        if let Some(ref renderer) = self.renderer {
            let (cols, rows) = renderer.measure_grid();
            if cols != self.grid.cols() || rows != self.grid.rows() {
                self.grid.resize(cols, rows);
                self.render();
            }
        }
    }

    /// Get current number of columns.
    #[wasm_bindgen(getter)]
    pub fn cols(&self) -> usize {
        self.grid.cols()
    }

    /// Get current number of rows.
    #[wasm_bindgen(getter)]
    pub fn rows(&self) -> usize {
        self.grid.rows()
    }

    /// Force a full redraw.
    pub fn refresh(&mut self) {
        self.render();
    }

    /// Resize to specific dimensions.
    pub fn resize(&mut self, cols: usize, rows: usize) {
        self.grid.resize(cols, rows);
        self.render();
    }

    /// Reset the terminal.
    pub fn reset(&mut self) {
        self.grid.reset();
        self.render();
    }

    /// Scroll up by `lines` lines into scrollback.
    #[wasm_bindgen(js_name = "scrollUp")]
    pub fn scroll_up(&mut self, lines: usize) {
        if let Some(ref mut renderer) = self.renderer {
            let max = self.grid.grid().scrollback_len();
            renderer.scroll_up(lines, max);
            renderer.draw(&self.grid);
        }
    }

    /// Scroll down by `lines` lines (towards live view).
    #[wasm_bindgen(js_name = "scrollDown")]
    pub fn scroll_down(&mut self, lines: usize) {
        if let Some(ref mut renderer) = self.renderer {
            renderer.scroll_down(lines);
            renderer.draw(&self.grid);
        }
    }

    /// Snap scroll to bottom (live view).
    #[wasm_bindgen(js_name = "scrollToBottom")]
    pub fn scroll_to_bottom(&mut self) {
        if let Some(ref mut renderer) = self.renderer {
            renderer.scroll_to_bottom();
            renderer.draw(&self.grid);
        }
    }

    /// Whether currently scrolled into scrollback.
    #[wasm_bindgen(getter, js_name = "isScrolled")]
    pub fn is_scrolled(&self) -> bool {
        self.renderer
            .as_ref()
            .map(|r| r.scroll_offset() > 0)
            .unwrap_or(false)
    }

    fn render(&mut self) {
        if let Some(ref mut renderer) = self.renderer {
            // New output snaps to bottom
            renderer.scroll_to_bottom();
            renderer.draw(&self.grid);
        }
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
