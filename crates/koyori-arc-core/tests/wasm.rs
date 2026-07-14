use wasm_bindgen_test::wasm_bindgen_test;

wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_node_experimental);

const TASKS: &str = r#"[
  {"id":"t1","title":"Design","progress_pct":100,"start":"2026-06-01","end":"2026-06-04"},
  {"id":"t2","title":"Build","progress_pct":50,"start":"2026-06-04","end":"2026-06-08"}
]"#;

const DEPS: &str = r#"[{"blocker_task_id":"t1","blocked_task_id":"t2"}]"#;

#[wasm_bindgen_test]
fn render_svg_returns_svg() {
    let svg = koyori_arc_core::render_svg(TASKS, DEPS, None, None);
    assert!(svg.starts_with("<svg "));
    assert!(svg.ends_with("</svg>"));
}

#[wasm_bindgen_test]
fn render_svg_contains_titles() {
    let svg = koyori_arc_core::render_svg(TASKS, DEPS, None, None);
    assert!(svg.contains("Design"));
    assert!(svg.contains("Build"));
}

#[wasm_bindgen_test]
fn render_svg_with_today_marker() {
    let svg = koyori_arc_core::render_svg(TASKS, DEPS, Some("2026-06-03".to_string()), None);
    // COLOR_TODAY = "#f59e0b"
    assert!(svg.contains("f59e0b"));
}

#[wasm_bindgen_test]
fn render_svg_parse_error_returns_safe_empty_svg() {
    let svg = koyori_arc_core::render_svg("not json", "[]", None, None);
    assert!(svg.contains("width=\"0\""));
    assert!(!svg.contains("<!--"));
    assert!(!svg.contains("parse error"));
}

#[wasm_bindgen_test]
fn render_svg_id_xss_payload_escaped() {
    let tasks = r#"[{"id":"x\" onmouseover=\"alert(1)","title":"Safe","progress_pct":0,"start":"2026-06-01","end":"2026-06-02"}]"#;
    let svg = koyori_arc_core::render_svg(tasks, "[]", None, None);
    assert!(svg.contains("&quot;"));
    assert!(!svg.contains(r#"onmouseover="alert"#));
}

#[wasm_bindgen_test]
fn render_svg_empty_tasks() {
    let svg = koyori_arc_core::render_svg("[]", "[]", None, None);
    assert!(svg.contains("width=\"0\""));
    assert!(svg.contains("viewBox="));
}

#[wasm_bindgen_test]
fn render_svg_has_a11y_and_progress_labels() {
    let svg = koyori_arc_core::render_svg(TASKS, DEPS, None, None);
    assert!(svg.contains(r#"role="img""#));
    assert!(svg.contains("viewBox="));
    assert!(svg.contains("100%"));
    assert!(svg.contains("50%"));
    assert!(svg.contains("progress-line-legend"));
    assert!(svg.contains("bar-tier-legend"));
}

#[wasm_bindgen_test]
fn render_svg_progress_line_today_anchored() {
    let svg = koyori_arc_core::render_svg(TASKS, DEPS, Some("2026-06-03".to_string()), None);
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
    let svg = koyori_arc_core::render_svg(tasks, "[]", None, None);
    assert!(svg.contains("<polygon"));
    assert!(svg.contains("100%"));
    assert!(svg.contains("<title>Ship: 2026-06-03 – 2026-06-03 (100%)</title>"));
}

#[wasm_bindgen_test]
fn render_canvas_commands_returns_json_buffer() {
    let json = koyori_arc_core::render_canvas_commands(TASKS, DEPS, None, None);
    let v: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    assert!(v.get("error").is_none());
    assert_eq!(v["viewport_width"].as_f64().unwrap(), 350.0);
    assert!(v["ops"].as_array().unwrap().len() > 10);
}

#[wasm_bindgen_test]
fn render_canvas_commands_progress_line_present() {
    let json = koyori_arc_core::render_canvas_commands(TASKS, DEPS, None, None);
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
    let json =
        koyori_arc_core::render_canvas_commands(TASKS, DEPS, Some("2026-06-03".to_string()), None);
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
    let json = koyori_arc_core::render_canvas_commands("not json", "[]", None, None);
    let v: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    assert!(v.get("error").is_some());
}

#[wasm_bindgen_test]
fn render_canvas_commands_empty_tasks() {
    let json = koyori_arc_core::render_canvas_commands("[]", "[]", None, None);
    let v: serde_json::Value = serde_json::from_str(&json).expect("valid json");
    assert_eq!(v["viewport_width"].as_f64().unwrap(), 0.0);
    assert!(v["ops"].as_array().unwrap().is_empty());
}
