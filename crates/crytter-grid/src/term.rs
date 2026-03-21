use crytter_vte::Action;

use crate::attr::Color;
use crate::cursor::Cursor;
use crate::grid::Grid;

/// Maximum length for OSC title to prevent unbounded allocation.
const MAX_TITLE_LEN: usize = 4096;

/// Maximum CSI parameter value we'll honor for repeat counts (scroll, move, etc).
/// Prevents DoS via `\x1b[65535S` looping 65K times.
const MAX_REPEAT: usize = 10_000;

/// Terminal modes.
#[derive(Debug, Clone)]
pub struct Modes {
    /// Auto-wrap at end of line (DECAWM).
    pub autowrap: bool,
    /// Origin mode (DECOM) — cursor relative to scroll region.
    pub origin: bool,
    /// Insert mode (IRM) — insert chars instead of overwriting.
    pub insert: bool,
    /// Bracket paste mode.
    pub bracket_paste: bool,
    /// Alternate screen buffer active.
    pub alt_screen: bool,
    /// Application cursor keys (DECCKM).
    pub app_cursor: bool,
    /// Application keypad mode (DECKPAM/DECKPNM).
    pub app_keypad: bool,
}

impl Default for Modes {
    fn default() -> Self {
        Self {
            autowrap: true,
            origin: false,
            insert: false,
            bracket_paste: false,
            alt_screen: false,
            app_cursor: false,
            app_keypad: false,
        }
    }
}

/// Full terminal state.
pub struct Terminal {
    /// Primary grid.
    grid: Grid,
    /// Alternate screen grid.
    alt_grid: Grid,
    /// Cursor.
    cursor: Cursor,
    /// Terminal modes.
    modes: Modes,
    /// Scroll region top (inclusive).
    scroll_top: usize,
    /// Scroll region bottom (exclusive).
    scroll_bottom: usize,
    /// Tab stops.
    tabs: Vec<bool>,
    /// Window title (set via OSC).
    title: String,
    /// Whether the cursor needs to wrap on next print.
    wrap_pending: bool,
    /// Response queue — bytes to send back to the PTY (DA1, CPR, etc).
    responses: Vec<Vec<u8>>,
}

impl Terminal {
    pub fn new(cols: usize, rows: usize) -> Self {
        // Grid::new clamps to min 1, so use its actual dimensions
        let grid = Grid::new(cols, rows);
        let cols = grid.cols();
        let rows = grid.rows();

        let mut tabs = vec![false; cols];
        for i in (0..cols).step_by(8) {
            tabs[i] = true;
        }

        Self {
            alt_grid: Grid::new(cols, rows),
            grid,
            cursor: Cursor::default(),
            modes: Modes::default(),
            scroll_top: 0,
            scroll_bottom: rows,
            tabs,
            title: String::new(),
            wrap_pending: false,
            responses: Vec::new(),
        }
    }

    pub fn grid(&self) -> &Grid {
        &self.grid
    }

    pub fn cursor(&self) -> &Cursor {
        &self.cursor
    }

    pub fn modes(&self) -> &Modes {
        &self.modes
    }

    pub fn title(&self) -> &str {
        &self.title
    }

    pub fn cols(&self) -> usize {
        self.grid.cols()
    }

    /// Drain response bytes queued by terminal queries (DA1, CPR, etc).
    /// The caller should send these back to the PTY.
    pub fn drain_responses(&mut self) -> Vec<Vec<u8>> {
        std::mem::take(&mut self.responses)
    }

    pub fn rows(&self) -> usize {
        self.grid.rows()
    }

    /// Clamp cursor to grid bounds.
    fn clamp_cursor(&mut self) {
        let max_col = self.grid.cols().saturating_sub(1);
        let max_row = self.grid.rows().saturating_sub(1);
        self.cursor.col = self.cursor.col.min(max_col);
        self.cursor.row = self.cursor.row.min(max_row);
    }

