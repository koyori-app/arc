//! 日の段の日の数字の色。色の名 `DayNumber` を `palette` で渡した時だけ、その色で描く。
//! 渡さぬ時は、名の無かった頃と同じく字に色を書かぬ（SVG は継ぐ色、canvas は色の番号 255）。

use koyori_arc_core::{render_canvas_commands, render_svg};

const TASKS: &str =
    r#"[{"id":"a","title":"A","progress_pct":50,"start":"2026-11-02","end":"2026-11-06"}]"#;
const DAY: &str = r#""scale":"day","px_per_day":32,"header_height_px":48"#;

fn options(palette: &str) -> Option<String> {
    Some(format!("{{{DAY}{palette}}}"))
}

/// canvas の日の数字（最初の日の 2）の色の番号。
fn canvas_color_of_day_two(options: Option<String>) -> u64 {
    let canvas: serde_json::Value =
        serde_json::from_str(&render_canvas_commands(TASKS, "[]", options, None)).unwrap();
    canvas["ops"]
        .as_array()
        .unwrap()
        .iter()
        .find(|op| op["DrawText"]["text"] == "2")
        .expect("日の数字 2")["DrawText"]["color_id"]
        .as_u64()
        .unwrap()
}

#[test]
fn a_day_number_without_its_colour_is_drawn_with_no_colour_of_its_own() {
    let svg = render_svg(TASKS, "[]", options(""), None);
    assert!(
        svg.contains(r#"text-anchor="middle" font-size="11">2</text>"#),
        "{svg}"
    );
    assert_eq!(canvas_color_of_day_two(options("")), 255);
}

#[test]
fn a_day_number_takes_its_colour_from_the_palette() {
    let palette = r#","palette":{"DayNumber":"var(--day-number)"}"#;
    let svg = render_svg(TASKS, "[]", options(palette), None);
    assert!(
        svg.contains(r#"text-anchor="middle" fill="var(--day-number)" font-size="11">2</text>"#),
        "{svg}"
    );
    // 曜日の字は今までどおりグリッドラベルの色
    assert!(
        svg.contains(r##"fill="#6b7280" font-size="11">Mo</text>"##),
        "{svg}"
    );
    // DayNumber の色の番号は 24
    assert_eq!(canvas_color_of_day_two(options(palette)), 24);
}
