//! `progress_line` の読み方と断り方。
//! 設計書 docs/design/progress-line.md の 3 にあたる。

use koyori_arc_core::{render_canvas_commands, render_svg, render_svg_error, CODE_PARSE_ERROR};

const TASKS: &str = r#"[{"id":"a","title":"A","progress_pct":20,"start":"2026-11-02","end":"2026-11-12"},{"id":"b","title":"B","progress_pct":100,"start":"2026-11-02","end":"2026-11-06"}]"#;

fn s(json: &str) -> Option<String> {
    Some(json.to_string())
}

#[test]
fn shown_and_null_draw_the_chart_without_the_key() {
    let options = r#"{"today":"2026-11-07"}"#;
    let plain = render_svg(TASKS, "[]", s(options), None);
    for with in [
        r#"{"today":"2026-11-07","progress_line":"shown"}"#,
        r#"{"today":"2026-11-07","progress_line":null}"#,
    ] {
        assert_eq!(render_svg(TASKS, "[]", s(with), None), plain, "{with}");
    }
}

#[test]
fn hidden_and_by_row_are_read() {
    let hidden = render_svg(
        TASKS,
        "[]",
        s(r#"{"today":"2026-11-07","progress_line":"hidden"}"#),
        None,
    );
    assert!(!hidden.contains("<polyline"));
    let by_row = render_svg(
        TASKS,
        "[]",
        s(r#"{"today":"2026-11-07","progress_line":"by_row"}"#),
        None,
    );
    assert!(by_row.contains("<polyline"));
    // 遅れの a と、100% でも済みでない b の折れ目に点が一つずつ
    assert_eq!(by_row.matches(r#"class="progress-point""#).count(), 2);
    // b の行を done にすれば、b は今日の線のまま
    let rows = r#"[{"kind":"task","task_id":"a"},{"kind":"task","task_id":"b","state":"done"}]"#;
    let done = render_svg(
        TASKS,
        "[]",
        s(r#"{"today":"2026-11-07","progress_line":"by_row"}"#),
        s(rows),
    );
    assert_eq!(done.matches(r#"class="progress-point""#).count(), 1);
}

#[test]
fn a_mode_the_chart_does_not_know_is_refused() {
    for options in [r#"{"progress_line":"rows"}"#, r#"{"progress_line":true}"#] {
        let reported = render_svg_error(TASKS, "[]", s(options), None)
            .unwrap_or_else(|| panic!("{options}: must be refused"));
        let v: serde_json::Value = serde_json::from_str(&reported).unwrap();
        assert_eq!(v["code"], CODE_PARSE_ERROR, "{options}");
        assert!(
            v["error"].as_str().unwrap().contains("progress_line"),
            "{v}"
        );
        let canvas: serde_json::Value =
            serde_json::from_str(&render_canvas_commands(TASKS, "[]", s(options), None)).unwrap();
        assert_eq!(canvas["error"], v["error"], "{options}");
    }
}
