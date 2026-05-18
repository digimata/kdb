//! Terminal truecolor helpers for colorized text output.
//!
//! Parses `#RRGGBB` hex colors and emits 24-bit ANSI escape sequences
//! when stdout is a TTY; otherwise returns the plain text unchanged.

use std::io::IsTerminal;

// ---------------------------------------------------
// projects/kdb/src/color.rs
//
// pub fn stdout_is_tty()                          L22
// pub fn parse_hex()                              L27
// fn ansi_fg()                                    L39
// pub fn colorize_for_stdout()                    L45
// pub fn pad_colored()                            L57
// mod tests                                       L69
// fn parse_hex_accepts_with_and_without_hash()    L73
// fn parse_hex_rejects_bad_input()                L80
// ---------------------------------------------------

/// True when stdout is attached to a terminal.
pub fn stdout_is_tty() -> bool {
    std::io::stdout().is_terminal()
}

/// Parse `#RRGGBB` (or `RRGGBB`) into `(r, g, b)`. Returns `None` for malformed input.
pub fn parse_hex(s: &str) -> Option<(u8, u8, u8)> {
    let s = s.strip_prefix('#').unwrap_or(s);
    if s.len() != 6 || !s.chars().all(|c| c.is_ascii_hexdigit()) {
        return None;
    }
    let r = u8::from_str_radix(&s[0..2], 16).ok()?;
    let g = u8::from_str_radix(&s[2..4], 16).ok()?;
    let b = u8::from_str_radix(&s[4..6], 16).ok()?;
    Some((r, g, b))
}

/// Build a truecolor foreground SGR escape for the given RGB triple.
fn ansi_fg(r: u8, g: u8, b: u8) -> String {
    format!("\x1b[38;2;{r};{g};{b}m")
}

/// Wrap `text` in a truecolor escape when stdout is a TTY and `hex` parses.
/// Otherwise returns `text` unchanged (no allocation when possible).
pub fn colorize_for_stdout(text: &str, hex: Option<&str>) -> String {
    if !stdout_is_tty() {
        return text.to_string();
    }
    let Some(hex) = hex else { return text.to_string() };
    let Some((r, g, b)) = parse_hex(hex) else { return text.to_string() };
    format!("{}{text}\x1b[0m", ansi_fg(r, g, b))
}

/// Render `text` left-padded to `width` visible columns, with the colorized
/// form substituted for the visible content. Padding spaces are uncolored.
/// Use this for aligned table cells so escape sequences don't throw off widths.
pub fn pad_colored(text: &str, hex: Option<&str>, width: usize) -> String {
    let pad = width.saturating_sub(text.chars().count());
    let colored = colorize_for_stdout(text, hex);
    let mut out = String::with_capacity(colored.len() + pad);
    out.push_str(&colored);
    for _ in 0..pad {
        out.push(' ');
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_hex_accepts_with_and_without_hash() {
        assert_eq!(parse_hex("#ff0000"), Some((255, 0, 0)));
        assert_eq!(parse_hex("00FF00"), Some((0, 255, 0)));
        assert_eq!(parse_hex("#000080"), Some((0, 0, 128)));
    }

    #[test]
    fn parse_hex_rejects_bad_input() {
        assert_eq!(parse_hex(""), None);
        assert_eq!(parse_hex("#12345"), None);
        assert_eq!(parse_hex("#1234567"), None);
        assert_eq!(parse_hex("#gghhii"), None);
    }
}
