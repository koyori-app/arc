//! `options_json`。Wasm の入口が受ける設定の読み方。
//!
//! 省略と `{}` は既定、読めない物は黙って捨てずに断る。

use chrono::NaiveDate;
use koyori_arc_core::{
    empty_svg, render, render_canvas_commands, render_svg, render_svg_error, ChartOptions,
    GanttDep, GanttTask, ScrollViewport, CODE_INPUT_LIMIT, CODE_PARSE_ERROR,
};

const TASKS: &str = r#"[{"id":"a","title":"A","progress_pct":50,"start":"2026-06-01","end":"2026-06-08"},{"id":"b","title":"B","progress_pct":10,"start":"2026-06-02","end":"2026-06-05"}]"#;

fn s(json: &str) -> Option<String> {
    Some(json.to_string())
}

fn svg(options: Option<String>) -> String {
    render_svg(TASKS, "[]", options)
}

fn error_of(options: Option<String>) -> serde_json::Value {
    let reported = render_svg_error(TASKS, "[]", options).expect("must be refused");
    serde_json::from_str(&reported).unwrap()
}

/// 三つの入口が、同じ入力を同じ理由で断る。
fn assert_refused(options: &str, code: &str, mentions: &str) {
    let v = error_of(s(options));
    assert_eq!(v["code"], code, "{options}: {v}");
    let message = v["error"].as_str().unwrap();
    assert!(message.contains(mentions), "{options}: {message}");

    assert_eq!(svg(s(options)), empty_svg(), "{options}");

    let canvas: serde_json::Value =
        serde_json::from_str(&render_canvas_commands(TASKS, "[]", s(options))).unwrap();
    assert_eq!(canvas["code"], code, "{options}");
    assert_eq!(canvas["error"], v["error"], "{options}");
}

// --- 既定 ---

#[test]
fn absent_empty_and_null_keys_are_the_defaults() {
    let plain = svg(None);
    assert_ne!(plain, empty_svg());
    for options in [
        "{}",
        " {} ",
        r#"{"today":null}"#,
        r#"{"scroll_viewport":null}"#,
        r#"{"tz_offset_minutes":null}"#,
        r#"{"today":null,"scroll_viewport":null,"tz_offset_minutes":null}"#,
    ] {
        assert_eq!(svg(s(options)), plain, "{options}");
        assert_eq!(
            render_canvas_commands(TASKS, "[]", s(options)),
            render_canvas_commands(TASKS, "[]", None),
            "{options}"
        );
        assert_eq!(render_svg_error(TASKS, "[]", s(options)), None, "{options}");
    }
}

