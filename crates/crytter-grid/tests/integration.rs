//! Integration tests for crytter-grid.
//!
//! Tests full parser → terminal pipeline with adversarial inputs,
//! real-world escape sequences, and edge cases.

use crytter_grid::{Color, Terminal};
use crytter_vte::Parser;

fn feed(term: &mut Terminal, input: &[u8]) {
    let mut parser = Parser::new();
    let actions = parser.parse(input);
    term.process(&actions);
}

// ============================================================
// Adversarial: bounds, panics, resource exhaustion
// ============================================================

#[test]
fn zero_size_terminal_does_not_panic() {
    let mut term = Terminal::new(0, 0);
    // Grid clamps to 1x1
    assert_eq!(term.cols(), 1);
    assert_eq!(term.rows(), 1);
    feed(&mut term, b"hello world");
    feed(&mut term, b"\x1b[999;999H");
    feed(&mut term, b"\x1b[2J");
    feed(&mut term, b"\x1b[?1049h");
    feed(&mut term, b"\x1b[?1049l");
}

#[test]
fn massive_cursor_movement_does_not_panic() {
    let mut term = Terminal::new(80, 24);
    // Move cursor way out of bounds in all directions
    feed(&mut term, b"\x1b[9999;9999H");
    assert_eq!(term.cursor().row, 23);
    assert_eq!(term.cursor().col, 79);

    feed(&mut term, b"\x1b[9999A"); // cursor up 9999
    assert_eq!(term.cursor().row, 0);

    feed(&mut term, b"\x1b[9999B"); // cursor down 9999
    assert_eq!(term.cursor().row, 23);

    feed(&mut term, b"\x1b[9999D"); // cursor back 9999
    assert_eq!(term.cursor().col, 0);

    feed(&mut term, b"\x1b[9999C"); // cursor forward 9999
    assert_eq!(term.cursor().col, 79);
}

#[test]
fn resize_to_zero_clamps() {
    let mut term = Terminal::new(80, 24);
    feed(&mut term, b"hello");
    term.resize(0, 0);
    assert_eq!(term.cols(), 1);
    assert_eq!(term.rows(), 1);
    // Cursor should be clamped
    assert_eq!(term.cursor().col, 0);
    assert_eq!(term.cursor().row, 0);
    // Should still be able to write
    feed(&mut term, b"X");
    assert_eq!(term.grid().cell(0, 0).c, 'X');
}

#[test]
fn rapid_scroll_does_not_oom() {
    let mut term = Terminal::new(80, 24);
    // Simulate `cat /dev/urandom | head -c 1000000` — lots of linefeeds
    let input: Vec<u8> = (0..50_000).map(|i| if i % 80 == 79 { b'\n' } else { b'X' }).collect();
    feed(&mut term, &input);
    // Scrollback should be capped
    assert!(term.grid().scrollback_len() <= 10_000);
}

#[test]
fn csi_scroll_capped_iterations() {
    let mut term = Terminal::new(80, 24);
    // Try to scroll up 65535 times — should be capped
    feed(&mut term, b"\x1b[65535S");
    // If this completes in reasonable time, the cap worked.
    // Scrollback should be at most 10_000 even with MAX_REPEAT capping at 10_000
    assert!(term.grid().scrollback_len() <= 10_000);
}

#[test]
fn inverted_scroll_region_rejected() {
    let mut term = Terminal::new(80, 24);
    // Try to set inverted scroll region (bottom < top)
    feed(&mut term, b"\x1b[20;5r");
    // Scroll region should NOT have been updated to inverted values
    // (cursor goes home regardless)
    assert_eq!(term.cursor().row, 0);
    assert_eq!(term.cursor().col, 0);
    // Normal scrolling should still work
    feed(&mut term, b"line1\r\nline2\r\n");
}

#[test]
fn single_line_scroll_region_rejected() {
    let mut term = Terminal::new(80, 24);
    // Region of exactly 1 line — not useful, should be rejected
    feed(&mut term, b"\x1b[5;5r");
    assert_eq!(term.cursor().row, 0);
}

// ============================================================
// OWASP-relevant: resource exhaustion, injection boundaries
// ============================================================

#[test]
fn osc_title_length_capped() {
    let mut term = Terminal::new(80, 24);
    // Send a title that's 100KB
    let mut input = b"\x1b]0;".to_vec();
    input.extend(vec![b'A'; 100_000]);
    input.push(0x07); // BEL terminator
    feed(&mut term, &input);
    // Title should be capped at MAX_TITLE_LEN (4096)
    assert!(term.title().len() <= 4096);
}

