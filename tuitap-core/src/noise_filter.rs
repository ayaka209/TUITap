use std::sync::LazyLock;

use regex::Regex;

// ── Static compiled patterns ──────────────────────────────────────────────────

/// All ANSI / VT escape sequences: CSI, OSC, SS2, SS3, DCS, ESC + char.
static ANSI_ESCAPE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(concat!(
        r"(?x)",
        // CSI sequences: ESC [ ... final-byte
        r"\x1b\[[0-?]*[ -/]*[@-~]",
        r"|",
        // OSC sequences: ESC ] ... ST or BEL
        r"\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)",
        r"|",
        // Simple two-byte ESC sequences
        r"\x1b[@-_]",
        r"|",
        // Stray ESC + printable
        r"\x1b.",
    ))
    .expect("ANSI_ESCAPE regex is valid")
});

/// Common ASCII spinner frames: |, /, -, \
static ASCII_SPINNER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?:^|\s)[|/\\\-](?:\s|$)").expect("ASCII_SPINNER regex is valid")
});

/// Braille spinner characters often used by CLI tools (e.g. yarn, npm).
static BRAILLE_SPINNER: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[⠋⠙⠹⠸⠼⠴⠦⠧⠇⠏]+").expect("BRAILLE_SPINNER regex is valid")
});

/// Unicode block / shade chars used in progress bars.
static BLOCK_PROGRESS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[█▓▒░▪▫◆◇●○►▶]+").expect("BLOCK_PROGRESS regex is valid")
});

/// ASCII progress bar: [####---] or (====>  )
static ASCII_PROGRESS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"[\[\(][#=>\-\s]{3,}[\]\)]\s*\d*\s*%?").expect("ASCII_PROGRESS regex is valid")
});

/// Sequences of more than two consecutive blank lines, collapsed to two.
static MULTI_BLANK: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"\n{3,}").expect("MULTI_BLANK regex is valid")
});

// ── NoiseFilter ───────────────────────────────────────────────────────────────

/// Configuration for the noise filter.
#[derive(Debug, Clone)]
pub struct NoiseFilterConfig {
    /// Strip ANSI / VT escape sequences (default: true).
    pub strip_ansi: bool,
    /// Strip spinner characters (default: true).
    pub strip_spinners: bool,
    /// Strip progress-bar patterns (default: true).
    pub strip_progress_bars: bool,
    /// Collapse runs of blank lines to at most two (default: true).
    pub collapse_blank_lines: bool,
}

impl Default for NoiseFilterConfig {
    fn default() -> Self {
        NoiseFilterConfig {
            strip_ansi: true,
            strip_spinners: true,
            strip_progress_bars: true,
            collapse_blank_lines: true,
        }
    }
}

/// Cleans raw terminal output into human-readable plain text by removing
/// ANSI escapes, spinner frames, and progress-bar noise.
#[derive(Debug, Clone)]
pub struct NoiseFilter {
    config: NoiseFilterConfig,
}

impl Default for NoiseFilter {
    fn default() -> Self {
        NoiseFilter::new(NoiseFilterConfig::default())
    }
}

impl NoiseFilter {
    pub fn new(config: NoiseFilterConfig) -> Self {
        NoiseFilter { config }
    }

    /// Apply all configured filter passes to `input` and return clean text.
    pub fn filter(&self, input: &str) -> String {
        let mut s = input.to_owned();

        if self.config.strip_ansi {
            s = ANSI_ESCAPE.replace_all(&s, "").into_owned();
        }

        // Normalise line endings: CRLF → LF, then lone CR → LF.
        s = s.replace("\r\n", "\n");
        s = s.replace('\r', "\n");

        if self.config.strip_spinners {
            s = BRAILLE_SPINNER.replace_all(&s, "").into_owned();
            // Only strip standalone ASCII spinner chars, not legitimate - or /
            s = ASCII_SPINNER.replace_all(&s, " ").into_owned();
        }

        if self.config.strip_progress_bars {
            s = BLOCK_PROGRESS.replace_all(&s, "").into_owned();
            s = ASCII_PROGRESS.replace_all(&s, "").into_owned();
        }

        if self.config.collapse_blank_lines {
            s = MULTI_BLANK.replace_all(&s, "\n\n").into_owned();
        }

        // Trim trailing whitespace from every line
        let s: String = s
            .lines()
            .map(|l| l.trim_end().to_string())
            .collect::<Vec<_>>()
            .join("\n");

        s.trim().to_string()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn filter(s: &str) -> String {
        NoiseFilter::default().filter(s)
    }

    #[test]
    fn test_strips_ansi_sgr() {
        assert_eq!(filter("\x1b[1;32mhello\x1b[0m"), "hello");
    }

    #[test]
    fn test_strips_csi_cursor() {
        assert_eq!(filter("ab\x1b[2Kcd"), "abcd");
    }

    #[test]
    fn test_strips_braille_spinner() {
        assert_eq!(filter("⠙ Loading..."), "Loading...");
    }

    #[test]
    fn test_strips_block_progress() {
        let input = "Downloading █████░░░░░ 50%";
        let out = filter(input);
        assert!(!out.contains('█'));
        assert!(!out.contains('░'));
    }

    #[test]
    fn test_strips_ascii_progress_bar() {
        let out = filter("Progress: [=========>    ] 70%");
        assert!(!out.contains('['));
    }

    #[test]
    fn test_lone_cr_becomes_newline() {
        let out = filter("abc\rdef");
        // "abc" was overwritten: after lone-CR replacement we get "abc\ndef"
        assert!(out.contains("def"));
    }

    #[test]
    fn test_collapse_blank_lines() {
        let out = filter("a\n\n\n\n\nb");
        assert_eq!(out, "a\n\nb");
    }

    #[test]
    fn test_plain_text_unchanged() {
        let input = "Hello, world!\nThis is line 2.";
        assert_eq!(filter(input), input);
    }

    #[test]
    fn test_osc_stripped() {
        // OSC 0 (window title)
        assert_eq!(filter("\x1b]0;my title\x07remaining"), "remaining");
    }
}
