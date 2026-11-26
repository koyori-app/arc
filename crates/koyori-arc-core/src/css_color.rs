//! The CSS colours the chart writes into SVG attributes and canvas styles: the
//! values of `palette` and the `color` of a task row.

use crate::render::MAX_COLOR_CHARS;

/// The colour forms `palette` takes: `#rgb`, `#rgba`, `#rrggbb`, `#rrggbbaa`;
/// `var(--name)` with a name of letters, digits, `-` and `_`; and `rgb()`,
/// `rgba()`, `hsl()`, `hsla()`, `oklch()`, `oklab()` whose arguments are
/// digits, lower-case letters, `.`, `,`, `%`, `/`, `+`, `-` and spaces. None
/// of these can close an attribute or open another.
pub(crate) fn is_css_color(c: &str) -> bool {
    if c.is_empty() || c.chars().count() > MAX_COLOR_CHARS {
        return false;
    }
    if let Some(hex) = c.strip_prefix('#') {
        return matches!(hex.len(), 3 | 4 | 6 | 8) && hex.chars().all(|ch| ch.is_ascii_hexdigit());
    }
    if let Some(name) = c.strip_prefix("var(--").and_then(|r| r.strip_suffix(')')) {
        return !name.is_empty()
            && name
                .chars()
                .all(|ch| ch.is_ascii_alphanumeric() || ch == '-' || ch == '_');
    }
    for f in ["rgb(", "rgba(", "hsl(", "hsla(", "oklch(", "oklab("] {
        if let Some(args) = c.strip_prefix(f).and_then(|r| r.strip_suffix(')')) {
            return args.chars().all(|ch| {
                ch.is_ascii_digit()
                    || ch.is_ascii_lowercase()
                    || matches!(ch, '.' | ',' | '%' | '/' | '+' | '-' | ' ')
            });
        }
    }
    false
}
