/// Default terminal width in columns.
pub const DEFAULT_COLS: usize = 80;
/// Default terminal height in rows.
pub const DEFAULT_ROWS: usize = 24;

/// A single terminal cell.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Cell {
    pub c: char,
}

impl Default for Cell {
    fn default() -> Self {
        Cell { c: ' ' }
    }
}

/// A two-dimensional character grid that models a terminal screen.
///
/// Tracks cursor position, handles scrolling, and implements the most common
/// cursor-movement and erase operations needed to reconstruct a readable
/// snapshot from raw VT output.
#[derive(Debug, Clone)]
pub struct ScreenBuffer {
    rows: usize,
    cols: usize,
    cells: Vec<Vec<Cell>>,
    cursor_row: usize,
    cursor_col: usize,
}

impl ScreenBuffer {
    /// Create a new buffer with the given dimensions.
    pub fn new(rows: usize, cols: usize) -> Self {
        let rows = rows.max(1);
        let cols = cols.max(1);
        ScreenBuffer {
            rows,
            cols,
            cells: vec![vec![Cell::default(); cols]; rows],
            cursor_row: 0,
            cursor_col: 0,
        }
    }

    // ── cursor helpers ──────────────────────────────────────────────────────

    pub fn cursor_row(&self) -> usize {
        self.cursor_row
    }

    pub fn cursor_col(&self) -> usize {
        self.cursor_col
    }

    pub fn rows(&self) -> usize {
        self.rows
    }

    pub fn cols(&self) -> usize {
        self.cols
    }

    // ── character printing ──────────────────────────────────────────────────

    /// Write a character at the current cursor position and advance it.
    pub fn print(&mut self, c: char) {
        if self.cursor_col < self.cols {
            self.cells[self.cursor_row][self.cursor_col] = Cell { c };
            self.cursor_col += 1;
        }
        // Implicit line wrap
        if self.cursor_col >= self.cols {
            self.cursor_col = 0;
            self.newline();
        }
    }

    // ── C0 / C1 control codes ───────────────────────────────────────────────

    /// Handle a C0/C1 control byte.
    pub fn execute(&mut self, byte: u8) {
        match byte {
            0x08 => {
                // BS – backspace
                if self.cursor_col > 0 {
                    self.cursor_col -= 1;
                }
            }
            0x09 => {
                // HT – horizontal tab (next 8-column stop)
                let next = (self.cursor_col / 8 + 1) * 8;
                self.cursor_col = next.min(self.cols - 1);
            }
            0x0a | 0x0b | 0x0c => {
                // LF / VT / FF – line feed
                self.newline();
            }
            0x0d => {
                // CR – carriage return
                self.cursor_col = 0;
            }
            _ => {}
        }
    }

    // ── CSI sequences ───────────────────────────────────────────────────────

    /// CSI A – cursor up.
    pub fn cursor_up(&mut self, n: usize) {
        self.cursor_row = self.cursor_row.saturating_sub(n.max(1));
    }

    /// CSI B – cursor down.
    pub fn cursor_down(&mut self, n: usize) {
        self.cursor_row = (self.cursor_row + n.max(1)).min(self.rows - 1);
    }

    /// CSI C – cursor forward (right).
    pub fn cursor_forward(&mut self, n: usize) {
        self.cursor_col = (self.cursor_col + n.max(1)).min(self.cols - 1);
    }

    /// CSI D – cursor backward (left).
    pub fn cursor_back(&mut self, n: usize) {
        self.cursor_col = self.cursor_col.saturating_sub(n.max(1));
    }

    /// CSI H / CSI f – cursor position (1-based row, col).
    pub fn cursor_position(&mut self, row: usize, col: usize) {
        self.cursor_row = row.saturating_sub(1).min(self.rows - 1);
        self.cursor_col = col.saturating_sub(1).min(self.cols - 1);
    }

    /// CSI J – erase in display.
    pub fn erase_display(&mut self, mode: usize) {
        match mode {
            0 => {
                // From cursor to end
                for col in self.cursor_col..self.cols {
                    self.cells[self.cursor_row][col] = Cell::default();
                }
                for row in (self.cursor_row + 1)..self.rows {
                    self.fill_row(row);
                }
            }
            1 => {
                // From start to cursor
                for row in 0..self.cursor_row {
                    self.fill_row(row);
                }
                for col in 0..=self.cursor_col.min(self.cols - 1) {
                    self.cells[self.cursor_row][col] = Cell::default();
                }
            }
            2 | 3 => {
                // Entire display
                for row in 0..self.rows {
                    self.fill_row(row);
                }
                self.cursor_row = 0;
                self.cursor_col = 0;
            }
            _ => {}
        }
    }

    /// CSI K – erase in line.
    pub fn erase_line(&mut self, mode: usize) {
        match mode {
            0 => {
                for col in self.cursor_col..self.cols {
                    self.cells[self.cursor_row][col] = Cell::default();
                }
            }
            1 => {
                for col in 0..=self.cursor_col.min(self.cols - 1) {
                    self.cells[self.cursor_row][col] = Cell::default();
                }
            }
            2 => {
                self.fill_row(self.cursor_row);
            }
            _ => {}
        }
    }

