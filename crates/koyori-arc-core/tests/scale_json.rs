//! `scale`、`px_per_day`、`weekend_days`、`weekday_labels` の読み方と断り方。
//! 設計書 docs/design/day-scale.md の 3 にあたる。

use koyori_arc_core::{render_canvas_commands, render_svg, render_svg_error, CODE_PARSE_ERROR};

const TASKS: &str =
    r#"[{"id":"a","title":"A","progress_pct":50,"start":"2026-11-02","end":"2026-11-16"}]"#;

fn s(json: &str) -> Option<String> {
    Some(json.to_string())
}

fn width_of(svg: &str) -> f64 {
    let start = svg.find(" width=\"").unwrap() + " width=\"".len();
    let rest = &svg[start..];
    rest[..rest.find('"').unwrap()].parse().unwrap()
}

fn refused(options: &str, mentions: &str) {
    let reported = render_svg_error(TASKS, "[]", s(options), None)
        .unwrap_or_else(|| panic!("{options}: must be refused"));
    let v: serde_json::Value = serde_json::from_str(&reported).unwrap();
    assert_eq!(v["code"], CODE_PARSE_ERROR, "{options}");
    assert!(
        v["error"].as_str().unwrap().contains(mentions),
        "{options}: {v}"
    );
    let canvas: serde_json::Value =
        serde_json::from_str(&render_canvas_commands(TASKS, "[]", s(options), None)).unwrap();
    assert_eq!(canvas["error"], v["error"], "{options}");
}

#[test]
fn the_defaults_however_they_are_spelt_draw_the_default_chart() {
    let plain = render_svg(TASKS, "[]", None, None);
    for options in [
        r#"{"scale":"week"}"#,
        r#"{"px_per_day":30}"#,
        r#"{"weekend_days":[]}"#,
        r#"{"scale":null,"px_per_day":null,"weekend_days":null,"weekday_labels":null}"#,
    ] {
        assert_eq!(
            render_svg(TASKS, "[]", s(options), None),
            plain,
            "{options}"
        );
    }
}

#[test]
fn the_keys_are_read() {
    let plain = render_svg(TASKS, "[]", None, None);
    let wide = render_svg(TASKS, "[]", s(r#"{"px_per_day":48}"#), None);
    assert_eq!(width_of(&wide) - width_of(&plain), 14.0 * (48.0 - 30.0));

    let day = render_svg(
        TASKS,
        "[]",
        s(
            r#"{"scale":"day","weekend_days":[5,6],"weekday_labels":["月","火","水","木","金","土","日"]}"#,
        ),
        None,
    );
    assert!(day.contains(">月</text>") && day.contains(">日</text>"));
    assert!(day.contains(">16</text>") || day.contains(">15</text>"));
    assert!(day.contains(r#"class="weekend""#));

    let month = render_svg(TASKS, "[]", s(r#"{"scale":"month"}"#), None);
    assert!(month.contains(">2026/11</text>"));
}

#[test]
fn values_the_chart_cannot_use_are_refused() {
    for bad in [r#""days""#, r#""Week""#, "1", "true"] {
        refused(&format!(r#"{{"scale":{bad}}}"#), "scale");
    }
    for bad in ["0", "-30", "1001", r#""30""#] {
        refused(&format!(r#"{{"px_per_day":{bad}}}"#), "px_per_day");
    }
    for bad in ["[7]", "[-1]", "[5,5]", "[1.5]", r#"["sat"]"#, "6", "{}"] {
        refused(&format!(r#"{{"weekend_days":{bad}}}"#), "weekend_days");
    }
    for bad in [
        r#"["a","b","c","d","e","f"]"#,
        r#"["a","b","c","d","e","f","g","h"]"#,
        r#"["a","b","c","d","e","f",""]"#,
        r#"["a","b","c","d","e","f","123456789"]"#,
        r#"["a","b","c","d","e","f",1]"#,
        r#""月""#,
    ] {
        refused(&format!(r#"{{"weekday_labels":{bad}}}"#), "weekday_labels");
    }
}

#[test]
fn a_weekday_label_is_escaped_like_any_text() {
    let svg = render_svg(
        TASKS,
        "[]",
        s(r#"{"scale":"day","weekday_labels":["<b>","&","\"","'","e","f","g"]}"#),
        None,
    );
    assert!(svg.contains("&lt;b&gt;") && svg.contains("&amp;"));
    assert!(!svg.contains("<b>"));
}

/// SVG の字（`>…</text>`）を並べる。
fn svg_texts(svg: &str) -> Vec<String> {
    svg.split("</text>")
        .filter_map(|chunk| chunk.rfind('>').map(|i| chunk[i + 1..].to_string()))
        .filter(|t| !t.is_empty())
        .collect()
}

/// canvas の命の字を並べる。
fn canvas_texts(json: &str) -> Vec<String> {
    let v: serde_json::Value = serde_json::from_str(json).unwrap();
    v["ops"]
        .as_array()
        .unwrap_or_else(|| panic!("{v}"))
        .iter()
        .filter_map(|op| op.get("DrawText"))
        .map(|t| t["text"].as_str().unwrap().to_string())
        .collect()
}

#[test]
fn a_milestone_alone_draws_its_day_on_both_entry_points() {
    // 日曜（2026-11-08）に始まり終わるマイルストーン一つ。日の段、見出しの高さは渡さぬ
    let task =
        r#"[{"id":"m","title":"M","progress_pct":0,"start":"2026-11-08","end":"2026-11-08"}]"#;
    let options =
        s(r#"{"scale":"day","percent_labels":"hidden","row_labels":"hidden","legend":"hidden"}"#);
    let svg = svg_texts(&render_svg(task, "[]", options.clone(), None));
    let canvas = canvas_texts(&render_canvas_commands(task, "[]", options, None));
    for want in ["2026/11", "8", "Su"] {
        assert!(svg.iter().any(|t| t == want), "SVG: {want} in {svg:?}");
        assert!(
            canvas.iter().any(|t| t == want),
            "canvas: {want} in {canvas:?}"
        );
    }
    assert_eq!(svg, canvas);
}
