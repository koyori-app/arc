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

const NOTE_ROWS: &str = r#"[{"kind":"task","task_id":"a","state":"warn","note":"目安を過ぎた"},{"kind":"task","task_id":"b"}]"#;

#[test]
fn the_note_band_comes_after_the_today_line_in_the_svg_and_the_canvas() {
    // 今日（11/7）の線は a の棒の右の添えの字の所を通る
    let options = s(r#"{"today":"2026-11-07"}"#);
    let svg = render_svg(TASKS, "[]", options.clone(), s(NOTE_ROWS));
    let today = svg.find(r#"stroke-dasharray="4,3""#).expect("今日の線");
    let band = svg.find(r#"class="note-bg""#).expect("字の帯");
    let note = svg.find(">目安を過ぎた</text>").expect("添えの字");
    assert!(today < band && band < note, "SVG: 今日の線、帯、字の順");

    let canvas: serde_json::Value =
        serde_json::from_str(&render_canvas_commands(TASKS, "[]", options, s(NOTE_ROWS))).unwrap();
    let ops = canvas["ops"].as_array().unwrap();
    let today = ops
        .iter()
        .position(|op| op["StrokePolyline"]["dash"] == "4,3")
        .expect("今日の線");
    // NoteBg の色の番号は 22
    let band = ops
        .iter()
        .position(|op| op["FillRect"]["color_id"] == 22)
        .expect("字の帯");
    let note = ops
        .iter()
        .position(|op| op["DrawText"]["text"] == "目安を過ぎた")
        .expect("添えの字");
    assert!(today < band && band < note, "canvas: 今日の線、帯、字の順");
}

#[test]
fn the_note_band_takes_the_ground_colour_from_the_palette() {
    let plain = render_svg(TASKS, "[]", None, s(NOTE_ROWS));
    let painted = render_svg(
        TASKS,
        "[]",
        s(r##"{"palette":{"NoteBg":"#faf8f5"}}"##),
        s(NOTE_ROWS),
    );
    let fill_of = |svg: &str| {
        let at = svg.find(r#"class="note-bg""#).unwrap();
        let rest = &svg[at..];
        let start = rest.find(r#"fill=""#).unwrap() + r#"fill=""#.len();
        rest[start..start + rest[start..].find('"').unwrap()].to_string()
    };
    // 渡さぬ時は見出しの帯の色に落ちる
    assert_eq!(fill_of(&plain), "#f3f4f6");
    assert_eq!(fill_of(&painted), "#faf8f5");
}

#[test]
fn rows_without_notes_draw_no_band() {
    let rows = r#"[{"kind":"task","task_id":"a","state":"warn","hard_deadline":"2026-11-04"}]"#;
    let svg = render_svg(TASKS, "[]", s(r#"{"today":"2026-11-07"}"#), s(rows));
    assert!(!svg.contains("note-bg"));
}
