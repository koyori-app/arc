//! Output-encoding regression tests — what the renderer emits for hostile input.
//!
//! The unit test `render.rs::xml_special_chars_escaped` only inspects `title`,
//! which is a *text node*. A three-character escape (`&` `<` `>`) is sufficient
//! there, so that test passes even while an attribute value is wide open: the
//! attribute-delimiting `"` is not part of its alphabet. It therefore cannot,
//! in principle, detect an injection through `data-task-id`.
//!
//! These tests look at attribute values instead, and do so through both public
//! entry points: `render()` (native, typed structs) and `render_svg()` (wasm,
//! JSON strings — the one `arc-vue` actually calls).

use koyori_arc_core::{render, render_canvas_commands, render_svg, GanttTask};

/// `task.id` is a plain `String` with no validation at either entry point.
/// Closing the attribute and appending an event handler is all an XSS needs,
/// because `GanttChart.vue` mounts the output with `v-html` (= innerHTML).
const HOSTILE_ID: &str = r#"x" onmouseover="alert(1)"#;

fn hostile_task() -> GanttTask {
    GanttTask {
        id: HOSTILE_ID.to_string(),
        title: "Design".to_string(),
        progress_pct: 50,
        start: chrono::NaiveDate::from_ymd_opt(2026, 6, 1).unwrap(),
        end: Some(chrono::NaiveDate::from_ymd_opt(2026, 6, 4).unwrap()),
    }
}

fn hostile_task_json() -> String {
    format!(
        r#"[{{"id":{id},"title":"Design","progress_pct":50,"start":"2026-06-01","end":"2026-06-04"}}]"#,
        id = serde_json::to_string(HOSTILE_ID).unwrap(),
    )
}

/// Every attribute name the renderer is allowed to emit.
///
/// This is the structural half of the defence: instead of asserting "this one
/// known payload does not appear", it asserts "no attribute the renderer never
/// writes appears". A future author who adds an attribute fed by user input and
/// forgets `escape_attr` will surface here as an unexpected attribute name, even
/// though the payload in this file was never written with their code in mind.
const ALLOWED_ATTRS: &[&str] = &[
    "xmlns",
    "width",
    "height",
    "viewBox",
    "role",
    "aria-label",
    "aria-hidden",
    "font-family",
    "font-size",
    "font-weight",
    "class",
    "x",
    "y",
    "rx",
    "x1",
    "y1",
    "x2",
    "y2",
    "d",
    "points",
    "fill",
    "stroke",
    "stroke-width",
    "stroke-dasharray",
    "stroke-linecap",
    "stroke-linejoin",
    "text-anchor",
    "dominant-baseline",
    "data-task-id",
];

/// Collect every `name="value"` attribute name in the markup, the way a browser
/// would: an attribute value ends at the first raw `"`, so an unescaped quote in
/// a value makes whatever follows a new attribute.
fn attribute_names(markup: &str) -> Vec<String> {
    let bytes: Vec<char> = markup.chars().collect();
    let mut names = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != '<' {
            i += 1;
            continue;
        }
        // Inside a tag: read `name` / `name="value"` pairs until `>`.
        i += 1;
        // Skip the tag name (and closing-tag slash).
        while i < bytes.len() && !bytes[i].is_whitespace() && bytes[i] != '>' {
            i += 1;
        }
        while i < bytes.len() && bytes[i] != '>' {
            while i < bytes.len() && bytes[i].is_whitespace() {
                i += 1;
            }
            let start = i;
            while i < bytes.len() && bytes[i] != '=' && bytes[i] != '>' && !bytes[i].is_whitespace()
            {
                i += 1;
            }
            if i > start {
                let name: String = bytes[start..i].iter().collect();
                let name = name.trim_end_matches('/').to_string();
                if !name.is_empty() {
                    names.push(name);
                }
            }
            if i < bytes.len() && bytes[i] == '=' {
                i += 1;
                if i < bytes.len() && bytes[i] == '"' {
                    i += 1;
                    while i < bytes.len() && bytes[i] != '"' {
                        i += 1;
                    }
                    i += 1; // closing quote
                }
            } else if i < bytes.len() && bytes[i] != '>' && !bytes[i].is_whitespace() {
                i += 1;
            }
        }
    }
    names
}

