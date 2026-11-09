//! 行の一覧の課題の行が持つ印の読み方と断り方。
//! 設計書 docs/design/bar-marks.md の 3 にあたる。

use koyori_arc_core::{render_canvas_commands, render_svg, render_svg_error, CODE_PARSE_ERROR};

const TASKS: &str = r#"[{"id":"a","title":"A","progress_pct":50,"start":"2026-11-02","end":"2026-11-06"},{"id":"b","title":"B","progress_pct":0,"start":"2026-11-05","end":"2026-11-10"}]"#;

fn s(json: &str) -> Option<String> {
    Some(json.to_string())
}

fn refused(rows: &str, mentions: &str) {
    let reported = render_svg_error(TASKS, "[]", None, s(rows))
        .unwrap_or_else(|| panic!("{rows}: must be refused"));
    let v: serde_json::Value = serde_json::from_str(&reported).unwrap();
    assert_eq!(v["code"], CODE_PARSE_ERROR, "{rows}");
    assert!(
        v["error"].as_str().unwrap().contains(mentions),
        "{rows}: {v}"
    );
    let canvas: serde_json::Value =
        serde_json::from_str(&render_canvas_commands(TASKS, "[]", None, s(rows))).unwrap();
    assert_eq!(canvas["error"], v["error"], "{rows}");
}

#[test]
fn rows_without_marks_draw_the_chart_without_rows() {
    let plain = render_svg(TASKS, "[]", None, None);
    let rows = r#"[{"kind":"task","task_id":"a","state":null,"note":null},{"kind":"task","task_id":"b","shape":"bar"}]"#;
    assert_eq!(render_svg(TASKS, "[]", None, s(rows)), plain);
}

#[test]
fn the_marks_are_read() {
    let rows = r#"[{"kind":"task","task_id":"a","state":"warn","hard_deadline":"2026-11-04","note":"目安が厳守を越えた"},{"kind":"task","task_id":"b","shape":"point","state":"done","hard_deadline":"2026-11-12"}]"#;
    let svg = render_svg(TASKS, "[]", None, s(rows));
    assert_eq!(svg.matches(r#"class="hard-deadline""#).count(), 2);
    assert!(svg.contains(r#"class="bar-point""#));
    assert!(svg.contains(">目安が厳守を越えた</text>"));
    assert_eq!(render_svg_error(TASKS, "[]", None, s(rows)), None);
}

#[test]
fn marks_the_chart_cannot_use_are_refused() {
    refused(r#"[{"kind":"task","task_id":"a","state":"late"}]"#, "rows");
    refused(r#"[{"kind":"task","task_id":"a","shape":"dot"}]"#, "rows");
    refused(
        r#"[{"kind":"task","task_id":"a","hard_deadline":"11/12"}]"#,
        "rows",
    );
    refused(
        &format!(
            r#"[{{"kind":"task","task_id":"a","note":"{}"}}]"#,
            "x".repeat(201)
        ),
        "note of `a`",
    );
    refused(
        r#"[{"kind":"task","task_id":"a","hard_deadline":"2040-01-01"}]"#,
        "hard_deadline of `a`",
    );
}

#[test]
fn a_note_is_escaped_like_any_text() {
    let rows = r#"[{"kind":"task","task_id":"a","note":"<b>&\""}]"#;
    let svg = render_svg(TASKS, "[]", None, s(rows));
    assert!(svg.contains("&lt;b&gt;&amp;"));
    assert!(!svg.contains("<b>"));
}
