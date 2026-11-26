//! 課題の行が渡す色で棒を塗る描き方と断り方。
//! 設計書 docs/design/row-bar-color.md の 3 と 4 にあたる。

use koyori_arc_core::{
    render_canvas_commands, render_svg, render_svg_error, render_with_rows, ChartOptions, GanttRow,
    GanttTask, RowColorPercent, RowColorProgress, TaskRow, CODE_PARSE_ERROR,
};

const TASKS: &str = r#"[{"id":"a","title":"A","progress_pct":50,"start":"2026-11-02","end":"2026-11-06"},{"id":"b","title":"B","progress_pct":0,"start":"2026-11-05","end":"2026-11-10"}]"#;

fn s(json: &str) -> Option<String> {
    Some(json.to_string())
}

fn svg(rows: &str, options: Option<&str>) -> String {
    render_svg(TASKS, "[]", options.map(str::to_string), s(rows))
}

fn canvas(rows: &str, options: Option<&str>) -> serde_json::Value {
    serde_json::from_str(&render_canvas_commands(
        TASKS,
        "[]",
        options.map(str::to_string),
        s(rows),
    ))
    .unwrap()
}

/// The ops of one task's group in a command buffer.
fn group_ops(buffer: &serde_json::Value, task_id: &str) -> Vec<serde_json::Value> {
    let ops = buffer["ops"].as_array().unwrap();
    let start = ops
        .iter()
        .position(|op| op["GroupStart"]["task_id"] == task_id)
        .unwrap_or_else(|| panic!("no group for {task_id}"));
    ops[start + 1..]
        .iter()
        .take_while(|op| *op != "GroupEnd")
        .cloned()
        .collect()
}