    /// Feed parsed VTE actions into the terminal.
    pub fn process(&mut self, actions: &[Action]) {
        for action in actions {
            match action {
                Action::Print(c) => self.print(*c),
                Action::Execute(byte) => self.execute(*byte),
                Action::Csi {
                    params,
                    intermediates,
                    action,
                } => self.csi(params, intermediates, *action),
                Action::Esc {
                    intermediates,
                    action,
                } => self.esc(intermediates, *action),
                Action::Osc(params) => self.osc(params),
                Action::Dcs { .. } => {} // TODO: DCS handling
            }
        }
    }

    fn print(&mut self, c: char) {
        if self.wrap_pending {
            self.cursor.col = 0;
            self.linefeed();
            self.wrap_pending = false;
        }

        let col = self.cursor.col;
        let row = self.cursor.row;
        let cols = self.grid.cols();

        if col < cols && row < self.grid.rows() {
            // Insert mode: shift chars right before writing
            if self.modes.insert {
                self.insert_chars(1);
            }

            if let Some(cell) = self.grid.cell_mut(row, col) {
                cell.c = c;
                cell.attr = self.cursor.attr;
                cell.dirty = true;
            }

            if col + 1 >= cols {
                if self.modes.autowrap {
                    self.wrap_pending = true;
                }
            } else {
                self.cursor.col += 1;
            }
        }
    }

    fn execute(&mut self, byte: u8) {
        match byte {
            // BEL
            0x07 => {} // TODO: bell callback
            // BS — backspace
            0x08 => {
                if self.cursor.col > 0 {
                    self.cursor.col -= 1;
                    self.wrap_pending = false;
                }
            }
            // HT — horizontal tab
            0x09 => self.tab(),
            // LF, VT, FF — linefeed
            0x0A | 0x0B | 0x0C => {
                self.linefeed();
                self.wrap_pending = false;
            }
            // CR — carriage return
            0x0D => {
                self.cursor.col = 0;
                self.wrap_pending = false;
            }
            // SO — shift out (G1 charset) — ignore for now
            0x0E => {}
            // SI — shift in (G0 charset) — ignore for now
            0x0F => {}
            _ => {}
        }
    }

    fn linefeed(&mut self) {
        if self.cursor.row + 1 >= self.scroll_bottom {
            self.grid.scroll_up(self.scroll_top, self.scroll_bottom);
        } else {
            self.cursor.row += 1;
        }
    }

    fn tab(&mut self) {
        let cols = self.grid.cols();
        let mut col = self.cursor.col + 1;
        while col < cols {
            if col < self.tabs.len() && self.tabs[col] {
                break;
            }
            col += 1;
        }
        self.cursor.col = col.min(cols.saturating_sub(1));
        self.wrap_pending = false;
    }