#[test]
fn osc_title_with_malicious_content() {
    let mut term = Terminal::new(80, 24);
    // Title containing HTML/JS — should be stored as-is (no interpretation)
    feed(
        &mut term,
        b"\x1b]0;<script>alert('xss')</script>\x07",
    );
    assert_eq!(term.title(), "<script>alert('xss')</script>");
    // The renderer layer MUST NOT inject this into DOM innerHTML.
    // This test documents that the grid layer stores raw text.
}

#[test]
fn osc_title_with_null_bytes() {
    let mut term = Terminal::new(80, 24);
    feed(&mut term, b"\x1b]0;hello\x00world\x07");
    // Should handle gracefully (lossy UTF-8 conversion)
    let title = term.title();
    assert!(title.contains("hello"));
}

#[test]
fn binary_garbage_does_not_panic() {
    let mut term = Terminal::new(80, 24);
    // Feed pure random bytes — should never panic
    let garbage: Vec<u8> = (0..10_000).map(|i| (i * 7 + 13) as u8).collect();
    feed(&mut term, &garbage);
    // Just verify it survived
    assert!(term.cursor().row < term.rows());
    assert!(term.cursor().col < term.cols());
}

#[test]
fn incomplete_escape_sequences() {
    let mut term = Terminal::new(80, 24);
    // Incomplete CSI — just ESC [
    feed(&mut term, b"\x1b[");
    // Partial SGR
    feed(&mut term, b"\x1b[38;5;");
    // Unterminated OSC
    feed(&mut term, b"\x1b]0;title without terminator");
    // Terminal should still be functional
    feed(&mut term, b"OK");
}

// ============================================================
// Real-world terminal sequences
// ============================================================

#[test]
fn bash_prompt_simulation() {
    let mut term = Terminal::new(80, 24);
    // Typical bash prompt: set title, color prompt, command, output
    feed(&mut term, b"\x1b]0;user@host:~\x07"); // set title
    feed(&mut term, b"\x1b[1;32muser@host\x1b[0m:\x1b[1;34m~\x1b[0m$ "); // colored prompt
    feed(&mut term, b"ls\r\n"); // command
    feed(&mut term, b"file1.txt  file2.txt\r\n"); // output

    assert_eq!(term.title(), "user@host:~");
    // Prompt should be on row 0
    assert_eq!(term.grid().cell(0, 0).c, 'u');
    assert!(term.grid().cell(0, 0).attr.bold); // green bold
    assert_eq!(term.grid().cell(0, 0).attr.fg, Color::Indexed(2));
}

#[test]
fn vim_open_simulation() {
    let mut term = Terminal::new(80, 24);
    // vim opens: switch to alt screen, clear, position cursor
    feed(&mut term, b"$ vim\r\n");
    feed(&mut term, b"\x1b[?1049h"); // alt screen
    feed(&mut term, b"\x1b[2J"); // clear
    feed(&mut term, b"\x1b[1;1H"); // cursor home
    feed(&mut term, b"~\r\n~\r\n~\r\n"); // tilde lines

    assert!(term.modes().alt_screen);
    assert_eq!(term.grid().cell(0, 0).c, '~');
    assert_eq!(term.grid().cell(1, 0).c, '~');

    // Exit vim: back to main screen
    feed(&mut term, b"\x1b[?1049l");
    assert!(!term.modes().alt_screen);
    // Main screen should have original content
    assert_eq!(term.grid().cell(0, 0).c, '$');
}

#[test]
fn htop_scroll_regions() {
    let mut term = Terminal::new(80, 24);
    // htop uses scroll regions for the process list
    feed(&mut term, b"\x1b[?1049h"); // alt screen
    feed(&mut term, b"\x1b[1;1H"); // home
    feed(&mut term, b"CPU [##########] 50%\r\n");
    feed(&mut term, b"MEM [#####     ] 25%\r\n");

    // Set scroll region for process list area (rows 3-24)
    feed(&mut term, b"\x1b[3;24r");
    // Cursor goes home after DECSTBM
    feed(&mut term, b"\x1b[3;1H"); // position in scroll region
    feed(&mut term, b"  PID USER      PR  NI\r\n");

    assert_eq!(term.grid().cell(2, 2).c, 'P');
}