#[test]
fn a_blank_today_draws_no_marker() {
    let plain = svg(None);
    assert_eq!(svg(s(r#"{"today":""}"#)), plain);
    assert_eq!(svg(s(r#"{"today":"   "}"#)), plain);
}

// --- 値が native の設定と同じ図になる ---

#[test]
fn every_key_reaches_the_native_options() {
    let tasks: Vec<GanttTask> = vec![
        GanttTask {
            id: "a".into(),
            title: "A".into(),
            progress_pct: 50,
            start: NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(),
            end: Some(NaiveDate::from_ymd_opt(2026, 6, 8).unwrap()),
        },
        GanttTask {
            id: "b".into(),
            title: "B".into(),
            progress_pct: 10,
            start: NaiveDate::from_ymd_opt(2026, 6, 2).unwrap(),
            end: Some(NaiveDate::from_ymd_opt(2026, 6, 5).unwrap()),
        },
    ];
    let deps: Vec<GanttDep> = vec![];
    let native = render(
        &tasks,
        &deps,
        &ChartOptions {
            today: Some(NaiveDate::from_ymd_opt(2026, 6, 4).unwrap()),
            scroll_viewport: Some(ScrollViewport {
                scroll_y: 0.0,
                client_height: 40.0,
            }),
        },
    );
    let wire = svg(s(
        r#"{"today":"2026-06-04","scroll_viewport":{"scroll_y":0,"client_height":40}}"#,
    ));
    assert_eq!(wire, native);
    assert_ne!(wire, svg(None));

    // 時差は、日時の字をどの暦の日と読むかを決める。
    let jst = svg(s(
        r#"{"today":"2026-06-03T15:30:00Z","tz_offset_minutes":540}"#,
    ));
    let day_four = svg(s(r#"{"today":"2026-06-04"}"#));
    assert_eq!(jst, day_four);
}

// --- 断る ---

#[test]
fn something_that_is_not_an_object_is_refused() {
    for options in [
        "[]",
        "42",
        "true",
        r#""2026-06-16""#,
        "2026-06-16",
        "null",
        "",
        " ",
        "{",
    ] {
        assert_refused(options, CODE_PARSE_ERROR, "options");
    }
}

#[test]
fn a_date_string_where_the_options_now_go_is_refused() {
    // 古い呼び方（第三引数が today の字）を、実行時に見つける。
    assert_refused("2026-06-16", CODE_PARSE_ERROR, "options");
    assert_refused(r#""2026-06-16""#, CODE_PARSE_ERROR, "options");
}

#[test]
fn an_unknown_key_is_refused_and_named() {
    assert_refused(r#"{"colour":"red"}"#, CODE_PARSE_ERROR, "colour");
    assert_refused(r#"{"lengend":"hidden"}"#, CODE_PARSE_ERROR, "lengend");
    // 正しい鍵と並んでいても、未知の鍵は断る。
    assert_refused(
        r#"{"today":"2026-06-04","todayy":"2026-06-04"}"#,
        CODE_PARSE_ERROR,
        "todayy",
    );
    // 古い名前（位置の引数の名前）は鍵ではない。
    assert_refused(
        r#"{"today_iso":"2026-06-04"}"#,
        CODE_PARSE_ERROR,
        "today_iso",
    );
    assert_refused(
        r#"{"viewport_json":"{}"}"#,
        CODE_PARSE_ERROR,
        "viewport_json",
    );
}

#[test]
fn a_value_of_the_wrong_shape_is_refused() {
    assert_refused(r#"{"today":20260604}"#, CODE_PARSE_ERROR, "today");
    assert_refused(r#"{"today":"not-a-date"}"#, CODE_PARSE_ERROR, "today");
    assert_refused(
        r#"{"tz_offset_minutes":"540"}"#,
        CODE_PARSE_ERROR,
        "tz_offset_minutes",
    );
    assert_refused(
        r#"{"tz_offset_minutes":30.5}"#,
        CODE_PARSE_ERROR,
        "tz_offset_minutes",
    );
    assert_refused(
        r#"{"tz_offset_minutes":1440}"#,
        CODE_PARSE_ERROR,
        "tz_offset_minutes",
    );
    assert_refused(
        r#"{"scroll_viewport":"x"}"#,
        CODE_PARSE_ERROR,
        "scroll_viewport",
    );
    assert_refused(
        r#"{"scroll_viewport":{}}"#,
        CODE_PARSE_ERROR,
        "scroll_viewport",
    );
    assert_refused(
        r#"{"scroll_viewport":{"scroll_y":0}}"#,
        CODE_PARSE_ERROR,
        "scroll_viewport",
    );
    assert_refused(
        r#"{"scroll_viewport":{"scroll_y":"0","client_height":10}}"#,
        CODE_PARSE_ERROR,
        "scroll_viewport",
    );
}

#[test]
fn an_unreadable_scroll_viewport_is_refused_not_ignored() {
    // 以前は黙って無視して、窓の無い図を描いた。これだけが、出力を意図して変えた入力である。
    assert_refused(
        r#"{"scroll_viewport":[1,2]}"#,
        CODE_PARSE_ERROR,
        "scroll_viewport",
    );
}

#[test]
fn options_over_the_limit_are_refused_with_the_input_limit_code() {
    let at_limit = format!("{{}}{}", " ".repeat(4096 - 2));
    assert_eq!(at_limit.len(), 4096);
    assert_eq!(svg(Some(at_limit)), svg(None));

    let over = format!("{{}}{}", " ".repeat(4096 - 1));
    assert_eq!(over.len(), 4097);
    assert_refused(&over, CODE_INPUT_LIMIT, "options");
}

#[test]
fn the_options_are_judged_before_the_tasks() {
    // 設定が読めない時は、タスクの誤りより先にそれを言う。
    let reported = render_svg_error("not json", "[]", s(r#"{"colour":1}"#)).unwrap();
    let v: serde_json::Value = serde_json::from_str(&reported).unwrap();
    assert!(v["error"].as_str().unwrap().contains("colour"), "{v}");
    // 設定が読めて、タスクが読めない時は、タスクの誤りを言う。
    let reported = render_svg_error("not json", "[]", s("{}")).unwrap();
    let v: serde_json::Value = serde_json::from_str(&reported).unwrap();
    assert!(!v["error"].as_str().unwrap().contains("options"), "{v}");
}