    fn csi(&mut self, params: &[Vec<u16>], intermediates: &[u8], action: char) {
        // Helper to get param with default value, capped to MAX_REPEAT
        let param = |i: usize, default: u16| -> u16 {
            params
                .get(i)
                .and_then(|p| p.first().copied())
                .filter(|&v| v != 0)
                .unwrap_or(default)
        };

        // CSI > ... — xterm-style queries
        if intermediates.first() == Some(&b'>') {
            match action {
                // XTVERSION — report terminal name/version
                'q' => {
                    self.responses.push(b"\x1bP>|crytter 0.1.0\x1b\\".to_vec());
                }
                // DA2 — Secondary Device Attributes
                'c' => {
                    self.responses.push(b"\x1b[>1;0;0c".to_vec());
                }
                _ => {}
            }
            return;
        }

        // Check for DEC private mode (? prefix)
        if intermediates.first() == Some(&b'?') {
            match action {
                'h' => self.dec_set(params),
                'l' => self.dec_reset(params),
                // DECRQM — DEC private mode report (CSI ? Ps $ p)
                // Some apps check this, respond with mode status
                'p' => {
                    // respond "not recognized" (0) for unknown modes
                    // This at least unblocks apps waiting for a response
                    if let Some(mode) = params.first().and_then(|p| p.first().copied()) {
                        let status = match mode {
                            1 => if self.modes.app_cursor { 1 } else { 2 },
                            6 => if self.modes.origin { 1 } else { 2 },
                            7 => if self.modes.autowrap { 1 } else { 2 },
                            25 => if self.cursor.visible { 1 } else { 2 },
                            1004 => 2, // focus reporting — not active
                            2004 => if self.modes.bracket_paste { 1 } else { 2 },
                            2026 => 4, // synchronized output — permanently reset (4)
                            _ => 0, // not recognized
                        };
                        self.responses.push(
                            format!("\x1b[?{mode};{status}$y").into_bytes(),
                        );
                    }
                }
                _ => {}
            }
            return;
        }

        match action {
            // CUU — Cursor Up
            'A' => {
                let n = (param(0, 1) as usize).min(MAX_REPEAT);
                self.cursor.row = self.cursor.row.saturating_sub(n);
                self.wrap_pending = false;
            }
            // CUD — Cursor Down
            'B' => {
                let n = (param(0, 1) as usize).min(MAX_REPEAT);
                self.cursor.row = (self.cursor.row + n).min(self.grid.rows().saturating_sub(1));
                self.wrap_pending = false;
            }
            // CUF — Cursor Forward
            'C' => {
                let n = (param(0, 1) as usize).min(MAX_REPEAT);
                self.cursor.col = (self.cursor.col + n).min(self.grid.cols().saturating_sub(1));
                self.wrap_pending = false;
            }
            // CUB — Cursor Backward
            'D' => {
                let n = (param(0, 1) as usize).min(MAX_REPEAT);
                self.cursor.col = self.cursor.col.saturating_sub(n);
                self.wrap_pending = false;
            }
            // CNL — Cursor Next Line
            'E' => {
                let n = (param(0, 1) as usize).min(MAX_REPEAT);
                self.cursor.row = (self.cursor.row + n).min(self.grid.rows().saturating_sub(1));
                self.cursor.col = 0;
                self.wrap_pending = false;
            }
            // CPL — Cursor Previous Line
            'F' => {
                let n = (param(0, 1) as usize).min(MAX_REPEAT);
                self.cursor.row = self.cursor.row.saturating_sub(n);
                self.cursor.col = 0;
                self.wrap_pending = false;
            }
            // CHA — Cursor Horizontal Absolute
            'G' => {
                let col = param(0, 1) as usize;
                self.cursor.col = col.saturating_sub(1).min(self.grid.cols().saturating_sub(1));
                self.wrap_pending = false;
            }
            // CUP — Cursor Position
            'H' | 'f' => {
                let row = param(0, 1) as usize;
                let col = param(1, 1) as usize;
                self.cursor.row = row.saturating_sub(1).min(self.grid.rows().saturating_sub(1));
                self.cursor.col = col.saturating_sub(1).min(self.grid.cols().saturating_sub(1));
                self.wrap_pending = false;
            }
            // ED — Erase in Display
            'J' => {
                let mode = param(0, 0);
                self.erase_display(mode);
            }
            // EL — Erase in Line
            'K' => {
                let mode = param(0, 0);
                self.erase_line(mode);
            }
            // IL — Insert Lines
            'L' => {
                let n = (param(0, 1) as usize).min(MAX_REPEAT);
                self.grid
                    .insert_lines(self.cursor.row, n, self.scroll_bottom);
            }
            // DL — Delete Lines
            'M' => {
                let n = (param(0, 1) as usize).min(MAX_REPEAT);
                self.grid
                    .delete_lines(self.cursor.row, n, self.scroll_bottom);
            }
            // DCH — Delete Characters
            'P' => {
                let n = (param(0, 1) as usize).min(MAX_REPEAT);
                self.delete_chars(n);
            }
            // SU — Scroll Up
            'S' => {
                let n = (param(0, 1) as usize).min(MAX_REPEAT);
                for _ in 0..n {
                    self.grid.scroll_up(self.scroll_top, self.scroll_bottom);
                }
            }
            // SD — Scroll Down
            'T' => {
                let n = (param(0, 1) as usize).min(MAX_REPEAT);
                for _ in 0..n {
                    self.grid.scroll_down(self.scroll_top, self.scroll_bottom);
                }
            }
            // ECH — Erase Characters
            'X' => {
                let n = (param(0, 1) as usize).min(MAX_REPEAT);
                let row = self.cursor.row;
                let col = self.cursor.col;
                self.grid.erase_cells(row, col, col + n);
            }
            // VPA — Vertical Line Position Absolute
            'd' => {
                let row = param(0, 1) as usize;
                self.cursor.row = row.saturating_sub(1).min(self.grid.rows().saturating_sub(1));
                self.wrap_pending = false;
            }
            // SGR — Select Graphic Rendition
            'm' => self.sgr(params),
            // DECSTBM — Set Scrolling Region
            'r' => {
                let top = param(0, 1) as usize;
                let bottom = param(1, self.grid.rows() as u16) as usize;
                let new_top = top.saturating_sub(1).min(self.grid.rows().saturating_sub(1));
                let new_bottom = bottom.min(self.grid.rows());
                // Validate: top must be above bottom, region must be at least 2 lines
                if new_top < new_bottom && new_bottom - new_top >= 2 {
                    self.scroll_top = new_top;
                    self.scroll_bottom = new_bottom;
                }
                // Cursor goes home regardless
                self.cursor.row = 0;
                self.cursor.col = 0;
                self.wrap_pending = false;
            }
            // ICH — Insert Characters
            '@' => {
                let n = (param(0, 1) as usize).min(MAX_REPEAT);
                self.insert_chars(n);
            }
            // SM — Set Mode
            'h' => {
                for p in params {
                    if p.first().copied() == Some(4) {
                        self.modes.insert = true;
                    }
                }
            }
            // RM — Reset Mode
            'l' => {
                for p in params {
                    if p.first().copied() == Some(4) {
                        self.modes.insert = false;
                    }
                }
            }
            // DA1 — Primary Device Attributes
            'c' => {
                if intermediates.is_empty() {
                    self.responses.push(b"\x1b[?62;22c".to_vec());
                }
                // DA2 (intermediates = '>') is handled in the > block above
            }
            // DSR — Device Status Report
            'n' => {
                let mode = param(0, 0);
                match mode {
                    // Status report — respond "OK"
                    5 => self.responses.push(b"\x1b[0n".to_vec()),
                    // CPR — Cursor Position Report
                    6 => {
                        let row = self.cursor.row + 1;
                        let col = self.cursor.col + 1;
                        self.responses
                            .push(format!("\x1b[{row};{col}R").into_bytes());
                    }
                    _ => {}
                }
            }
            // DECSCUSR — Set Cursor Style (CSI Ps SP q)
            'q' if intermediates.first() == Some(&b' ') => {
                let style = param(0, 1);
                self.cursor.shape = match style {
                    0 | 1 | 2 => crate::cursor::CursorShape::Block,
                    3 | 4 => crate::cursor::CursorShape::Underline,
                    5 | 6 => crate::cursor::CursorShape::Bar,
                    _ => crate::cursor::CursorShape::Block,
                };
            }
            // CSI t — Window manipulation
            't' => {
                let mode = param(0, 0);
                match mode {
                    // Report terminal size in chars
                    18 => {
                        let rows = self.grid.rows();
                        let cols = self.grid.cols();
                        self.responses
                            .push(format!("\x1b[8;{rows};{cols}t").into_bytes());
                    }
                    _ => {}
                }
            }
            _ => {} // Unhandled CSI
        }
    }

