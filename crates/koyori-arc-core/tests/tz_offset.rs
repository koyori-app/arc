//! 時差の指定（`tz_offset_minutes`）。日時の字を、利用者の暦の日へ直す。
//!
//! 日付は、タスクの `<title>`（`名前: 開始 – 終了 (進捗)`）に書かれる。
//! 図の位置を見るより、何の日に直ったかが一目で分かる。

use koyori_arc_core::{
    empty_svg, render_canvas_commands, render_svg, render_svg_error, CODE_PARSE_ERROR,
};

fn one_task(start: &str, end: &str) -> String {
    format!(r#"[{{"id":"t","title":"T","progress_pct":50,"start":"{start}","end":"{end}"}}]"#)
}

fn svg(tasks: &str, today: Option<&str>, tz: Option<f64>) -> String {
    render_svg(tasks, "[]", today.map(str::to_string), None, tz)
}

// --- 陽性対照 ---

#[test]
fn a_utc_end_is_the_next_day_in_jst_and_the_same_day_in_utc() {
    let tasks = one_task("2026-06-01", "2026-06-08T15:30:00Z");
    assert!(svg(&tasks, None, Some(540.0)).contains(" – 2026-06-09 ("));
    assert!(svg(&tasks, None, Some(0.0)).contains(" – 2026-06-08 ("));
}

#[test]
fn a_utc_start_and_today_follow_the_same_rule() {
    let tasks = one_task("2026-06-08T15:30:00Z", "2026-06-20");
    assert!(svg(&tasks, None, Some(540.0)).contains("T: 2026-06-09 – "));
    assert!(svg(&tasks, None, Some(0.0)).contains("T: 2026-06-08 – "));

    // today は、日付だけの字で書いた同じ日と同じ図になる。
    let dates = one_task("2026-06-01", "2026-06-20");
    assert_eq!(
        svg(&dates, Some("2026-06-08T15:30:00Z"), Some(540.0)),
        svg(&dates, Some("2026-06-09"), None)
    );
    assert_eq!(
        svg(&dates, Some("2026-06-08T15:30:00Z"), Some(0.0)),
        svg(&dates, Some("2026-06-08"), None)
    );
}

// --- 日付だけの字は換算しない ---

#[test]
fn a_date_only_string_is_the_same_day_whatever_the_offset() {
    let tasks = one_task("2026-06-01", "2026-06-08");
    let plain = svg(&tasks, Some("2026-06-04"), None);
    for tz in [-720.0, -300.0, 0.0, 540.0, 840.0] {
        assert_eq!(svg(&tasks, Some("2026-06-04"), Some(tz)), plain, "tz={tz}");
        assert!(
            svg(&tasks, None, Some(tz)).contains(" – 2026-06-08 ("),
            "tz={tz}"
        );
    }
}

#[test]
fn a_datetime_without_an_offset_is_the_same_day_whatever_the_offset() {
    let tasks = one_task("2026-06-01", "2026-06-08T23:30:00");
    for tz in [-720.0, 0.0, 540.0, 840.0] {
        assert!(
            svg(&tasks, None, Some(tz)).contains(" – 2026-06-08 ("),
            "tz={tz}"
        );
    }
}

// --- 日をまたぐ際どい字 ---

#[test]
fn strings_near_midnight() {
    // 日本の 23:30。JST でも UTC でも 8 日。
    let a = one_task("2026-06-01", "2026-06-08T23:30:00+09:00");
    assert!(svg(&a, None, Some(540.0)).contains(" – 2026-06-08 ("));
    assert!(svg(&a, None, Some(0.0)).contains(" – 2026-06-08 ("));
    // 00:30 UTC は、JST では 9 日、アメリカ東部（-300）では 8 日。
    let b = one_task("2026-06-01", "2026-06-09T00:30:00Z");
    assert!(svg(&b, None, Some(540.0)).contains(" – 2026-06-09 ("));
    assert!(svg(&b, None, Some(-300.0)).contains(" – 2026-06-08 ("));
}

#[test]
fn no_offset_is_not_the_same_as_zero_for_a_string_with_an_offset() {
    // JST の 9 日 00:30（UTC では 8 日 15:30）。指定なしは書かれた 9 日、0 は UTC の 8 日。
    let tasks = one_task("2026-06-01", "2026-06-09T00:30:00+09:00");
    assert!(svg(&tasks, None, None).contains(" – 2026-06-09 ("));
    assert!(svg(&tasks, None, Some(0.0)).contains(" – 2026-06-08 ("));
    assert!(svg(&tasks, None, Some(540.0)).contains(" – 2026-06-09 ("));
}

// --- 既定は今までどおり ---

#[test]
fn absent_offset_draws_what_it_drew_before_the_option_existed() {
    // 日付だけの入力は、指定なしでも 0 でも 540 でも 1 バイトも変わらない。
    let tasks = r#"[
      {"id":"a","title":"A","progress_pct":50,"start":"2026-06-01","end":"2026-06-05"},
      {"id":"b","title":"B","progress_pct":20,"start":"2026-06-03"}
    ]"#;
    let base = render_svg(tasks, "[]", Some("2026-06-03".to_string()), None, None);
    for tz in [0.0, 540.0, -300.0] {
        assert_eq!(
            render_svg(tasks, "[]", Some("2026-06-03".to_string()), None, Some(tz)),
            base
        );
    }
    let canvas = render_canvas_commands(tasks, "[]", Some("2026-06-03".to_string()), None, None);
    assert_eq!(
        render_canvas_commands(
            tasks,
            "[]",
            Some("2026-06-03".to_string()),
            None,
            Some(540.0)
        ),
        canvas
    );
}

// --- 正気でない値 ---

#[test]
fn an_absurd_offset_is_refused_by_all_three_entries() {
    let tasks = one_task("2026-06-01", "2026-06-08");
    for bad in [1440.0, -1440.0, 100000.0, 30.5, f64::NAN, f64::INFINITY] {
        let reported = render_svg_error(&tasks, "[]", None, Some(bad))
            .unwrap_or_else(|| panic!("{bad} must be reported"));
        let v: serde_json::Value = serde_json::from_str(&reported).unwrap();
        assert_eq!(v["code"], CODE_PARSE_ERROR, "{bad}");
        assert!(
            v["error"].as_str().unwrap().contains("tz_offset_minutes"),
            "{reported}"
        );
        assert_eq!(svg(&tasks, None, Some(bad)), empty_svg(), "{bad}");
        let canvas = render_canvas_commands(&tasks, "[]", None, None, Some(bad));
        let c: serde_json::Value = serde_json::from_str(&canvas).unwrap();
        assert_eq!(c["code"], CODE_PARSE_ERROR, "{bad}");
    }
}

#[test]
fn real_world_offsets_are_accepted() {
    let tasks = one_task("2026-06-01", "2026-06-08");
    for ok in [-720.0, -300.0, 0.0, 330.0, 345.0, 540.0, 840.0] {
        assert_eq!(render_svg_error(&tasks, "[]", None, Some(ok)), None, "{ok}");
    }
}

#[test]
fn a_bad_date_names_the_task_and_the_field() {
    let tasks = r#"[
      {"id":"a","title":"A","progress_pct":0,"start":"2026-06-01"},
      {"id":"b","title":"B","progress_pct":0,"start":"2026-06-01","end":"soon"}
    ]"#;
    let reported = render_svg_error(tasks, "[]", None, Some(540.0)).expect("refused");
    let v: serde_json::Value = serde_json::from_str(&reported).unwrap();
    assert!(
        v["error"].as_str().unwrap().contains("tasks[1].end"),
        "{reported}"
    );
}