    /// CSI P – delete characters (shift left).
    pub fn delete_chars(&mut self, n: usize) {
        let n = n.max(1).min(self.cols - self.cursor_col);
        let row = self.cursor_row;
        let col = self.cursor_col;
        self.cells[row].drain(col..col + n);
        self.cells[row].resize(self.cols, Cell::default());
    }

    /// CSI L – insert lines.
    pub fn insert_lines(&mut self, n: usize) {
        let n = n.max(1);
        for _ in 0..n {
            if self.cursor_row < self.rows {
                self.cells.insert(self.cursor_row, vec![Cell::default(); self.cols]);
                if self.cells.len() > self.rows {
                    self.cells.pop();
                }
            }
        }
    }

    /// CSI M – delete lines.
    pub fn delete_lines(&mut self, n: usize) {
        let n = n.max(1);
        for _ in 0..n {
            if self.cursor_row < self.cells.len() {
                self.cells.remove(self.cursor_row);
                self.cells.push(vec![Cell::default(); self.cols]);
            }
        }
    }

    // ── resize ──────────────────────────────────────────────────────────────

    /// Resize the buffer to new dimensions, preserving existing content.
    pub fn resize(&mut self, rows: usize, cols: usize) {
        let rows = rows.max(1);
        let cols = cols.max(1);
        // Adjust each row's width
        for row in &mut self.cells {
            row.resize(cols, Cell::default());
        }
        // Adjust row count
        while self.cells.len() > rows {
            self.cells.pop();
        }
        while self.cells.len() < rows {
            self.cells.push(vec![Cell::default(); cols]);
        }
        self.rows = rows;
        self.cols = cols;
        self.cursor_row = self.cursor_row.min(rows - 1);
        self.cursor_col = self.cursor_col.min(cols - 1);
    }

    // ── snapshot ─────────────────────────────────────────────────────────────

    /// Render the current screen as a plain-text string.
    ///
    /// Trailing whitespace on each line is stripped; trailing empty lines are
    /// also stripped.
    pub fn snapshot(&self) -> String {
        let mut lines: Vec<String> = self
            .cells
            .iter()
            .map(|row| {
                let s: String = row.iter().map(|c| c.c).collect();
                s.trim_end().to_string()
            })
            .collect();
        // Drop trailing empty lines
        while lines.last().map(|l: &String| l.is_empty()).unwrap_or(false) {
            lines.pop();
        }
        lines.join("\n")
    }

    // ── private helpers ──────────────────────────────────────────────────────

    fn newline(&mut self) {
        if self.cursor_row + 1 < self.rows {
            self.cursor_row += 1;
        } else {
            // Scroll up: discard top line, add blank at bottom
            self.cells.remove(0);
            self.cells.push(vec![Cell::default(); self.cols]);
        }
    }

    fn fill_row(&mut self, row: usize) {
        for col in 0..self.cols {
            self.cells[row][col] = Cell::default();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_print_and_snapshot() {
        let mut buf = ScreenBuffer::new(3, 10);
        for c in "hello".chars() {
            buf.print(c);
        }
        assert_eq!(buf.snapshot(), "hello");
    }

    #[test]
    fn test_cursor_position() {
        let mut buf = ScreenBuffer::new(5, 10);
        buf.cursor_position(2, 3); // 1-based → row 1, col 2
        assert_eq!(buf.cursor_row(), 1);
        assert_eq!(buf.cursor_col(), 2);
    }

    #[test]
    fn test_erase_line() {
        let mut buf = ScreenBuffer::new(3, 10);
        for c in "hello".chars() {
            buf.print(c);
        }
        buf.cursor_position(1, 1); // move to start of first row
        buf.erase_line(2); // erase entire line
        assert_eq!(buf.snapshot(), "");
    }

    #[test]
    fn test_scroll_up_on_newline() {
        let mut buf = ScreenBuffer::new(2, 5);
        for c in "AAAAA".chars() {
            buf.print(c);
        }
        // cursor wraps → newline
        for c in "BBBBB".chars() {
            buf.print(c);
        }
        // next char forces scroll
        buf.print('C');
        let snap = buf.snapshot();
        // After scroll, "AAAAA" should be gone; "BBBBB" is row 0
        assert!(snap.contains("BBBBB"));
        assert!(!snap.contains("AAAAA"));
    }

    #[test]
    fn test_resize() {
        let mut buf = ScreenBuffer::new(3, 10);
        buf.resize(5, 20);
        assert_eq!(buf.rows(), 5);
        assert_eq!(buf.cols(), 20);
    }

    #[test]
    fn test_carriage_return_overwrites() {
        let mut buf = ScreenBuffer::new(3, 10);
        for c in "hello".chars() {
            buf.print(c);
        }
        buf.execute(0x0d); // CR
        for c in "world".chars() {
            buf.print(c);
        }
        assert_eq!(buf.snapshot(), "world");
    }
}