    fn sgr(&mut self, params: &[Vec<u16>]) {
        if params.is_empty() {
            self.cursor.attr.reset();
            return;
        }

        let mut i = 0;
        while i < params.len() {
            let p = params[i].first().copied().unwrap_or(0);
            match p {
                0 => self.cursor.attr.reset(),
                1 => self.cursor.attr.bold = true,
                2 => self.cursor.attr.dim = true,
                3 => self.cursor.attr.italic = true,
                4 => self.cursor.attr.underline = true,
                5 => self.cursor.attr.blink = true,
                7 => self.cursor.attr.inverse = true,
                8 => self.cursor.attr.hidden = true,
                9 => self.cursor.attr.strikethrough = true,
                22 => {
                    self.cursor.attr.bold = false;
                    self.cursor.attr.dim = false;
                }
                23 => self.cursor.attr.italic = false,
                24 => self.cursor.attr.underline = false,
                25 => self.cursor.attr.blink = false,
                27 => self.cursor.attr.inverse = false,
                28 => self.cursor.attr.hidden = false,
                29 => self.cursor.attr.strikethrough = false,
                // Foreground colors
                30..=37 => self.cursor.attr.fg = Color::Indexed((p - 30) as u8),
                38 => {
                    i += 1;
                    self.parse_extended_color(params, &mut i, true);
                    continue;
                }
                39 => self.cursor.attr.fg = Color::Default,
                // Background colors
                40..=47 => self.cursor.attr.bg = Color::Indexed((p - 40) as u8),
                48 => {
                    i += 1;
                    self.parse_extended_color(params, &mut i, false);
                    continue;
                }
                49 => self.cursor.attr.bg = Color::Default,
                // Bright foreground
                90..=97 => self.cursor.attr.fg = Color::Indexed((p - 90 + 8) as u8),
                // Bright background
                100..=107 => self.cursor.attr.bg = Color::Indexed((p - 100 + 8) as u8),
                _ => {}
            }
            i += 1;
        }
    }

