//! 文字の幅の見積もり。
//!
//! 描く側（SVG の `<text>`、Canvas の `fillText`）の実測の幅は、字体と環境で変わる。
//! wasm の中には字体が無いので測れない。そこで、字の種類ごとの幅（em の割合）を
//! 表にして見積もる。目的は「ラベル欄や凡例の枠から字がはみ出さぬこと」なので、
//! 迷う字は広めに数える。
//!
//! 広めに数えて失うのは、題が少し早く切れることだけである。
//! 狭めに数えて失うのは、図が viewBox からはみ出して頭の字が消えることである。
//!
//! 「迷う字」の筋は UAX #11（East Asian Width）の A（曖昧）である。
//! A は、字体しだいで半角にも全角にも描かれる。
//! 手で帯を選ぶと漏れるので、A の判定は `unicode-width` の表（`width_cjk`）に任せる。
//! この表の A は記号と句読点（① → ※ ■ ± × ° など）で、文字は含まない。
//! 含まない文字（ギリシャ文字、キリル文字）は、下の帯で別に数える。
//!
//! 記号と句読点の A は全角に数える。
//! 日本語の環境では全角の字体で描かれ、欧文の字体には字が無く全角の字体へ落ちるからである。
//!
//! ギリシャ文字とキリル文字は、日本語の字体では全角に近く、欧文の字体では比例幅で描かれる。
//! 全角に数えると、ロシア語やギリシャ語の題が 9 字ほどで切れてしまう。
//! 既定の 0.56 に任せると、全角に近い字体で 3 割ほど超える。
//! そこで、表にある比例幅のうち最も広い値（`m` `w` `W` と同じ 0.85）に数える。
//! アクセント付きのラテン文字は、字体が変わっても比例幅の字形で描かれるので、既定のままにする。
//!
//! 数え方を `str::len()`（UTF-8 のバイト数）にしてはならない。和文は 1 字が 3 バイトで、
//! 幅は約 3 倍に出る。
//!
//! 表が正しいかどうかは、実際の字体では確かめられない。
//! 試験が縛るのは、表の値そのもの（この字はこの値）と、字を分ける規則である。

use unicode_width::UnicodeWidthChar;

/// ギリシャ文字とキリル文字に当てる幅（em の割合）。表にある比例幅のうち最も広い値。
const GREEK_CYRILLIC_EM: f64 = 0.85;

/// UAX #11 の A（曖昧）か。W と F（常に全角）は含まない。
fn is_ambiguous_width(c: char) -> bool {
    c.width_cjk() == Some(2) && c.width() != Some(2)
}

/// 1 字の幅（em の割合）。`1.0` は全角。
pub fn glyph_em(c: char) -> f64 {
    let cp = c as u32;
    let wide = matches!(
        cp,
        0x1100..=0x115F
            | 0x2026 // …
            | 0x2150..=0x2BFF // ローマ数字、矢印、数学記号、囲み文字、罫線、図形、記号、装飾記号。A でない字も、全角の字体へ落ちる
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
    if is_ambiguous_width(c) {
        return 1.0;
    }
    // ギリシャ文字（0x0370..）からキリル文字の補助（..0x052F）まで
    if (0x0370..=0x052F).contains(&cp) {
        return GREEK_CYRILLIC_EM;
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

    /// 表の値を字ごとに名指しで縛る。
    /// 字体の実測とは突き合わせられないので、表が黙って変わった時に落ちることだけを守る。
    #[test]
    fn the_table_is_pinned_per_glyph() {
        let pinned: &[(char, f64)] = &[
            // 全角（W と F）
            ('未', 1.0),
            ('あ', 1.0),
            ('ア', 1.0),
            ('Ａ', 1.0),
            ('（', 1.0),
            ('한', 1.0),
            ('😀', 1.0),
            // 曖昧幅の記号と句読点（A）
            ('①', 1.0),
            ('⑩', 1.0),
            ('→', 1.0),
            ('←', 1.0),
            ('※', 1.0),
            ('■', 1.0),
            ('□', 1.0),
            ('●', 1.0),
            ('★', 1.0),
            ('±', 1.0),
            ('×', 1.0),
            ('÷', 1.0),
            ('°', 1.0),
            ('§', 1.0),
            ('\u{00B7}', 1.0), // MIDDLE DOT
            ('…', 1.0),
            ('\u{2014}', 1.0), // EM DASH
            ('“', 1.0),
            ('Ⅳ', 1.0),
            // A でないが全角の字体へ落ちる記号
            ('✓', 1.0),
            ('⚠', 1.0),
            ('⬆', 1.0),
            // ギリシャ文字とキリル文字
            ('Σ', 0.85),
            ('λ', 0.85),
            ('Ж', 0.85),
            ('я', 0.85),
            // アクセント付きのラテン文字は既定のまま
            ('é', 0.56),
            ('ø', 0.56),
            // 曖昧幅でない字
            ('a', 0.56),
            ('Z', 0.68),
            ('W', 0.85),
            ('i', 0.3),
            ('(', 0.38),
            (' ', 0.3),
            ('5', 0.56),
        ];
        for (c, want) in pinned {
            assert_eq!(glyph_em(*c), *want, "U+{:04X} {c}", *c as u32);
        }
    }

    #[test]
    fn circled_digits_and_an_arrow_are_counted_as_full_width() {
        assert_eq!(text_width_px("①②③④⑤⑥⑦⑧⑨⑩", 12.0), 120.0);
        assert_eq!(text_width_px("→", 12.0), 12.0);
        assert_eq!(text_width_px("※要確認", 12.0), 48.0);
    }

    /// 曖昧幅の字が、全角の字より狭く数えられることは無い。
    /// 範囲は `unicode-width` の表から引くので、表が更新されても漏れない。
    #[test]
    fn no_ambiguous_width_glyph_is_counted_narrower_than_full_width() {
        let mut seen = 0;
        for cp in 0..=0x2FFFFu32 {
            let Some(c) = char::from_u32(cp) else {
                continue;
            };
            if is_ambiguous_width(c) {
                seen += 1;
                assert_eq!(glyph_em(c), 1.0, "U+{cp:04X} {c}");
            }
        }
        // 表が空になって何も検めぬ試験になるのを防ぐ。
        assert!(seen > 100, "{seen}");
    }

    /// ギリシャ文字とキリル文字は、どのラテン文字より狭く数えられることは無い。
    #[test]
    fn greek_and_cyrillic_are_not_narrower_than_any_latin_glyph() {
        let widest_latin = ('!'..='~').map(glyph_em).fold(0.0, f64::max);
        for cp in (0x0370..=0x052Fu32).filter_map(char::from_u32) {
            assert!(glyph_em(cp) >= widest_latin, "U+{:04X} {cp}", cp as u32);
        }
    }

    #[test]
    fn truncation_keeps_ambiguous_symbols_inside_the_budget() {
        // 112px は全角 9 字分。① は 1 字 12px で、8 字と … が入る。
        let out = truncate_to_width("①②③④⑤⑥⑦⑧⑨⑩", 12.0, 112.0);
        assert_eq!(out, "①②③④⑤⑥⑦⑧…");
    }
}