#[test]
fn color_256_and_rgb() {
    let mut term = Terminal::new(80, 24);
    // 256-color: ESC[38;5;196m = bright red foreground
    feed(&mut term, b"\x1b[38;5;196mR");
    assert_eq!(term.grid().cell(0, 0).c, 'R');
    assert_eq!(term.grid().cell(0, 0).attr.fg, Color::Indexed(196));

    // RGB: ESC[38;2;255;128;0m = orange foreground
    feed(&mut term, b"\x1b[38;2;255;128;0mO");
    assert_eq!(term.grid().cell(0, 1).c, 'O');
    assert_eq!(term.grid().cell(0, 1).attr.fg, Color::Rgb(255, 128, 0));

    // Background RGB
    feed(&mut term, b"\x1b[48;2;0;0;255mB");
    assert_eq!(term.grid().cell(0, 2).c, 'B');
    assert_eq!(term.grid().cell(0, 2).attr.bg, Color::Rgb(0, 0, 255));
}

#[test]
fn sgr_reset_clears_all() {
    let mut term = Terminal::new(80, 24);
    feed(&mut term, b"\x1b[1;3;4;31;42mX"); // bold italic underline red-on-green
    let cell = term.grid().cell(0, 0);
    assert!(cell.attr.bold);
    assert!(cell.attr.italic);
    assert!(cell.attr.underline);

    feed(&mut term, b"\x1b[0mY"); // reset
    let cell = term.grid().cell(0, 1);
    assert!(!cell.attr.bold);
    assert!(!cell.attr.italic);
    assert!(!cell.attr.underline);
    assert_eq!(cell.attr.fg, Color::Default);
    assert_eq!(cell.attr.bg, Color::Default);
}

#[test]
fn cursor_save_restore() {
    let mut term = Terminal::new(80, 24);
    feed(&mut term, b"\x1b[5;10H"); // row 4, col 9
    feed(&mut term, b"\x1b7"); // save
    feed(&mut term, b"\x1b[1;1H"); // home
    feed(&mut term, b"X");
    feed(&mut term, b"\x1b8"); // restore
    assert_eq!(term.cursor().row, 4);
    assert_eq!(term.cursor().col, 9);
}

#[test]
fn erase_line_modes() {
    let mut term = Terminal::new(10, 3);
    feed(&mut term, b"ABCDEFGHIJ");
    feed(&mut term, b"\x1b[1;6H"); // cursor at col 5

    // EL 0: erase from cursor to end of line
    feed(&mut term, b"\x1b[0K");
    assert_eq!(term.grid().cell(0, 4).c, 'E');
    assert_eq!(term.grid().cell(0, 5).c, ' ');

    // Row 2: test EL 1 (erase from start to cursor)
    feed(&mut term, b"\x1b[2;1H");
    feed(&mut term, b"ABCDEFGHIJ");
    feed(&mut term, b"\x1b[2;6H");
    feed(&mut term, b"\x1b[1K");
    assert_eq!(term.grid().cell(1, 4).c, ' '); // erased
    assert_eq!(term.grid().cell(1, 5).c, ' '); // erased (cursor position inclusive)
    assert_eq!(term.grid().cell(1, 6).c, 'G'); // preserved
}

#[test]
fn insert_mode() {
    let mut term = Terminal::new(10, 1);
    feed(&mut term, b"ABCDE");
    feed(&mut term, b"\x1b[1;3H"); // cursor at col 2
    feed(&mut term, b"\x1b[4h"); // enable insert mode
    feed(&mut term, b"XY");

    // "AB" then "XY" inserted, pushing "CDE" right
    assert_eq!(term.grid().cell(0, 0).c, 'A');
    assert_eq!(term.grid().cell(0, 1).c, 'B');
    assert_eq!(term.grid().cell(0, 2).c, 'X');
    assert_eq!(term.grid().cell(0, 3).c, 'Y');
    assert_eq!(term.grid().cell(0, 4).c, 'C');
    assert_eq!(term.grid().cell(0, 5).c, 'D');
    assert_eq!(term.grid().cell(0, 6).c, 'E');

    // Disable insert mode
    feed(&mut term, b"\x1b[4l");
    assert!(!term.modes().insert);
}

#[test]
fn delete_lines_no_scrollback_leak() {
    let mut term = Terminal::new(80, 5);
    feed(&mut term, b"line0\r\nline1\r\nline2\r\nline3\r\nline4");
    feed(&mut term, b"\x1b[1;1H"); // cursor at row 0
    feed(&mut term, b"\x1b[2M"); // delete 2 lines

    // Deleted lines should NOT appear in scrollback
    assert_eq!(term.grid().scrollback_len(), 0);
    // Remaining lines shifted up
    assert_eq!(term.grid().cell(0, 0).c, 'l'); // was line2
}