    fn parse_extended_color(
        &mut self,
        params: &[Vec<u16>],
        i: &mut usize,
        foreground: bool,
    ) {
        let kind = params.get(*i).and_then(|p| p.first().copied());
        match kind {
            // 256-color
            Some(5) => {
                *i += 1;
                if let Some(idx) = params.get(*i).and_then(|p| p.first().copied()) {
                    let color = Color::Indexed(idx as u8);
                    if foreground {
                        self.cursor.attr.fg = color;
                    } else {
                        self.cursor.attr.bg = color;
                    }
                }
                *i += 1;
            }
            // RGB
            Some(2) => {
                *i += 1;
                let r = params.get(*i).and_then(|p| p.first().copied()).unwrap_or(0) as u8;
                *i += 1;
                let g = params.get(*i).and_then(|p| p.first().copied()).unwrap_or(0) as u8;
                *i += 1;
                let b = params.get(*i).and_then(|p| p.first().copied()).unwrap_or(0) as u8;
                let color = Color::Rgb(r, g, b);
                if foreground {
                    self.cursor.attr.fg = color;
                } else {
                    self.cursor.attr.bg = color;
                }
                *i += 1;
            }
            _ => {
                *i += 1;
            }
        }
    }

    fn erase_display(&mut self, mode: u16) {
        let row = self.cursor.row;
        let col = self.cursor.col;
        let rows = self.grid.rows();
        let cols = self.grid.cols();

        match mode {
            // Below (from cursor to end)
            0 => {
                self.grid.erase_cells(row, col, cols);
                for r in row + 1..rows {
                    self.grid.erase_cells(r, 0, cols);
                }
            }
            // Above (from start to cursor)
            1 => {
                for r in 0..row {
                    self.grid.erase_cells(r, 0, cols);
                }
                self.grid.erase_cells(row, 0, col + 1);
            }
            // Entire display
            2 => self.grid.clear(),
            // Entire display + scrollback
            3 => {
                self.grid.clear();
                self.grid.clear_scrollback();
            }
            _ => {}
        }
    }

    fn erase_line(&mut self, mode: u16) {
        let row = self.cursor.row;
        let col = self.cursor.col;
        let cols = self.grid.cols();

        match mode {
            0 => self.grid.erase_cells(row, col, cols),
            1 => self.grid.erase_cells(row, 0, col + 1),
            2 => self.grid.erase_cells(row, 0, cols),
            _ => {}
        }
    }

    fn delete_chars(&mut self, count: usize) {
        let row = self.cursor.row;
        let col = self.cursor.col;
        let cols = self.grid.cols();

        // Shift cells left
        for c in col..cols {
            let src = c + count;
            if src < cols {
                let src_cell = self.grid.cell(row, src).clone();
                if let Some(dst) = self.grid.cell_mut(row, c) {
                    *dst = src_cell;
                }
            } else {
                if let Some(cell) = self.grid.cell_mut(row, c) {
                    cell.reset();
                }
            }
        }
    }

