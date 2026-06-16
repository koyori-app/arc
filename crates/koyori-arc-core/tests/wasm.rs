use wasm_bindgen_test::wasm_bindgen_test;

wasm_bindgen_test::wasm_bindgen_test_configure!(run_in_node_experimental);

const TASKS: &str = r#"[
  {"id":"t1","title":"Design","progress_pct":100,"start":"2026-06-01","end":"2026-06-04"},
  {"id":"t2","title":"Build","progress_pct":50,"start":"2026-06-04","end":"2026-06-08"}
]"#;

const DEPS: &str = r#"[{"blocker_task_id":"t1","blocked_task_id":"t2"}]"#;

#[wasm_bindgen_test]
fn render_svg_returns_svg() {
    let svg = koyori_arc_core::render_svg(TASKS, DEPS, None);
    assert!(svg.starts_with("<svg "));
    assert!(svg.ends_with("</svg>"));
}

#[wasm_bindgen_test]
fn render_svg_contains_titles() {
    let svg = koyori_arc_core::render_svg(TASKS, DEPS, None);
    assert!(svg.contains("Design"));
    assert!(svg.contains("Build"));
}

#[wasm_bindgen_test]
fn render_svg_with_today_marker() {
    let svg = koyori_arc_core::render_svg(TASKS, DEPS, Some("2026-06-03".to_string()));
    // COLOR_TODAY = "#f59e0b"
    assert!(svg.contains("f59e0b"));
}

#[wasm_bindgen_test]
fn render_svg_parse_error_returns_comment() {
    let svg = koyori_arc_core::render_svg("not json", "[]", None);
    assert!(svg.starts_with("<!-- parse error:"));
}

#[wasm_bindgen_test]
fn render_svg_empty_tasks() {
    let svg = koyori_arc_core::render_svg("[]", "[]", None);
    assert!(svg.contains("width=\"0\""));
}
