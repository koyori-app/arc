mod common;

use wasm_bindgen_test::wasm_bindgen_test;

wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_node_experimental);

const TASKS: &str = r#"[
  {"id":"t1","title":"Design","progress_pct":100,"start":"2026-06-01","end":"2026-06-04"},
  {"id":"t2","title":"Build","progress_pct":50,"start":"2026-06-04","end":"2026-06-08"}
]"#;

const DEPS: &str = r#"[{"blocker_task_id":"t1","blocked_task_id":"t2"}]"#;

#[wasm_bindgen_test]
fn render_svg_returns_svg() {
    let svg = koyori_arc_core::render_svg(TASKS, DEPS, common::options_json(None, None, None));
    assert!(svg.starts_with("<svg "));
    assert!(svg.ends_with("</svg>"));
}

#[wasm_bindgen_test]
fn render_svg_contains_titles() {
    let svg = koyori_arc_core::render_svg(TASKS, DEPS, common::options_json(None, None, None));
    assert!(svg.contains("Design"));
    assert!(svg.contains("Build"));
}

#[wasm_bindgen_test]
fn render_svg_with_today_marker() {
    let svg = koyori_arc_core::render_svg(
        TASKS,
        DEPS,
        common::options_json(Some("2026-06-03".to_string()), None, None),
    );
    // Palette::standard() の ColorId::Today（#f59e0b）。palette を変えたらここも落ちる。
    assert!(svg.contains("f59e0b"));
}

#[wasm_bindgen_test]
fn render_svg_parse_error_returns_safe_empty_svg() {
    let svg = koyori_arc_core::render_svg("not json", "[]", common::options_json(None, None, None));
    assert!(svg.contains("width=\"0\""));
    assert!(!svg.contains("<!--"));
    assert!(!svg.contains("parse error"));
}

