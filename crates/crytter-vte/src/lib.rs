//! VTE parser wrapper for crytter.
//!
//! Wraps the `vte` crate's zero-alloc state machine and translates parsed
//! actions into a higher-level `Action` enum that crytter-grid consumes.

use vte::{Params, Perform};

/// High-level terminal action produced by parsing PTY output.
#[derive(Debug, Clone, PartialEq)]
pub enum Action {
    /// Print a visible character at the cursor position.
    Print(char),
    /// Execute a C0/C1 control code.
    Execute(u8),
    /// CSI (Control Sequence Introducer) command.
    Csi {
        params: Vec<Vec<u16>>,
        intermediates: Vec<u8>,
        action: char,
    },
    /// ESC sequence.
    Esc {
        intermediates: Vec<u8>,
        action: u8,
    },
    /// OSC (Operating System Command) — title, colors, etc.
    Osc(Vec<Vec<u8>>),
    /// DCS (Device Control String) — sixel, DECRQSS, etc.
    Dcs {
        params: Vec<Vec<u16>>,
        intermediates: Vec<u8>,
        action: u8,
    },
}

/// Collects VTE parser callbacks into a queue of `Action`s.
struct ActionCollector {
    actions: Vec<Action>,
}

impl ActionCollector {
    fn new() -> Self {
        Self {
            actions: Vec::with_capacity(64),
        }
    }
}

impl Perform for ActionCollector {
    fn print(&mut self, c: char) {
        self.actions.push(Action::Print(c));
    }

    fn execute(&mut self, byte: u8) {
        self.actions.push(Action::Execute(byte));
    }

    fn csi_dispatch(&mut self, params: &Params, intermediates: &[u8], _ignore: bool, action: char) {
        let params: Vec<Vec<u16>> = params.iter().map(|p| p.to_vec()).collect();
        self.actions.push(Action::Csi {
            params,
            intermediates: intermediates.to_vec(),
            action,
        });
    }

    fn esc_dispatch(&mut self, intermediates: &[u8], _ignore: bool, byte: u8) {
        self.actions.push(Action::Esc {
            intermediates: intermediates.to_vec(),
            action: byte,
        });
    }

    fn osc_dispatch(&mut self, params: &[&[u8]], _bell_terminated: bool) {
        let params: Vec<Vec<u8>> = params.iter().map(|p| p.to_vec()).collect();
        self.actions.push(Action::Osc(params));
    }

    fn hook(&mut self, params: &Params, intermediates: &[u8], _ignore: bool, action: char) {
        let params: Vec<Vec<u16>> = params.iter().map(|p| p.to_vec()).collect();
        self.actions.push(Action::Dcs {
            params,
            intermediates: intermediates.to_vec(),
            action: action as u8,
        });
    }

    fn unhook(&mut self) {}
    fn put(&mut self, _byte: u8) {}
}

/// Terminal parser. Feed it bytes, get back actions.
pub struct Parser {
    inner: vte::Parser,
}

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}

impl Parser {
    pub fn new() -> Self {
        Self {
            inner: vte::Parser::new(),
        }
    }

    /// Parse a chunk of bytes and return the resulting actions.
    pub fn parse(&mut self, bytes: &[u8]) -> Vec<Action> {
        let mut collector = ActionCollector::new();
        self.inner.advance(&mut collector, bytes);
        collector.actions
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_plain_text() {
        let mut parser = Parser::new();
        let actions = parser.parse(b"hello");
        assert_eq!(
            actions,
            vec![
                Action::Print('h'),
                Action::Print('e'),
                Action::Print('l'),
                Action::Print('l'),
                Action::Print('o'),
            ]
        );
    }

    #[test]
    fn parse_csi_cursor_move() {
        let mut parser = Parser::new();
        // CSI 5 A = cursor up 5
        let actions = parser.parse(b"\x1b[5A");
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            Action::Csi {
                params,
                action,
                intermediates,
            } => {
                assert_eq!(*action, 'A');
                assert_eq!(params, &vec![vec![5]]);
                assert!(intermediates.is_empty());
            }
            other => panic!("expected Csi, got {:?}", other),
        }
    }

    #[test]
    fn parse_newline() {
        let mut parser = Parser::new();
        let actions = parser.parse(b"\n");
        assert_eq!(actions, vec![Action::Execute(0x0A)]);
    }

    #[test]
    fn parse_sgr() {
        let mut parser = Parser::new();
        // CSI 1;31 m = bold + red foreground
        let actions = parser.parse(b"\x1b[1;31m");
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            Action::Csi { params, action, .. } => {
                assert_eq!(*action, 'm');
                assert_eq!(params, &vec![vec![1], vec![31]]);
            }
            other => panic!("expected Csi, got {:?}", other),
        }
    }

    #[test]
    fn parse_osc_title() {
        let mut parser = Parser::new();
        // OSC 0 ; my title BEL
        let actions = parser.parse(b"\x1b]0;my title\x07");
        assert_eq!(actions.len(), 1);
        match &actions[0] {
            Action::Osc(params) => {
                assert_eq!(params.len(), 2);
                assert_eq!(params[0], b"0");
                assert_eq!(params[1], b"my title");
            }
            other => panic!("expected Osc, got {:?}", other),
        }
    }
}
