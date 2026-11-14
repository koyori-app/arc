//! `view` の読み方と断り方、canvas の上限への効き方。
//! 設計書 docs/design/view-range.md の 3 にあたる。

use koyori_arc_core::{
    render_canvas_commands, render_svg, render_svg_error, CODE_CANVAS_CAPACITY, CODE_PARSE_ERROR,
};

const TASKS: &str = r#"[{"id":"a","title":"A","progress_pct":50,"start":"2026-11-02","end":"2026-11-18"},{"id":"b","title":"B","progress_pct":0,"start":"2026-11-20","end":"2026-11-24"}]"#;

fn s(json: &str) -> Option<String> {
    Some(json.to_string())
}

fn width_of(svg: &str) -> f64 {
    let start = svg.find(" width=\"").unwrap() + " width=\"".len();
    let rest = &svg[start..];
    rest[..rest.find('"').unwrap()].parse().unwrap()
}

fn refused(options: &str) {
    let reported = render_svg_error(TASKS, "[]", s(options), None)
        .unwrap_or_else(|| panic!("{options}: must be refused"));
    let v: serde_json::Value = serde_json::from_str(&reported).unwrap();
    assert_eq!(v["code"], CODE_PARSE_ERROR, "{options}");
    assert!(
        v["error"].as_str().unwrap().contains("view"),
        "{options}: {v}"
    );
    let canvas: serde_json::Value =
        serde_json::from_str(&render_canvas_commands(TASKS, "[]", s(options), None)).unwrap();
    assert_eq!(canvas["error"], v["error"], "{options}");
}

#[test]
fn a_view_of_null_draws_the_chart_without_one() {
    let plain = render_svg(TASKS, "[]", None, None);
    assert_eq!(render_svg(TASKS, "[]", s(r#"{"view":null}"#), None), plain);
}

#[test]
fn a_view_is_read_and_draws_its_days() {
    let options = r#"{"view":{"from":"2026-11-09","to":"2026-11-15"},"legend":"hidden"}"#;
    assert_eq!(render_svg_error(TASKS, "[]", s(options), None), None);
    let svg = render_svg(TASKS, "[]", s(options), None);
    // 題の列 120px、七日 × 30px、右の余白 20px
    assert_eq!(width_of(&svg), 120.0 + 7.0 * 30.0 + 20.0);
    // a は範囲の前から後ろへ続き、b は範囲より後に始まる
    assert!(svg.contains(">← 11/2</text>"));
    assert!(svg.contains(">→ 11/17</text>"));
    assert!(svg.contains(">→ 11/20</text>"));
    // 一日だけの範囲も描ける
    let one = r#"{"view":{"from":"2026-11-09","to":"2026-11-09"}}"#;
    assert_eq!(render_svg_error(TASKS, "[]", s(one), None), None);
}

#[test]
fn a_view_the_chart_cannot_use_is_refused() {
    refused(r#"{"view":"2026-11-09"}"#);
    refused(r#"{"view":{"from":"2026-11-09"}}"#);
    refused(r#"{"view":{"from":"2026-11-09","to":"11/15"}}"#);
    refused(r#"{"view":{"from":"2026-11-09T00:00:00Z","to":"2026-11-15"}}"#);
    refused(r#"{"view":{"from":"2026-11-15","to":"2026-11-09"}}"#);
    refused(r#"{"view":{"from":"2026-11-09","to":"2026-11-15","days":7}}"#);
    refused(r#"{"view":{"from":"2026-01-01","to":"2040-01-01"}}"#);
}

#[test]
fn a_view_lets_the_canvas_draw_a_short_part_of_a_long_chart() {
    let long = r#"[{"id":"a","title":"A","progress_pct":0,"start":"2026-01-05","end":"2026-01-20"},{"id":"b","title":"B","progress_pct":0,"start":"2029-01-01","end":"2029-01-10"}]"#;
    let whole: serde_json::Value =
        serde_json::from_str(&render_canvas_commands(long, "[]", None, None)).unwrap();
    assert_eq!(whole["code"], CODE_CANVAS_CAPACITY);
    let part: serde_json::Value = serde_json::from_str(&render_canvas_commands(
        long,
        "[]",
        s(r#"{"view":{"from":"2026-01-05","to":"2026-02-28"}}"#),
        None,
    ))
    .unwrap();
    assert!(part.get("error").is_none_or(|e| e.is_null()), "{part}");
}

#[test]
fn the_today_line_follows_the_view_s_days_on_both_entry_points() {
    // 範囲 11/9〜11/15。今日の線は SVG では破線 4,3・太さ 2 の line、canvas では色の番号 8 の線
    let today_lines = |today: &str| {
        let options = format!(
            r#"{{"view":{{"from":"2026-11-09","to":"2026-11-15"}},"legend":"hidden","today":"{today}"}}"#
        );
        let svg = render_svg(TASKS, "[]", s(&options), None);
        let svg_count = svg
            .split("<line ")
            .skip(1)
            .filter(|l| {
                l.contains(r#"stroke-width="2""#) && l.contains(r#"stroke-dasharray="4,3""#)
            })
            .count();
        let canvas: serde_json::Value =
            serde_json::from_str(&render_canvas_commands(TASKS, "[]", s(&options), None)).unwrap();
        let canvas_count = canvas["ops"]
            .as_array()
            .unwrap_or_else(|| panic!("{canvas}"))
            .iter()
            .filter_map(|op| op.get("StrokePolyline"))
            .filter(|l| l["color_id"] == 8)
            .count();
        (svg_count, canvas_count)
    };
    assert_eq!(today_lines("2026-11-15"), (1, 1));
    assert_eq!(today_lines("2026-11-16"), (0, 0));
    assert_eq!(today_lines("2026-11-08"), (0, 0));
}