    fn insert_chars(&mut self, count: usize) {
        let row = self.cursor.row;
        let col = self.cursor.col;
        let cols = self.grid.cols();

        // Shift cells right
        for c in (col..cols).rev() {
            if c >= col + count {
                let src_cell = self.grid.cell(row, c - count).clone();
                if let Some(dst) = self.grid.cell_mut(row, c) {
                    *dst = src_cell;
                }
            } else {
                if let Some(cell) = self.grid.cell_mut(row, c) {
                    cell.reset();
                }
            }
        }
    }

    fn esc(&mut self, intermediates: &[u8], action: u8) {
        match (intermediates.first(), action) {
            // ESC 7 — Save Cursor (DECSC)
            (None, b'7') => self.cursor.save(),
            // ESC 8 — Restore Cursor (DECRC)
            (None, b'8') => {
                self.cursor.restore();
                self.clamp_cursor();
            }
            // ESC D — Index (linefeed)
            (None, b'D') => self.linefeed(),
            // ESC M — Reverse Index (reverse linefeed)
            (None, b'M') => self.reverse_index(),
            // ESC E — Next Line
            (None, b'E') => {
                self.cursor.col = 0;
                self.linefeed();
            }
            // ESC c — Full Reset (RIS)
            (None, b'c') => self.reset(),
            // ESC H — Tab Set (HTS)
            (None, b'H') => {
                let col = self.cursor.col;
                if col < self.tabs.len() {
                    self.tabs[col] = true;
                }
            }
            // ESC ( — Designate G0 character set — ignore
            (Some(b'('), _) => {}
            // ESC ) — Designate G1 character set — ignore
            (Some(b')'), _) => {}
            _ => {}
        }
    }

    fn reverse_index(&mut self) {
        if self.cursor.row == self.scroll_top {
            self.grid.scroll_down(self.scroll_top, self.scroll_bottom);
        } else if self.cursor.row > 0 {
            self.cursor.row -= 1;
        }
    }

    fn osc(&mut self, params: &[Vec<u8>]) {
        if params.is_empty() {
            return;
        }

        let cmd = params[0].as_slice();
        match cmd {
            // Set window title
            b"0" | b"2" => {
                if let Some(title) = params.get(1) {
                    // Cap title length to prevent unbounded allocation
                    let len = title.len().min(MAX_TITLE_LEN);
                    self.title = String::from_utf8_lossy(&title[..len]).into_owned();
                }
            }
            _ => {}
        }
    }

    fn dec_set(&mut self, params: &[Vec<u16>]) {
        for p in params {
            match p.first().copied().unwrap_or(0) {
                1 => self.modes.app_cursor = true,
                6 => self.modes.origin = true,
                7 => self.modes.autowrap = true,
                12 => {} // Cursor blink — acknowledge, no-op
                25 => self.cursor.visible = true,
                47 | 1047 => self.switch_alt_screen(true),
                1004 => {} // Focus reporting — acknowledge, no-op for now
                1049 => {
                    self.cursor.save();
                    self.switch_alt_screen(true);
                }
                2004 => self.modes.bracket_paste = true,
                2026 => {} // Synchronized output (DECSYNC) — acknowledge, no-op
                _ => {}
            }
        }
    }

    fn dec_reset(&mut self, params: &[Vec<u16>]) {
        for p in params {
            match p.first().copied().unwrap_or(0) {
                1 => self.modes.app_cursor = false,
                6 => self.modes.origin = false,
                7 => self.modes.autowrap = false,
                12 => {} // Cursor blink
                25 => self.cursor.visible = false,
                47 | 1047 => self.switch_alt_screen(false),
                1004 => {} // Focus reporting
                1049 => {
                    self.switch_alt_screen(false);
                    self.cursor.restore();
                    self.clamp_cursor();
                }
                2004 => self.modes.bracket_paste = false,
                2026 => {} // Synchronized output
                _ => {}
            }
        }
    }