#[test]
fn tab_stops() {
    let mut term = Terminal::new(80, 24);
    feed(&mut term, b"A\tB\tC");
    // Default tabs every 8 columns: A at 0, tab to 8, B at 8, tab to 16, C at 16
    assert_eq!(term.grid().cell(0, 0).c, 'A');
    assert_eq!(term.grid().cell(0, 8).c, 'B');
    assert_eq!(term.grid().cell(0, 16).c, 'C');
}

#[test]
fn reverse_index_at_top() {
    let mut term = Terminal::new(80, 3);
    feed(&mut term, b"AAA\r\nBBB\r\nCCC");
    feed(&mut term, b"\x1b[1;1H"); // cursor at row 0
    feed(&mut term, b"\x1bM"); // reverse index — should scroll down

    assert_eq!(term.grid().cell(0, 0).c, ' '); // new blank line
    assert_eq!(term.grid().cell(1, 0).c, 'A'); // old row 0
    assert_eq!(term.grid().cell(2, 0).c, 'B'); // old row 1
    // CCC fell off the bottom
}

#[test]
fn erase_display_plus_scrollback() {
    let mut term = Terminal::new(80, 3);
    // Generate some scrollback
    feed(&mut term, b"a\r\nb\r\nc\r\nd\r\ne");
    assert!(term.grid().scrollback_len() > 0);
    // ED 3 — erase display + scrollback
    feed(&mut term, b"\x1b[3J");
    assert_eq!(term.grid().scrollback_len(), 0);
    assert_eq!(term.grid().cell(0, 0).c, ' ');
}

#[test]
fn resize_during_content() {
    let mut term = Terminal::new(80, 24);
    feed(&mut term, b"\x1b[12;40H"); // cursor at row 11, col 39
    feed(&mut term, b"HERE");

    // Shrink smaller than cursor position
    term.resize(20, 5);
    assert!(term.cursor().row < 5);
    assert!(term.cursor().col < 20);

    // Grow back
    term.resize(80, 24);
    // Should not panic
    feed(&mut term, b"still works");
}

#[test]
fn wrap_at_exact_boundary() {
    let mut term = Terminal::new(5, 3);
    // Fill exactly one line
    feed(&mut term, b"ABCDE");
    assert_eq!(term.cursor().col, 4); // at last col, wrap pending
    // Next char should wrap
    feed(&mut term, b"F");
    assert_eq!(term.cursor().row, 1);
    assert_eq!(term.cursor().col, 1);
    assert_eq!(term.grid().cell(1, 0).c, 'F');
}

#[test]
fn no_wrap_when_autowrap_disabled() {
    let mut term = Terminal::new(5, 3);
    feed(&mut term, b"\x1b[?7l"); // disable autowrap
    feed(&mut term, b"ABCDEFGH");
    // Without autowrap, cursor stays at last column, chars overwrite
    assert_eq!(term.cursor().row, 0);
    assert_eq!(term.cursor().col, 4);
    assert_eq!(term.grid().cell(0, 4).c, 'H'); // last char wins
    assert_eq!(term.grid().cell(1, 0).c, ' '); // no wrap to row 1
}

#[test]
fn bracket_paste_mode() {
    let mut term = Terminal::new(80, 24);
    assert!(!term.modes().bracket_paste);
    feed(&mut term, b"\x1b[?2004h");
    assert!(term.modes().bracket_paste);
    feed(&mut term, b"\x1b[?2004l");
    assert!(!term.modes().bracket_paste);
}

#[test]
fn stress_rapid_resize() {
    let mut term = Terminal::new(80, 24);
    feed(&mut term, b"initial content");
    // Rapidly resize many times
    for i in 1..100 {
        term.resize(i % 200 + 1, i % 50 + 1);
        feed(&mut term, b"X");
    }
    // Should not panic or corrupt state
    assert!(term.cursor().row < term.rows());
    assert!(term.cursor().col < term.cols());
}

#[test]
fn cell_out_of_bounds_returns_default() {
    let term = Terminal::new(80, 24);
    // Access way out of bounds
    let cell = term.grid().cell(999, 999);
    assert_eq!(cell.c, ' ');
}
