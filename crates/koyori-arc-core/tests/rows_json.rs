//! `rows_json` と行の寸法の鍵。Wasm の入口が、使う側の並べた行をどう読み、何を断るか。
//! 設計書 docs/design/external-rows.md の 3 と 5 にあたる。

use koyori_arc_core::{
    empty_svg, render_canvas_commands, render_svg, render_svg_error, CODE_CANVAS_CAPACITY,
    CODE_INPUT_LIMIT, CODE_PARSE_ERROR,
};

const TASKS: &str = r#"[{"id":"a","title":"A","progress_pct":50,"start":"2026-06-01","end":"2026-06-08"},{"id":"b","title":"B","progress_pct":10,"start":"2026-06-02","end":"2026-06-05"}]"#;

fn s(json: &str) -> Option<String> {
    Some(json.to_string())
}

fn height_of(svg: &str) -> f64 {
    let start = svg.find(" height=\"").unwrap() + " height=\"".len();
    let rest = &svg[start..];
    rest[..rest.find('"').unwrap()].parse().unwrap()
}

/// 三つの入口が、同じ入力を同じ理由で断る。
fn assert_refused(options: Option<&str>, rows: &str, code: &str, mentions: &str) {
    let reported = render_svg_error(TASKS, "[]", options.map(str::to_string), s(rows))
        .unwrap_or_else(|| panic!("{rows}: must be refused"));
    let v: serde_json::Value = serde_json::from_str(&reported).unwrap();
    assert_eq!(v["code"], code, "{rows}: {v}");
    let message = v["error"].as_str().unwrap();
    assert!(message.contains(mentions), "{rows}: {message}");
    assert_eq!(
        render_svg(TASKS, "[]", options.map(str::to_string), s(rows)),
        empty_svg(),
        "{rows}"
    );
    let canvas: serde_json::Value = serde_json::from_str(&render_canvas_commands(
        TASKS,
        "[]",
        options.map(str::to_string),
        s(rows),
    ))
    .unwrap();
    assert_eq!(canvas["code"], code, "{rows}");
    assert_eq!(canvas["error"], v["error"], "{rows}");
}