/// The markup of one task's `<g>` in an SVG.
fn group_svg<'a>(svg: &'a str, task_id: &str) -> &'a str {
    let open = format!(r#"<g data-task-id="{task_id}">"#);
    let start = svg
        .find(&open)
        .unwrap_or_else(|| panic!("no group for {task_id}"));
    let end = svg[start..].find("</g>").unwrap() + start;
    &svg[start..end]
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

const PLAIN_ROWS: &str = r#"[{"kind":"task","task_id":"a"},{"kind":"task","task_id":"b"}]"#;

#[test]
fn rows_without_a_colour_draw_as_before_whatever_the_choices() {
    let plain = svg(PLAIN_ROWS, None);
    let nulls = r#"[{"kind":"task","task_id":"a","color":null},{"kind":"task","task_id":"b"}]"#;
    assert_eq!(svg(nulls, None), plain);
    for options in [
        r#"{"row_color_progress":"line","row_color_percent":"hidden"}"#,
        r#"{"row_color_progress":"shade","row_color_percent":"halo"}"#,
        r#"{"row_color_progress":null,"row_color_percent":null}"#,
    ] {
        assert_eq!(svg(PLAIN_ROWS, Some(options)), plain, "{options}");
        assert_eq!(
            canvas(PLAIN_ROWS, Some(options)),
            canvas(PLAIN_ROWS, None),
            "{options}"
        );
    }
    // No row colour, no palette entry.
    let colors = canvas(PLAIN_ROWS, None)["palette"]["colors"].clone();
    assert_eq!(colors.as_array().unwrap().len(), 13);
}

#[test]
fn shade_fills_the_rest_faint_and_the_progress_in_the_colour() {
    let rows =
        r##"[{"kind":"task","task_id":"a","color":"#ffff00"},{"kind":"task","task_id":"b"}]"##;
    let out = svg(rows, None);
    let a = group_svg(&out, "a");
    assert!(
        a.contains(r##"class="bar-bg" x="120" y="40" width="120" height="20" rx="4" fill="#ffff00" fill-opacity="0.35"/>"##),
        "{a}"
    );
    assert!(
        a.contains(r##"class="bar-progress bar-tier-mid" x="120" y="40" width="60" height="20" rx="4" fill="#ffff00"/>"##),
        "{a}"
    );
    // Always an outline; no state is `BarStroke`, which falls back to `GridLabel`.
    assert!(a.contains(r##"stroke="#6b7280" stroke-width="1""##), "{a}");
    // The uncoloured row is the row it was.
    assert_eq!(group_svg(&out, "b"), group_svg(&svg(PLAIN_ROWS, None), "b"));

    let buffer = canvas(rows, None);
    let ops = group_ops(&buffer, "a");
    assert_eq!(
        ops[0],
        serde_json::json!({"FillRectAlpha": {"x": 120.0, "y": 40.0, "w": 120.0, "h": 20.0, "color_id": 256, "radius": 4.0, "alpha": 0.35}})
    );
    assert_eq!(ops[1]["FillRect"]["color_id"], 256);
    assert_eq!(ops[2]["StrokePath"]["color_id"], 15);
    let colors = buffer["palette"]["colors"].as_array().unwrap();
    assert_eq!(colors.len(), 14);
    assert_eq!(colors[13], serde_json::json!([{"Row": 0}, "#ffff00"]));
}

#[test]
fn line_fills_the_whole_bar_and_draws_the_progress_under_it() {
    let rows =
        r##"[{"kind":"task","task_id":"a","color":"#1e3a8a"},{"kind":"task","task_id":"b"}]"##;
    let out = svg(rows, Some(r#"{"row_color_progress":"line"}"#));
    let a = group_svg(&out, "a");
    assert!(
        a.contains(
            r##"class="bar-bg" x="120" y="40" width="120" height="20" rx="4" fill="#1e3a8a"/>"##
        ),
        "{a}"
    );
    assert!(!a.contains("bar-progress"), "{a}");
    // 2px under the bar's foot at y = 60, 3px tall, in `Grid` then the outline's colour.
    assert!(
        a.contains(
            r##"class="row-progress-track" x="120" y="62" width="120" height="3" fill="#e5e7eb"/>"##
        ),
        "{a}"
    );
    assert!(
        a.contains(
            r##"class="row-progress" x="120" y="62" width="60" height="3" fill="#6b7280"/>"##
        ),
        "{a}"
    );

    let ops = group_ops(&canvas(rows, Some(r#"{"row_color_progress":"line"}"#)), "a");
    assert_eq!(ops[0]["FillRect"]["color_id"], 256);
    assert_eq!(ops[1]["FillRect"]["color_id"], 7);
    assert_eq!(ops[2]["FillRect"]["w"], 60.0);
    assert_eq!(ops[2]["FillRect"]["color_id"], 15);
}

#[test]
fn the_progress_line_stays_inside_a_short_row() {
    // A row 22px tall (y 30 to 52) leaves 1px under a 20px bar at y 31: the
    // line's foot meets the row's, over the bar's foot.
    let rows = r##"[{"kind":"task","task_id":"a","color":"#1e3a8a","height":22},{"kind":"task","task_id":"b"}]"##;
    let out = svg(rows, Some(r#"{"row_color_progress":"line"}"#));
    let a = group_svg(&out, "a");
    assert!(
        a.contains(r##"class="row-progress-track" x="120" y="49" width="120" height="3""##),
        "{a}"
    );
}

#[test]
fn the_state_keeps_its_outline_and_note() {
    let rows = r##"[{"kind":"task","task_id":"a","color":"#ffffff","state":"warn","note":"遅れ"},{"kind":"task","task_id":"b","color":"#ffffff","state":"done"}]"##;
    let out = svg(rows, None);
    let a = group_svg(&out, "a");
    // `WarnStroke` falls back to `Progress`.
    assert!(a.contains(r##"stroke="#ef4444" stroke-width="1""##), "{a}");
    assert!(
        !a.contains(r##"fill="#d1d5db""##),
        "the state's fill gives way: {a}"
    );
    assert!(
        out.contains(r##"fill="#ef4444" font-size="11" dominant-baseline="middle">遅れ</text>"##),
        "{out}"
    );
    let b = group_svg(&out, "b");
    // `DoneStroke` falls back to `GridLabel`.
    assert!(b.contains(r##"stroke="#6b7280" stroke-width="1""##), "{b}");
}

#[test]
fn halo_draws_the_label_inside_the_bar_over_a_band() {
    let rows =
        r##"[{"kind":"task","task_id":"a","color":"#1e3a8a"},{"kind":"task","task_id":"b"}]"##;
    let out = svg(rows, None);
    let a = group_svg(&out, "a");
    assert!(
        a.contains(r##"text-anchor="middle" fill="#374151" stroke="#ffffff" stroke-width="3" stroke-linejoin="round" paint-order="stroke" font-size="11" font-weight="600" dominant-baseline="middle">50%</text>"##),
        "{a}"
    );
    // The uncoloured row's label has no halo.
    assert!(!group_svg(&out, "b").contains("paint-order"));

    let ops = group_ops(&canvas(rows, None), "a");
    let text = ops
        .iter()
        .find(|op| op.get("DrawHaloText").is_some())
        .unwrap();
    assert_eq!(text["DrawHaloText"]["color_id"], 27);
    assert_eq!(text["DrawHaloText"]["halo_color_id"], 28);
    assert_eq!(text["DrawHaloText"]["halo_width"], 3.0);
    assert_eq!(text["DrawHaloText"]["text"], "50%");
}

#[test]
fn the_halo_colours_follow_the_palette() {
    let rows =
        r##"[{"kind":"task","task_id":"a","color":"#1e3a8a"},{"kind":"task","task_id":"b"}]"##;
    let options = r##"{"palette":{"ColoredBarText":"#f9fafb","ColoredBarHalo":"var(--ink)"}}"##;
    let a = svg(rows, Some(options));
    let a = group_svg(&a, "a");
    assert!(
        a.contains(r##"fill="#f9fafb" stroke="var(--ink)" stroke-width="3""##),
        "{a}"
    );
}

#[test]
fn a_label_beside_a_coloured_bar_stays_as_it_was() {
    // At 4px a day the bars are too short for their labels, which run on past them.
    let options = r#"{"px_per_day":4}"#;
    let rows =
        r##"[{"kind":"task","task_id":"a","color":"#1e3a8a"},{"kind":"task","task_id":"b"}]"##;
    let out = svg(rows, Some(options));
    let a = group_svg(&out, "a");
    assert!(a.contains(r#"text-anchor="start""#), "{a}");
    assert!(!a.contains("paint-order"), "{a}");
}

#[test]
fn hidden_leaves_out_the_labels_of_coloured_rows_and_their_width() {
    // Without the legend, whose width would hide the label's.
    let options_halo = r#"{"px_per_day":4,"legend":"hidden"}"#;
    let options_hidden = r#"{"px_per_day":4,"legend":"hidden","row_color_percent":"hidden"}"#;
    // `b` ends last; its label runs past the last day and holds up the width.
    let rows =
        r##"[{"kind":"task","task_id":"a"},{"kind":"task","task_id":"b","color":"#ffff00"}]"##;
    let halo = svg(rows, Some(options_halo));
    let hidden = svg(rows, Some(options_hidden));
    assert!(group_svg(&halo, "b").contains(">0%</text>"));
    assert!(!group_svg(&hidden, "b").contains("%</text>"));
    assert!(group_svg(&hidden, "a").contains(">50%</text>"), "{hidden}");
    let width = |svg: &str| -> f64 {
        let v: serde_json::Value = serde_json::from_str(&render_canvas_commands(
            TASKS,
            "[]",
            s(if svg == "halo" {
                options_halo
            } else {
                options_hidden
            }),
            s(rows),
        ))
        .unwrap();
        v["viewport_width"].as_f64().unwrap()
    };
    assert!(width("hidden") < width("halo"));
    assert!(halo.contains(&format!(r#"width="{}""#, width("halo"))));
    assert!(hidden.contains(&format!(r#"width="{}""#, width("hidden"))));
    // The canvas draws no label for the coloured row either.
    let ops = group_ops(&canvas(rows, Some(options_hidden)), "b");
    assert!(ops.iter().all(|op| op.get("DrawHaloText").is_none()
        && !op["DrawText"]["text"]
            .as_str()
            .is_some_and(|t| t.ends_with('%'))));
}

#[test]
fn rows_of_one_colour_share_a_palette_entry() {
    let tasks = r#"[{"id":"a","title":"A","progress_pct":50,"start":"2026-11-02","end":"2026-11-06"},{"id":"b","title":"B","progress_pct":0,"start":"2026-11-05","end":"2026-11-10"},{"id":"c","title":"C","progress_pct":10,"start":"2026-11-05","end":"2026-11-10"}]"#;
    let rows = r##"[{"kind":"task","task_id":"a","color":"#ff0000"},{"kind":"group"},{"kind":"task","task_id":"b","color":"var(--s)"},{"kind":"task","task_id":"c","color":"#ff0000"}]"##;
    let buffer: serde_json::Value =
        serde_json::from_str(&render_canvas_commands(tasks, "[]", None, s(rows))).unwrap();
    let colors = buffer["palette"]["colors"].as_array().unwrap();
    assert_eq!(
        &colors[13..],
        &[
            serde_json::json!([{"Row": 0}, "#ff0000"]),
            serde_json::json!([{"Row": 1}, "var(--s)"])
        ]
    );
    let tag = |id: &str| group_ops(&buffer, id)[0]["FillRectAlpha"]["color_id"].clone();
    assert_eq!(tag("a"), 256);
    assert_eq!(tag("b"), 257);
    assert_eq!(tag("c"), 256);
    let out = render_svg(tasks, "[]", None, s(rows));
    assert!(group_svg(&out, "b").contains(r#"fill="var(--s)" fill-opacity="0.35""#));
}

#[test]
fn every_colour_form_palette_takes_is_taken() {
    for color in [
        "#fff",
        "#ffff",
        "#ffffff",
        "#ffffff80",
        "rgb(255, 255, 0)",
        "rgba(255 255 0 / 50%)",
        "hsl(60deg 100% 50%)",
        "hsla(60, 100%, 50%, 0.5)",
        "oklch(0.97 0.2 110)",
        "oklab(0.97 -0.07 0.2)",
        "var(--status-color_1)",
    ] {
        let rows = format!(
            r#"[{{"kind":"task","task_id":"a","color":"{color}"}},{{"kind":"task","task_id":"b"}}]"#
        );
        assert_eq!(
            render_svg_error(TASKS, "[]", None, s(&rows)),
            None,
            "{color}"
        );
        assert!(
            svg(&rows, None).contains(&format!(r#"fill="{color}""#)),
            "{color}"
        );
    }
}

#[test]
fn a_colour_the_chart_cannot_write_is_refused() {
    for color in [
        r#"#fff\" onload=\"alert(1)"#,
        "#fff><script>",
        "javascript:alert(1)",
        "url(#x)",
        "url(javascript:alert(1))",
        "red",
        "",
        "#ggg",
        "var(--x) ",
        "var(--x;y)",
        "rgb(1,2,3);fill:red",
        "expression(alert(1))",
        "RGB(1,2,3)",
    ] {
        refused(
            &format!(r#"[{{"kind":"task","task_id":"a","color":"{color}"}}]"#),
            "color of `a`",
        );
    }
    refused(
        &format!(
            r#"[{{"kind":"task","task_id":"a","color":"rgb({})"}}]"#,
            "1".repeat(96)
        ),
        "color of `a`",
    );
    refused(r#"[{"kind":"task","task_id":"a","color":12}]"#, "rows");
}

#[test]
fn the_longest_colour_is_taken() {
    let color = format!("rgb({})", "1".repeat(95));
    assert_eq!(color.chars().count(), 100);
    let rows = format!(r#"[{{"kind":"task","task_id":"a","color":"{color}"}}]"#);
    assert_eq!(render_svg_error(TASKS, "[]", None, s(&rows)), None);
}

#[test]
fn a_row_colour_is_not_a_palette_name() {
    for options in [
        r##"{"palette":{"Row":"#fff"}}"##,
        r##"{"palette":{"Row(0)":"#fff"}}"##,
    ] {
        let v: serde_json::Value = serde_json::from_str(
            &render_svg_error(TASKS, "[]", s(options), None).expect("refused"),
        )
        .unwrap();
        assert!(
            v["error"].as_str().unwrap().contains("unknown colour name"),
            "{options}: {v}"
        );
    }
}

#[test]
fn the_choices_take_only_their_words() {
    for (options, mentions) in [
        (r#"{"row_color_progress":"tint"}"#, "row_color_progress"),
        (r#"{"row_color_progress":1}"#, "row_color_progress"),
        (r#"{"row_color_percent":"shown"}"#, "row_color_percent"),
    ] {
        let v: serde_json::Value = serde_json::from_str(
            &render_svg_error(TASKS, "[]", s(options), None).expect("refused"),
        )
        .unwrap();
        assert!(v["error"].as_str().unwrap().contains(mentions), "{v}");
    }
}

#[test]
fn a_native_caller_s_unwritable_colour_is_drawn_as_none() {
    let tasks: Vec<GanttTask> = serde_json::from_str(TASKS).unwrap();
    let plain_rows = vec![
        GanttRow::Task(TaskRow::new("a")),
        GanttRow::Task(TaskRow::new("b")),
    ];
    let plain = render_with_rows(&tasks, &[], Some(&plain_rows), &ChartOptions::default());
    let bad = vec![
        GanttRow::Task(TaskRow {
            color: Some(r#"red" onload="alert(1)"#.to_string()),
            ..TaskRow::new("a")
        }),
        GanttRow::Task(TaskRow::new("b")),
    ];
    let out = render_with_rows(&tasks, &[], Some(&bad), &ChartOptions::default());
    assert_eq!(out, plain);
    assert!(!out.contains("onload"));

    let good = vec![
        GanttRow::Task(TaskRow {
            color: Some("#00ff00".to_string()),
            ..TaskRow::new("a")
        }),
        GanttRow::Task(TaskRow::new("b")),
    ];
    let options = ChartOptions {
        row_color_progress: RowColorProgress::Line,
        row_color_percent: RowColorPercent::Hidden,
        ..Default::default()
    };
    let out = render_with_rows(&tasks, &[], Some(&good), &options);
    assert!(out.contains(r#"class="row-progress""#));
    assert!(out.contains(r##"fill="#00ff00""##));
}

#[test]
fn a_point_and_a_milestone_take_the_colour_inside_an_outline() {
    let tasks = r#"[{"id":"a","title":"A","progress_pct":50,"start":"2026-11-02","end":"2026-11-06"},{"id":"m","title":"M","progress_pct":0,"start":"2026-11-08","end":"2026-11-08"}]"#;
    let rows = r##"[{"kind":"task","task_id":"a","shape":"point","color":"#ffff00","state":"warn"},{"kind":"task","task_id":"m","color":"#ffff00"}]"##;
    let out = render_svg(tasks, "[]", None, s(rows));
    let a = group_svg(&out, "a");
    // A ring of the outline's colour (10px) with the colour inside (8px).
    assert!(a.contains(r##"class="bar-point""##));
    assert!(
        a.contains(r##"width="10" height="10" rx="5" fill="#ef4444"/>"##),
        "{a}"
    );
    assert!(
        a.contains(r##"width="8" height="8" rx="4" fill="#ffff00"/>"##),
        "{a}"
    );
    let m = group_svg(&out, "m");
    assert!(
        m.contains(r##"fill="#ffff00" stroke="#6b7280" stroke-width="1"/>"##),
        "{m}"
    );
}
