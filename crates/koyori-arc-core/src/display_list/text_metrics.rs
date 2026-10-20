//! 文字の幅の見積もり。
//!
//! 描く側（SVG の `<text>`、Canvas の `fillText`）の実測の幅は、字体と環境で変わる。
//! wasm の中には字体が無いので測れない。そこで、字の種類ごとの幅（em の割合）を
//! 表にして見積もる。目的は「ラベル欄や凡例の枠から字がはみ出さぬこと」なので、
//! 迷う字は広めに数える。
//!
//! 数え方を `str::len()`（UTF-8 のバイト数）にしてはならない。和文は 1 字が 3 バイトで、
//! 幅は約 3 倍に出る。

/// 1 字の幅（em の割合）。`1.0` は全角。
pub fn glyph_em(c: char) -> f64 {
    let cp = c as u32;
    let wide = matches!(
        cp,
        0x1100..=0x115F
            | 0x2026 // …
            | 0x2E80..=0xA4CF // CJK の部首・記号・かな・漢字
            | 0xAC00..=0xD7A3 // ハングル
            | 0xF900..=0xFAFF
            | 0xFE30..=0xFE6F
            | 0xFF00..=0xFF60 // 全角形
            | 0xFFE0..=0xFFE6
            | 0x1F300..=0x1FAFF // 絵文字
            | 0x20000..=0x3FFFD
    );
    if wide {
        return 1.0;
    }
    match c {
        ' ' | '.' | ',' | ':' | ';' | '!' | '|' | '\'' | '`' | 'i' | 'j' | 'l' => 0.3,
        '(' | ')' | '[' | ']' | '{' | '}' | '-' | '/' | '\\' | 't' | 'f' | 'r' | 'I' | '1' => 0.38,
        'm' | 'w' | 'M' | 'W' | '@' | '%' => 0.85,
        'A'..='Z' => 0.68,
        _ => 0.56,
    }
}

/// `s` を `font_px` の大きさで並べた時の幅の見積もり（px）。
pub fn text_width_px(s: &str, font_px: f64) -> f64 {
    s.chars().map(glyph_em).sum::<f64>() * font_px
}

/// 幅が `max_width_px` に収まるまで切り、切った時は末尾に `…` を付ける。
/// `…` を含めた幅が上限を越えぬ。収まるなら `s` のまま返す。
pub fn truncate_to_width(s: &str, font_px: f64, max_width_px: f64) -> String {
    if text_width_px(s, font_px) <= max_width_px {
        return s.to_string();
    }
    let ellipsis_w = glyph_em('…') * font_px;
    let mut kept = String::new();
    let mut width = 0.0;
    for c in s.chars() {
        let w = glyph_em(c) * font_px;
        if width + w + ellipsis_w > max_width_px {
            break;
        }
        kept.push(c);
        width += w;
    }
    kept.push('…');
    kept
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_width_is_one_em_and_ascii_is_narrower() {
        assert_eq!(text_width_px("未達", 9.0), 18.0);
        assert!(text_width_px("ab", 12.0) < 2.0 * 12.0);
    }

    #[test]
    fn width_is_by_glyph_not_by_utf8_byte() {
        // 「未達」は 6 バイトだが 2 字である。
        assert_eq!("未達".len(), 6);
        assert_eq!(text_width_px("未達", 10.0), 20.0);
    }

    #[test]
    fn truncation_fits_the_budget_with_the_ellipsis() {
        let title = "とても長い題名のタスクでどこまで表示されるかを確かめるための題";
        let out = truncate_to_width(title, 12.0, 112.0);
        assert!(out.ends_with('…'));
        assert!(text_width_px(&out, 12.0) <= 112.0, "{out}");
        // 和文 12px は 1 字 12px。112px には 8 字と … が入る。
        assert_eq!(out.chars().count(), 9);
    }

    #[test]
    fn short_text_is_returned_as_is() {
        assert_eq!(truncate_to_width("短い", 12.0, 112.0), "短い");
        assert_eq!(truncate_to_width("", 12.0, 112.0), "");
    }

    #[test]
    fn a_budget_smaller_than_the_ellipsis_still_returns_one() {
        assert_eq!(truncate_to_width("あいう", 12.0, 5.0), "…");
    }
}