#[test]
fn absent_rows_draw_the_chart_drawn_before_rows_existed() {
    let plain = render_svg(TASKS, "[]", None, None);
    assert_ne!(plain, empty_svg());
    assert_eq!(
        render_canvas_commands(TASKS, "[]", None, None),
        render_canvas_commands(TASKS, "[]", None, None)
    );
    // 課題の並びのままの一覧は、一覧の無い図と同じ
    assert_eq!(
        render_svg(
            TASKS,
            "[]",
            None,
            s(r#"[{"kind":"task","task_id":"a"},{"kind":"task","task_id":"b"}]"#)
        ),
        plain
    );
}

#[test]
fn a_group_row_adds_a_band_and_its_height() {
    let plain = render_svg(TASKS, "[]", None, None);
    let grouped = render_svg(
        TASKS,
        "[]",
        None,
        s(
            r#"[{"kind":"group"},{"kind":"task","task_id":"a"},{"kind":"task","task_id":"b","height":64}]"#,
        ),
    );
    assert!(grouped.contains(r#"class="group-row""#));
    assert!(!plain.contains(r#"class="group-row""#));
    assert_eq!(
        height_of(&grouped) - height_of(&plain),
        40.0 + (64.0 - 40.0)
    );
    assert_eq!(
        render_svg_error(TASKS, "[]", None, s(r#"[{"kind":"group"}]"#)),
        None
    );
}

#[test]
fn sizes_are_read_from_the_options() {
    let plain = render_svg(TASKS, "[]", None, None);
    let sized = render_svg(
        TASKS,
        "[]",
        s(r#"{"row_height_px":32,"header_height_px":48,"bar_height_px":12,"bar_align":"bottom"}"#),
        None,
    );
    // 見出しが 18 高く、二つの行が 8 ずつ低い
    assert_eq!(height_of(&sized) - height_of(&plain), 18.0 - 2.0 * 8.0);
    for options in [
        r#"{"bar_align":"middle"}"#,
        r#"{"row_height_px":null,"bar_align":null}"#,
    ] {
        assert_eq!(
            render_svg(TASKS, "[]", s(options), None),
            plain,
            "{options}"
        );
    }
}

#[test]
fn a_row_list_that_cannot_be_read_is_refused() {
    for rows in [
        "not json",
        "{}",
        r#"[{"kind":"lane"}]"#,
        r#"[{"task_id":"a"}]"#,
        r#"[{"kind":"task"}]"#,
    ] {
        assert_refused(None, rows, CODE_PARSE_ERROR, "rows");
    }
}

#[test]
fn a_row_list_must_name_each_task_once_and_only_tasks_given() {
    assert_refused(
        None,
        r#"[{"kind":"task","task_id":"z"}]"#,
        CODE_PARSE_ERROR,
        "unknown task_id `z`",
    );
    assert_refused(
        None,
        r#"[{"kind":"task","task_id":"a"},{"kind":"task","task_id":"a"}]"#,
        CODE_PARSE_ERROR,
        "listed twice",
    );
}

#[test]
fn a_height_must_be_a_number_above_zero_and_at_most_the_limit() {
    for bad in ["0", "-1", "1001"] {
        assert_refused(
            None,
            &format!(r#"[{{"kind":"group","height":{bad}}}]"#),
            CODE_PARSE_ERROR,
            "height",
        );
    }
    // 数でない高さは、一覧を読めぬ時と同じ断り
    for bad in [r#""40""#, "true"] {
        assert_refused(
            None,
            &format!(r#"[{{"kind":"group","height":{bad}}}]"#),
            CODE_PARSE_ERROR,
            "rows",
        );
    }
    for (key, bad) in [
        ("row_height_px", "0"),
        ("header_height_px", "-4"),
        ("bar_height_px", "1001"),
        ("row_height_px", r#""32""#),
    ] {
        let options = format!(r#"{{"{key}":{bad}}}"#);
        let v: serde_json::Value = serde_json::from_str(
            &render_svg_error(TASKS, "[]", s(&options), None).expect("must be refused"),
        )
        .unwrap();
        assert_eq!(v["code"], CODE_PARSE_ERROR, "{options}");
        assert!(v["error"].as_str().unwrap().contains(key), "{options}");
    }
    let v: serde_json::Value = serde_json::from_str(
        &render_svg_error(TASKS, "[]", s(r#"{"bar_align":"top"}"#), None).unwrap(),
    )
    .unwrap();
    assert!(v["error"].as_str().unwrap().contains("bar_align"));
}

#[test]
fn a_bar_may_not_be_taller_than_a_task_row() {
    assert_refused(
        Some(r#"{"bar_height_px":30}"#),
        r#"[{"kind":"task","task_id":"a","height":24},{"kind":"group","height":10}]"#,
        CODE_PARSE_ERROR,
        "bar_height_px",
    );
    // 群れの行は棒を持たぬので数えぬ
    assert_eq!(
        render_svg_error(
            TASKS,
            "[]",
            s(r#"{"bar_height_px":30}"#),
            s(r#"[{"kind":"task","task_id":"a"},{"kind":"group","height":10}]"#)
        ),
        None
    );
    let v: serde_json::Value = serde_json::from_str(
        &render_svg_error(
            TASKS,
            "[]",
            s(r#"{"bar_height_px":30,"row_height_px":24}"#),
            None,
        )
        .unwrap(),
    )
    .unwrap();
    assert!(v["error"].as_str().unwrap().contains("bar_height_px"));
}

#[test]
fn too_many_rows_are_refused_before_they_are_read_one_by_one() {
    let many = format!(
        "[{}]",
        (0..20_001)
            .map(|_| r#"{"kind":"group"}"#)
            .collect::<Vec<_>>()
            .join(",")
    );
    assert_refused(None, &many, CODE_INPUT_LIMIT, "row count");
}

#[test]
fn the_canvas_guard_measures_the_rows_as_drawn() {
    // 二つの課題でも、行を高くすれば canvas の一辺を越える
    let tall = r#"[{"kind":"task","task_id":"a","height":1000}]"#;
    let groups = format!(
        "[{}{}]",
        (0..17)
            .map(|_| r#"{"kind":"group","height":1000},"#)
            .collect::<String>(),
        tall.trim_start_matches('[').trim_end_matches(']')
    );
    let v: serde_json::Value =
        serde_json::from_str(&render_canvas_commands(TASKS, "[]", None, s(&groups))).unwrap();
    assert_eq!(v["code"], CODE_CANVAS_CAPACITY, "{v}");
    // SVG は canvas の上限を持たぬ
    assert_eq!(render_svg_error(TASKS, "[]", None, s(&groups)), None);
}
