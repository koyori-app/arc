//! Output-encoding regression tests — what the renderer emits for hostile input.
//!
//! The unit tests in `src/` look at this from the inside: `escape_xml` is called
//! directly, or `render()` is handed typed structs. Two things that are true of
//! the shipped product are therefore invisible to them.
//!
//! 1. `arc-vue` does not call `render()`. It calls `render_svg()` — the JSON
//!    entry point — and mounts the result with `v-html` (= innerHTML). The hop
//!    through JSON is where an externally supplied `task.id` realistically
//!    arrives, and no test crosses it.
//! 2. Asserting that one known payload is neutralised says nothing about the
//!    next attribute someone feeds with user input. The structural assertion
//!    below (`assert_no_foreign_attributes`) is written so it fires for code
//!    that did not exist when this file was written.
//!
//! The single-quote case is here because it was measured, not guessed: deleting
//! `.replace('\'', "&#39;")` from `escape_xml` leaves the whole existing suite
//! green (106 passed), and fails `single_quote_in_task_id_is_encoded` here.

use koyori_arc_core::GanttTask;
use koyori_arc_core::{empty_svg, render, render_canvas_commands, render_svg, render_svg_error};

/// `task.id` is a plain `String` with no validation at either entry point.
/// Closing the attribute and appending an event handler is all an XSS needs.
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
/// forgets to escape it surfaces here as an unexpected attribute name, even
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

/// Positive control for the structural scan itself: markup that really does
/// carry an attribute outside the allowlist must be reported. Without this, a
/// scanner that silently found nothing would read exactly like a safe renderer.
#[test]
fn structural_scan_reports_an_attribute_outside_the_allowlist() {
    let names = attribute_names(r#"<g data-task-id="x" onmouseover="alert(1)"></g>"#);
    assert!(
        names.iter().any(|n| n == "onmouseover"),
        "the scan must see an injected attribute, else its silence proves nothing: {names:?}"
    );
}

#[test]
fn positive_control_text_nodes_still_escape() {
    // Proves the measurement is alive: the escaping that already existed is
    // still in force for text nodes.
    let mut task = hostile_task();
    task.title = "A & B < C > D".to_string();
    let svg = render(&[task], &[], None, None);
    assert!(svg.contains("A &amp; B &lt; C &gt; D"), "got: {svg}");
}

/// The known-payload half of this is already covered by
/// `render::tests::native_id_xss_payload_escaped_in_output`. What is new here is
/// the structural check: not "the payload we thought of is gone" but "nothing
/// the renderer never writes is present".
#[test]
fn native_entry_emits_no_attribute_the_renderer_never_writes() {
    let svg = render(&[hostile_task()], &[], None, None);
    assert_no_foreign_attributes(&svg, "native render()");
}

#[test]
fn wasm_entry_task_id_cannot_close_the_attribute() {
    // `render_svg` is the entry `arc-vue` uses; the native path does not cover
    // it, and the JSON hop is where an external id realistically arrives.
    let svg = render_svg(&hostile_task_json(), "[]", None, None);
    // Without this, a refusal (which returns the fixed empty chart) would sail
    // through every assertion below and report the entry point as safe.
    assert_ne!(
        svg,
        empty_svg(),
        "the hostile id must be rendered, not refused, or this test proves nothing"
    );
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
fn rejected_input_never_reaches_the_markup() {
    // serde echoes the offending value verbatim into its message
    // ("invalid type: string \"...\""), so any path that puts that message in
    // the markup hands the attacker their own bytes back under `v-html`.
    // `render_svg` refuses by returning the fixed empty chart instead; this
    // test states that contract in terms of the input, not of one payload
    // shape, so it also covers a future rejection that tries to explain itself.
    let hostile = r#"[{"id":"a","title":"t","progress_pct":"--><img src=x onerror=alert(1)>","start":"2026-06-01","end":"2026-06-02"}]"#;
    let out = render_svg(hostile, "[]", None, None);
    assert_eq!(
        out,
        empty_svg(),
        "refusal must return the fixed empty chart"
    );
    for fragment in ["-->", "<img", "onerror", "alert(1)"] {
        assert!(
            !out.contains(fragment),
            "attacker-controlled fragment {fragment:?} reached the markup: {out}"
        );
    }
}

#[test]
fn rejection_reason_stays_json_data_even_when_it_quotes_the_input() {
    // `render_svg_error` is the channel that tells "nothing to draw" apart from
    // "refused", and it does echo serde's message. That is safe only while the
    // message stays a JSON string the JS side can render as text — if the quote
    // handling ever breaks, the payload arrives as structure, not as data.
    let hostile = r#"[{"id":"a","title":"t","progress_pct":"\"}]<img src=x onerror=alert(1)>","start":"2026-06-01","end":"2026-06-02"}]"#;
    let err = render_svg_error(hostile, "[]").expect("this input must be refused");
    let parsed: serde_json::Value = serde_json::from_str(&err)
        .unwrap_or_else(|e| panic!("rejection reason is not valid JSON ({e}): {err}"));
    assert!(
        parsed.get("error").and_then(|v| v.as_str()).is_some(),
        "expected an `error` string the JS side can branch on: {err}"
    );
    assert_eq!(
        parsed.get("code").and_then(|v| v.as_str()),
        Some("parse_error")
    );
}

#[test]
fn canvas_command_parse_error_on_deps_is_valid_json() {
    // `render::tests::canvas_parse_error_json_is_valid` only breaks the tasks
    // payload. The deps payload is deserialized separately, so its failure path
    // is a second, untested exit — and `parseCommandBuffer` on the JS side
    // throws inside a floating promise if either exit returns non-JSON.
    let ok_tasks =
        r#"[{"id":"a","title":"t","progress_pct":10,"start":"2026-06-01","end":"2026-06-02"}]"#;
    let hostile_deps = r#"[{"blocker_task_id":{"x":"\"}]"},"blocked_task_id":"b"}]"#;
    let out = render_canvas_commands(ok_tasks, hostile_deps, None, None);
    let parsed: serde_json::Value = serde_json::from_str(&out)
        .unwrap_or_else(|e| panic!("error payload is not valid JSON ({e}): {out}"));
    assert!(parsed.get("error").is_some(), "got: {out}");
}