fn assert_no_foreign_attributes(markup: &str, label: &str) {
    let unexpected: Vec<String> = attribute_names(markup)
        .into_iter()
        .filter(|n| !ALLOWED_ATTRS.contains(&n.as_str()))
        .collect();
    assert!(
        unexpected.is_empty(),
        "{label}: attributes the renderer never writes appeared, so a value escaped its quotes: {unexpected:?}"
    );
}

#[test]
fn positive_control_text_nodes_still_escape() {
    // Proves the measurement is alive: the three-character escape that already
    // existed is still in force for text nodes.
    let mut task = hostile_task();
    task.title = "A & B < C > D".to_string();
    let svg = render(&[task], &[], None, None);
    assert!(svg.contains("A &amp; B &lt; C &gt; D"), "got: {svg}");
}

#[test]
fn native_entry_task_id_cannot_close_the_attribute() {
    let svg = render(&[hostile_task()], &[], None, None);
    assert!(
        !svg.contains(r#"onmouseover=""#),
        "live onmouseover attribute injected via native render(): {svg}"
    );
    assert!(
        svg.contains("&quot;"),
        "the double quote in task.id must be encoded: {svg}"
    );
    assert_no_foreign_attributes(&svg, "native render()");
}

#[test]
fn wasm_entry_task_id_cannot_close_the_attribute() {
    // `render_svg` is the entry `arc-vue` uses; the native path above does not
    // cover it, and the JSON hop is where an external id realistically arrives.
    let svg = render_svg(&hostile_task_json(), "[]", None, None);
    assert!(
        !svg.contains(r#"onmouseover=""#),
        "live onmouseover attribute injected via render_svg(): {svg}"
    );
    assert_no_foreign_attributes(&svg, "render_svg()");
}

#[test]
fn single_quote_in_task_id_is_encoded() {
    let mut task = hostile_task();
    task.id = "o'brien".to_string();
    let svg = render(&[task], &[], None, None);
    assert!(
        svg.contains("data-task-id=\"o&#39;brien\""),
        "single quotes must be encoded so single-quoted hosts stay safe: {svg}"
    );
}

#[test]
fn parse_error_cannot_break_out_of_the_svg_comment() {
    // serde echoes the offending value verbatim ("invalid type: string \"...\""),
    // so an input carrying `-->` used to terminate the comment and leave live
    // markup behind under `v-html`.
    let hostile = r#"[{"id":"a","title":"t","progress_pct":"--><img src=x onerror=alert(1)>","start":"2026-06-01","end":"2026-06-02"}]"#;
    let out = render_svg(hostile, "[]", None, None);
    assert!(out.starts_with("<!--"), "got: {out}");
    assert!(out.ends_with("-->"), "got: {out}");
    assert_eq!(
        out.matches("-->").count(),
        1,
        "comment terminated early, markup after it is live: {out}"
    );
    assert!(
        !out.contains("<img"),
        "attacker markup reached output: {out}"
    );
}

#[test]
fn canvas_command_parse_error_is_valid_json() {
    // The doc comment on `render_canvas_commands` promises valid JSON here, and
    // `parseCommandBuffer` on the JS side relies on it: a broken payload throws
    // inside a floating promise and the user gets a blank chart with no error.
    let hostile = r#"[{"id":"a","title":"t","progress_pct":"\"}]<img src=x onerror=alert(1)>","start":"2026-06-01","end":"2026-06-02"}]"#;
    let out = render_canvas_commands(hostile, "[]", None, None);
    let parsed: serde_json::Value = serde_json::from_str(&out)
        .unwrap_or_else(|e| panic!("error payload is not valid JSON ({e}): {out}"));
    assert!(
        parsed.get("error").and_then(|v| v.as_str()).is_some(),
        "expected an `error` string the JS side can branch on: {out}"
    );
}

#[test]
fn canvas_command_parse_error_on_deps_is_valid_json() {
    let ok_tasks =
        r#"[{"id":"a","title":"t","progress_pct":10,"start":"2026-06-01","end":"2026-06-02"}]"#;
    let hostile_deps = r#"[{"blocker_task_id":{"x":"\"}]"},"blocked_task_id":"b"}]"#;
    let out = render_canvas_commands(ok_tasks, hostile_deps, None, None);
    let parsed: serde_json::Value = serde_json::from_str(&out)
        .unwrap_or_else(|e| panic!("error payload is not valid JSON ({e}): {out}"));
    assert!(parsed.get("error").is_some(), "got: {out}");
}