    fn switch_alt_screen(&mut self, alt: bool) {
        if alt != self.modes.alt_screen {
            std::mem::swap(&mut self.grid, &mut self.alt_grid);
            self.modes.alt_screen = alt;
            if alt {
                self.grid.clear();
            }
        }
    }

    /// Full terminal reset.
    pub fn reset(&mut self) {
        let cols = self.grid.cols();
        let rows = self.grid.rows();
        *self = Self::new(cols, rows);
    }

    /// Resize the terminal.
    pub fn resize(&mut self, cols: usize, rows: usize) {
        self.grid.resize(cols, rows);
        self.alt_grid.resize(cols, rows);
        let cols = self.grid.cols();
        let rows = self.grid.rows();
        self.scroll_top = 0;
        self.scroll_bottom = rows;
        self.tabs = vec![false; cols];
        for i in (0..cols).step_by(8) {
            self.tabs[i] = true;
        }
        self.clamp_cursor();
        self.wrap_pending = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crytter_vte::Parser;

    fn parse_and_process(term: &mut Terminal, input: &[u8]) {
        let mut parser = Parser::new();
        let actions = parser.parse(input);
        term.process(&actions);
    }

    #[test]
    fn print_characters() {
        let mut term = Terminal::new(80, 24);
        parse_and_process(&mut term, b"ABC");
        assert_eq!(term.grid().cell(0, 0).c, 'A');
        assert_eq!(term.grid().cell(0, 1).c, 'B');
        assert_eq!(term.grid().cell(0, 2).c, 'C');
        assert_eq!(term.cursor().col, 3);
    }

    #[test]
    fn cursor_movement() {
        let mut term = Terminal::new(80, 24);
        parse_and_process(&mut term, b"\x1b[5;10H");
        assert_eq!(term.cursor().row, 4);
        assert_eq!(term.cursor().col, 9);
    }

    #[test]
    fn erase_display_below() {
        let mut term = Terminal::new(80, 24);
        parse_and_process(&mut term, b"ABCDEF");
        parse_and_process(&mut term, b"\x1b[1;4H");
        parse_and_process(&mut term, b"\x1b[0J");
        assert_eq!(term.grid().cell(0, 0).c, 'A');
        assert_eq!(term.grid().cell(0, 1).c, 'B');
        assert_eq!(term.grid().cell(0, 2).c, 'C');
        assert_eq!(term.grid().cell(0, 3).c, ' ');
    }

    #[test]
    fn sgr_colors() {
        let mut term = Terminal::new(80, 24);
        parse_and_process(&mut term, b"\x1b[1;31mX");
        let cell = term.grid().cell(0, 0);
        assert_eq!(cell.c, 'X');
        assert!(cell.attr.bold);
        assert_eq!(cell.attr.fg, Color::Indexed(1));
    }

    #[test]
    fn linefeed_scrolls() {
        let mut term = Terminal::new(80, 3);
        parse_and_process(&mut term, b"line1\r\nline2\r\nline3\r\nline4");
        assert_eq!(term.grid().scrollback_len(), 1);
        assert_eq!(term.grid().cell(0, 0).c, 'l');
        assert_eq!(term.grid().cell(2, 0).c, 'l');
    }

    #[test]
    fn osc_sets_title() {
        let mut term = Terminal::new(80, 24);
        parse_and_process(&mut term, b"\x1b]0;hello world\x07");
        assert_eq!(term.title(), "hello world");
    }

    #[test]
    fn alt_screen_switch() {
        let mut term = Terminal::new(80, 24);
        parse_and_process(&mut term, b"main");
        parse_and_process(&mut term, b"\x1b[?1049h");
        assert!(term.modes().alt_screen);
        assert_eq!(term.grid().cell(0, 0).c, ' ');
        parse_and_process(&mut term, b"\x1b[?1049l");
        assert!(!term.modes().alt_screen);
        assert_eq!(term.grid().cell(0, 0).c, 'm');
    }

    #[test]
    fn autowrap() {
        let mut term = Terminal::new(5, 3);
        parse_and_process(&mut term, b"12345X");
        assert_eq!(term.grid().cell(0, 4).c, '5');
        assert_eq!(term.grid().cell(1, 0).c, 'X');
    }
}
