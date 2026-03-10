use vte::{Params, Parser, Perform};

use crate::screen_buffer::ScreenBuffer;

// ── Perform impl ──────────────────────────────────────────────────────────────

/// Bridges the `vte` parser callbacks into [`ScreenBuffer`] mutations.
struct Performer {
    buffer: ScreenBuffer,
}

impl Performer {
    fn new(rows: usize, cols: usize) -> Self {
        Performer {
            buffer: ScreenBuffer::new(rows, cols),
        }
    }

    /// Read the first numeric parameter, defaulting to `default` when absent.
    fn param1(params: &Params, default: usize) -> usize {
        params
            .iter()
            .next()
            .and_then(|p| p.first().copied())
            .map(|v| v as usize)
            .filter(|&v| v != 0)
            .unwrap_or(default)
    }

    /// Read the first two numeric parameters with individual defaults.
    fn param2(params: &Params, d1: usize, d2: usize) -> (usize, usize) {
        let mut iter = params.iter();
        let p1 = iter
            .next()
            .and_then(|p| p.first().copied())
            .map(|v| v as usize)
            .filter(|&v| v != 0)
            .unwrap_or(d1);
        let p2 = iter
            .next()
            .and_then(|p| p.first().copied())
            .map(|v| v as usize)
            .filter(|&v| v != 0)
            .unwrap_or(d2);
        (p1, p2)
    }
}

impl Perform for Performer {
    fn print(&mut self, c: char) {
        self.buffer.print(c);
    }

    fn execute(&mut self, byte: u8) {
        self.buffer.execute(byte);
    }

    fn csi_dispatch(
        &mut self,
        params: &Params,
        _intermediates: &[u8],
        _ignore: bool,
        action: char,
    ) {
        match action {
            'A' => self.buffer.cursor_up(Self::param1(params, 1)),
            'B' => self.buffer.cursor_down(Self::param1(params, 1)),
            'C' => self.buffer.cursor_forward(Self::param1(params, 1)),
            'D' => self.buffer.cursor_back(Self::param1(params, 1)),
            'E' => {
                // CNL – cursor next line
                let n = Self::param1(params, 1);
                self.buffer.cursor_down(n);
                self.buffer.cursor_position(self.buffer.cursor_row() + 1, 1);
            }
            'F' => {
                // CPL – cursor preceding line
                let n = Self::param1(params, 1);
                self.buffer.cursor_up(n);
                self.buffer.cursor_position(self.buffer.cursor_row() + 1, 1);
            }
            'G' => {
                // CHA – cursor horizontal absolute (1-based col)
                let col = Self::param1(params, 1);
                self.buffer
                    .cursor_position(self.buffer.cursor_row() + 1, col);
            }
            'H' | 'f' => {
                // CUP / HVP – cursor position row;col (1-based)
                let (row, col) = Self::param2(params, 1, 1);
                self.buffer.cursor_position(row, col);
            }
            'J' => self.buffer.erase_display(Self::param1(params, 0)),
            'K' => self.buffer.erase_line(Self::param1(params, 0)),
            'L' => self.buffer.insert_lines(Self::param1(params, 1)),
            'M' => self.buffer.delete_lines(Self::param1(params, 1)),
            'P' => self.buffer.delete_chars(Self::param1(params, 1)),
            // SGR (colors/styles) – not needed for plain-text snapshot
            'm' => {}
            // Private mode set/reset (DEC) – ignore for text extraction
            'h' | 'l' => {}
            _ => {}
        }
    }

    // All other Perform methods use the default no-op implementations.
}

// ── Public VteProcessor ───────────────────────────────────────────────────────

/// Wraps a [`vte::Parser`] and a [`Performer`] to provide incremental
/// processing of raw terminal byte streams and clean text snapshots.
pub struct VteProcessor {
    parser: Parser,
    performer: Performer,
}

impl VteProcessor {
    /// Create a processor with the given terminal dimensions.
    pub fn new(rows: usize, cols: usize) -> Self {
        VteProcessor {
            parser: Parser::new(),
            performer: Performer::new(rows, cols),
        }
    }

    /// Feed raw bytes from the PTY master into the state machine.
    pub fn process(&mut self, data: &[u8]) {
        for &byte in data {
            self.parser.advance(&mut self.performer, byte);
        }
    }

    /// Return a plain-text snapshot of the current screen content.
    pub fn snapshot(&self) -> String {
        self.performer.buffer.snapshot()
    }

    /// Resize the underlying screen buffer.
    pub fn resize(&mut self, rows: usize, cols: usize) {
        self.performer.buffer.resize(rows, cols);
    }

    /// Expose the underlying buffer for direct inspection (e.g. in tests).
    pub fn buffer(&self) -> &ScreenBuffer {
        &self.performer.buffer
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plain_text() {
        let mut vte = VteProcessor::new(5, 20);
        vte.process(b"hello world");
        assert_eq!(vte.snapshot(), "hello world");
    }

    #[test]
    fn test_strips_sgr_colors() {
        let mut vte = VteProcessor::new(5, 40);
        // Bold red "error"
        vte.process(b"\x1b[1;31merror\x1b[0m");
        assert_eq!(vte.snapshot(), "error");
    }

    #[test]
    fn test_cursor_movement() {
        let mut vte = VteProcessor::new(5, 20);
        vte.process(b"AAAAAA");
        // Move cursor back 3, overwrite with BBB
        vte.process(b"\x1b[3DBBB");
        assert_eq!(vte.snapshot(), "AAABBB");
    }

    #[test]
    fn test_erase_to_end_of_line() {
        let mut vte = VteProcessor::new(5, 20);
        vte.process(b"hello world");
        // Move to col 6 (1-based) and erase to end
        vte.process(b"\x1b[1;6H\x1b[K");
        assert_eq!(vte.snapshot(), "hello");
    }

    #[test]
    fn test_multiline() {
        let mut vte = VteProcessor::new(5, 20);
        vte.process(b"line1\r\nline2\r\nline3");
        let snap = vte.snapshot();
        assert!(snap.contains("line1"));
        assert!(snap.contains("line2"));
        assert!(snap.contains("line3"));
    }
}