#[wasm_bindgen_test]
fn render_svg_id_xss_payload_escaped() {
    let tasks = r#"[{"id":"x\" onmouseover=\"alert(1)","title":"Safe","progress_pct":0,"start":"2026-06-01","end":"2026-06-02"}]"#;
    let svg = koyori_arc_core::render_svg(tasks, "[]", common::options_json(None, None, None));
    assert!(svg.contains("&quot;"));
    assert!(!svg.contains(r#"onmouseover="alert"#));
}

#[wasm_bindgen_test]
fn render_svg_empty_tasks() {
    let svg = koyori_arc_core::render_svg("[]", "[]", common::options_json(None, None, None));
    assert!(svg.contains("width=\"0\""));
    assert!(svg.contains("viewBox="));
}

#[wasm_bindgen_test]
fn render_svg_has_a11y_and_progress_labels() {
    let svg = koyori_arc_core::render_svg(TASKS, DEPS, common::options_json(None, None, None));
    assert!(svg.contains(r#"role="img""#));
    assert!(svg.contains("viewBox="));
    assert!(svg.contains("100%"));
    assert!(svg.contains("50%"));
    assert!(svg.contains("progress-line-legend"));
    assert!(svg.contains("bar-tier-legend"));
}

#[wasm_bindgen_test]
fn render_svg_progress_line_today_anchored() {
    let svg = koyori_arc_core::render_svg(
        TASKS,
        DEPS,
        common::options_json(Some("2026-06-03".to_string()), None, None),
    );
    let poly = svg
        .split(r#"class="progress-status-line" points=""#)
        .nth(1)
        .and_then(|s| s.split('"').next())
        .expect("progress polyline");
    assert!(poly.starts_with("180,"));
    let last_pt = poly.split(' ').next_back().expect("last point");
    assert!(last_pt.starts_with("180,"));
    assert!(svg.contains("bar-tier-done"));
}

#[wasm_bindgen_test]
fn render_svg_milestone_and_tooltip() {
    let tasks = r#"[{"id":"ms","title":"Ship","progress_pct":100,"start":"2026-06-03","end":"2026-06-03"}]"#;
    let svg = koyori_arc_core::render_svg(tasks, "[]", common::options_json(None, None, None));
    assert!(svg.contains("<polygon"));
    assert!(svg.contains("100%"));
    assert!(svg.contains("<title>Ship: 2026-06-03 – 2026-06-03 (100%)</title>"));
}

#[wasm_bindgen_test]
fn render_canvas_commands_returns_json_buffer() {
    let json = koyori_arc_core::render_canvas_commands(
        TASKS,
        DEPS,
        common::options_json(None, None, None),
    );
    let v: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    assert!(v.get("error").is_none());
    assert_eq!(v["viewport_width"].as_f64().unwrap(), 422.0);
    assert!(v["ops"].as_array().unwrap().len() > 10);
}

#[wasm_bindgen_test]
fn render_canvas_commands_progress_line_present() {
    let json = koyori_arc_core::render_canvas_commands(
        TASKS,
        DEPS,
        common::options_json(None, None, None),
    );
    let v: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    let ops = v["ops"].as_array().expect("ops array");
    let has_progress_poly = ops.iter().any(|op| {
        op.get("StrokePolyline")
            .and_then(|p| p.get("color_id"))
            .and_then(|c| c.as_u64())
            == Some(6)
    });
    assert!(has_progress_poly, "progress line StrokePolyline expected");
}

#[wasm_bindgen_test]
fn render_canvas_commands_today_anchored_progress_line() {
    let json = koyori_arc_core::render_canvas_commands(
        TASKS,
        DEPS,
        common::options_json(Some("2026-06-03".to_string()), None, None),
    );
    let v: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    let ops = v["ops"].as_array().expect("ops array");
    let progress = ops.iter().find_map(|op| {
        op.get("StrokePolyline")
            .filter(|p| p.get("color_id").and_then(|c| c.as_u64()) == Some(6))
    });
    let points = progress
        .and_then(|p| p.get("points"))
        .and_then(|p| p.as_array())
        .expect("progress polyline points");
    let first = points[0].as_array().expect("point");
    assert_eq!(first[0].as_f64().unwrap(), 180.0);
    let last = points.last().unwrap().as_array().unwrap();
    assert_eq!(last[0].as_f64().unwrap(), 180.0);
}

#[wasm_bindgen_test]
fn render_canvas_commands_parse_error_json() {
    let json = koyori_arc_core::render_canvas_commands(
        "not json",
        "[]",
        common::options_json(None, None, None),
    );
    let v: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    assert!(v.get("error").is_some());
}

#[wasm_bindgen_test]
fn render_svg_error_separates_nothing_to_draw_from_refused_input() {
    assert_eq!(
        koyori_arc_core::render_svg_error("[]", "[]", common::options_json(None, None, None)),
        None
    );
    assert_eq!(
        koyori_arc_core::render_svg_error(TASKS, DEPS, common::options_json(None, None, None)),
        None
    );

    let refused =
        r#"[{"id":"t","title":"T","progress_pct":0,"start":"2026-06-01","end":"2126-06-01"}]"#;
    let reported =
        koyori_arc_core::render_svg_error(refused, "[]", common::options_json(None, None, None))
            .expect("refusal reported");
    let v: serde_json::Value = serde_json::from_str(&reported).expect("valid json");
    assert_eq!(v["code"].as_str(), Some("input_limit"));
    // The SVG entry point itself still answers with the empty chart.
    assert_eq!(
        koyori_arc_core::render_svg(refused, "[]", common::options_json(None, None, None)),
        koyori_arc_core::empty_svg_markup()
    );
}

#[wasm_bindgen_test]
fn empty_svg_markup_matches_what_refusals_return() {
    assert_eq!(
        koyori_arc_core::render_svg("not json", "[]", common::options_json(None, None, None)),
        koyori_arc_core::empty_svg_markup()
    );
}

#[wasm_bindgen_test]
fn render_canvas_commands_empty_tasks() {
    let json =
        koyori_arc_core::render_canvas_commands("[]", "[]", common::options_json(None, None, None));
    let v: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    assert_eq!(v["viewport_width"].as_f64().unwrap(), 0.0);
    assert!(v["ops"].as_array().unwrap().is_empty());
}

#[wasm_bindgen_test]
fn datetime_strings_cross_the_wasm_boundary() {
    let datetimes = r#"[
      {"id":"t1","title":"Design","progress_pct":100,"start":"2026-06-01T09:00:00Z","end":"2026-06-04T23:30:00+09:00"},
      {"id":"t2","title":"Build","progress_pct":50,"start":"2026-06-04T00:00:00.000Z","end":"2026-06-08T00:00:00Z"}
    ]"#;
    let from_dates = koyori_arc_core::render_svg(
        TASKS,
        DEPS,
        common::options_json(Some("2026-06-03".to_string()), None, None),
    );
    let from_datetimes = koyori_arc_core::render_svg(
        datetimes,
        DEPS,
        common::options_json(Some("2026-06-03T12:00:00+09:00".to_string()), None, None),
    );
    assert_eq!(from_dates, from_datetimes);
}

#[wasm_bindgen_test]
fn an_unreadable_today_is_reported_not_dropped() {
    let bad = Some("garbage".to_string());
    assert_eq!(
        koyori_arc_core::render_svg(TASKS, DEPS, common::options_json(bad.clone(), None, None)),
        koyori_arc_core::empty_svg_markup()
    );
    let reported = koyori_arc_core::render_svg_error(
        TASKS,
        DEPS,
        common::options_json(bad.clone(), None, None),
    )
    .expect("an unreadable today is reported");
    let v: serde_json::Value = serde_json::from_str(&reported).expect("valid json");
    assert_eq!(v["code"].as_str(), Some("parse_error"));
    let canvas =
        koyori_arc_core::render_canvas_commands(TASKS, DEPS, common::options_json(bad, None, None));
    let c: serde_json::Value = serde_json::from_str(&canvas).expect("valid json");
    assert_eq!(c["code"].as_str(), Some("parse_error"));
}

#[wasm_bindgen_test]
fn tz_offset_minutes_crosses_the_wasm_boundary() {
    let tasks = r#"[{"id":"t","title":"T","progress_pct":50,"start":"2026-06-01","end":"2026-06-08T15:30:00Z"}]"#;
    let jst =
        koyori_arc_core::render_svg(tasks, "[]", common::options_json(None, None, Some(540.0)));
    let utc = koyori_arc_core::render_svg(tasks, "[]", common::options_json(None, None, Some(0.0)));
    assert!(jst.contains(" – 2026-06-09 ("));
    assert!(utc.contains(" – 2026-06-08 ("));
    // NaN は i32 で受けると黙って 0 になる。f64 で受けて断る。
    let reported = koyori_arc_core::render_svg_error(
        tasks,
        "[]",
        common::options_json(None, None, Some(f64::NAN)),
    )
    .expect("an unusable offset is reported");
    assert!(reported.contains("tz_offset_minutes"));
}

#[wasm_bindgen_test]
fn legend_spec_json_crosses_the_wasm_boundary() {
    let json = koyori_arc_core::legend_spec_json();
    assert!(json.contains(r#""key":"done""#), "{json}");
    assert!(json.contains(r#""dash":"6,3""#), "{json}");
}

#[wasm_bindgen_test]
fn the_legend_can_be_hidden_through_options_json() {
    let shown = koyori_arc_core::render_svg(TASKS, DEPS, None);
    let hidden =
        koyori_arc_core::render_svg(TASKS, DEPS, Some(r#"{"legend":"hidden"}"#.to_string()));
    assert!(shown.contains("bar-tier-legend"));
    assert!(!hidden.contains("bar-tier-legend"));
}
