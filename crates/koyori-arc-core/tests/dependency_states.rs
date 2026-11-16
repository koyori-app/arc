//! 鍵のある依存の辺（docs/design/dependency-states.md）: 状態の色と破線、棒の下の層、板の形の道、
//! 阻まれる棒の右の添えの字、群れの見出しの行と範囲の外の端の代わり、断り。SVG と canvas の両方の入口で揃える。

use koyori_arc_core::{render_canvas_commands, render_svg, render_svg_error, CODE_PARSE_ERROR};

fn s(json: &str) -> Option<String> {
    Some(json.to_string())
}

// 行の高さ 40、見出し 30（週の目盛り）、棒 20 の真ん中寄せ。日は 30px、題の列は 120px。
// 行 0: 群れ、行 1: a（11/2〜11/5）、行 2: b（11/9〜11/12）、行 3: c（11/3〜11/6）、行 4: 畳んだ群れ。
const TASKS: &str = r#"[{"id":"a","title":"A","progress_pct":50,"start":"2026-11-02","end":"2026-11-06"},{"id":"b","title":"B","progress_pct":0,"start":"2026-11-09","end":"2026-11-13"},{"id":"c","title":"C","progress_pct":0,"start":"2026-11-03","end":"2026-11-07"}]"#;
const ROWS: &str = r#"[{"kind":"group"},{"kind":"task","task_id":"a"},{"kind":"task","task_id":"b"},{"kind":"task","task_id":"c"},{"kind":"group"}]"#;
const OPTIONS: &str = r##"{"legend":"hidden","palette":{"Dep":"#0000aa","DepDone":"#aaaaaa","DepDelayed":"#dd0000"}}"##;

const HEADER: f64 = 30.0;
const ROW: f64 = 40.0;
const LABEL_W: f64 = 120.0;
const DAY: f64 = 30.0;

fn row_mid(i: usize) -> f64 {
    HEADER + i as f64 * ROW + ROW / 2.0
}
fn row_top(i: usize) -> f64 {
    HEADER + i as f64 * ROW
}
/// 11/2 から `d` 日目の左の x。
fn day_x(d: f64) -> f64 {
    LABEL_W + d * DAY
}

fn svg(deps: &str) -> String {
    render_svg(TASKS, deps, s(OPTIONS), s(ROWS))
}

fn canvas(deps: &str) -> serde_json::Value {
    serde_json::from_str(&render_canvas_commands(TASKS, deps, s(OPTIONS), s(ROWS))).unwrap()
}

/// SVG の `<path …/>` を、色・破線・道の三つ組で並べる。
fn svg_paths(svg: &str) -> Vec<(String, Option<String>, String)> {
    svg.split("<path d=\"")
        .skip(1)
        .map(|rest| {
            let tag = rest.split("/>").next().unwrap();
            let d = tag.split('"').next().unwrap().to_string();
            let attr = |name: &str| {
                tag.split(&format!("{name}=\""))
                    .nth(1)
                    .map(|v| v.split('"').next().unwrap().to_string())
            };
            (attr("stroke").unwrap(), attr("stroke-dasharray"), d)
        })
        .collect()
}

/// 道の点（M・L・Q の行き先）を並べる。矢じりの `M` から後は除く。
fn route(d: &str) -> Vec<(f64, f64)> {
    let body = d.split(" M ").next().unwrap();
    let t: Vec<&str> = body.split_whitespace().collect();
    let mut pts = Vec::new();
    let mut i = 0;
    while i < t.len() {
        match t[i] {
            "M" | "L" => {
                pts.push((t[i + 1].parse().unwrap(), t[i + 2].parse().unwrap()));
                i += 3;
            }
            "Q" => {
                // 角の頂点と、曲がった先
                pts.push((t[i + 1].parse().unwrap(), t[i + 2].parse().unwrap()));
                pts.push((t[i + 3].parse().unwrap(), t[i + 4].parse().unwrap()));
                i += 5;
            }
            other => panic!("unexpected {other} in {d}"),
        }
    }
    pts
}

fn texts(svg: &str) -> Vec<String> {
    svg.split("<text ")
        .skip(1)
        .map(|t| {
            t.split('>')
                .nth(1)
                .unwrap()
                .split('<')
                .next()
                .unwrap()
                .to_string()
        })
        .collect()
}

#[test]
fn an_edge_without_the_new_keys_is_drawn_as_before() {
    let plain = r#"[{"blocker_task_id":"a","blocked_task_id":"b"}]"#;
    let svg = svg(plain);
    assert!(!svg.contains("stroke-dasharray=\"4,3\""));
    // 今の矢印は Dep の一色で、棒の上の層に描く
    let paths = svg_paths(&svg);
    assert_eq!(paths.len(), 1);
    assert_eq!(paths[0].0, "#0000aa");
    assert!(svg.find("class=\"bar-bg\"").unwrap() < svg.find("<path d=").unwrap());
}

#[test]
fn each_state_takes_its_colour_and_delayed_is_dashed_in_both_entries() {
    let deps = r#"[
        {"blocker_task_id":"a","blocked_task_id":"b","state":"normal"},
        {"blocker_task_id":"a","blocked_task_id":"c","state":"done"},
        {"blocker_task_id":"c","blocked_task_id":"b","state":"delayed"}
    ]"#;
    let paths = svg_paths(&svg(deps));
    let colours: Vec<(&str, Option<&str>)> = paths
        .iter()
        .map(|(c, dash, _)| (c.as_str(), dash.as_deref()))
        .collect();
    assert_eq!(
        colours,
        [
            ("#0000aa", None),
            ("#aaaaaa", None),
            ("#dd0000", Some("4,3"))
        ]
    );
    // canvas: 普通と済みは StrokePath（色の番号 5 と 25）、遅れは StrokeDashedPath（26、4,3）
    let ops = canvas(deps)["ops"].as_array().unwrap().clone();
    let solid: Vec<(u64, String)> = ops
        .iter()
        .filter_map(|op| {
            let p = &op["StrokePath"];
            p.is_object().then(|| {
                (
                    p["color_id"].as_u64().unwrap(),
                    p["d"].as_str().unwrap().to_string(),
                )
            })
        })
        .collect();
    let dashed: Vec<(u64, String, String)> = ops
        .iter()
        .filter_map(|op| {
            let p = &op["StrokeDashedPath"];
            p.is_object().then(|| {
                (
                    p["color_id"].as_u64().unwrap(),
                    p["d"].as_str().unwrap().to_string(),
                    p["dash"].as_str().unwrap().to_string(),
                )
            })
        })
        .collect();
    assert_eq!(solid, [(5, paths[0].2.clone()), (25, paths[1].2.clone())]);
    assert_eq!(dashed, [(26, paths[2].2.clone(), "4,3".to_string())]);
}

#[test]
fn a_styled_edge_runs_under_the_bars() {
    let svg = svg(r#"[{"blocker_task_id":"a","blocked_task_id":"b","state":"normal"}]"#);
    assert!(svg.find("<path d=").unwrap() < svg.find("class=\"bar-bg\"").unwrap());
}

#[test]
fn it_leaves_inside_the_blocker_s_end_goes_down_and_turns_right_into_the_blocked_bar() {
    let svg = svg(r#"[{"blocker_task_id":"a","blocked_task_id":"b","state":"normal"}]"#);
    let d = &svg_paths(&svg)[0].2;
    let pts = route(d);
    // a の棒は 11/2〜11/5（四日）。終わりの 8px 手前の、棒の下の縁から出る
    let a_bottom = row_mid(1) + 10.0;
    assert_eq!(pts[0], (day_x(4.0) - 8.0, a_bottom), "{d}");
    // 縦に下ろして b の行の中ほどで右へ折れ、b の頭の 2px 手前で止まる
    assert!(
        pts.iter()
            .all(|p| p.0 == day_x(4.0) - 8.0 || p.1 == row_mid(2)),
        "{d}"
    );
    assert_eq!(*pts.last().unwrap(), (day_x(7.0) - 2.0, row_mid(2)), "{d}");
}

#[test]
fn a_blocked_bar_starting_before_the_edge_leaves_is_entered_from_its_row_s_top_border() {
    let svg = svg(r#"[{"blocker_task_id":"a","blocked_task_id":"c","state":"normal"}]"#);
    let d = &svg_paths(&svg)[0].2;
    let pts = route(d);
    // c は a の終わりより前（11/3）に始まる。c の行の上の境まで下ろし、c の頭の左へ戻ってから入る
    assert!(pts.iter().any(|p| p.1 == row_top(3)), "{d}");
    assert!(pts.iter().any(|p| p.0 < day_x(1.0) - 2.0), "{d}");
    assert_eq!(*pts.last().unwrap(), (day_x(1.0) - 2.0, row_mid(3)), "{d}");
}

#[test]
fn the_note_sits_right_of_the_blocked_bar_after_the_row_s_own_note() {
    let rows = r#"[{"kind":"group"},{"kind":"task","task_id":"a"},{"kind":"task","task_id":"b","state":"warn","note":"目安 11/9 を過ぎた"},{"kind":"task","task_id":"c"},{"kind":"group"}]"#;
    let deps = r#"[{"blocker_task_id":"c","blocked_task_id":"b","state":"delayed","note":"C 待ち"},{"blocker_task_id":"a","blocked_task_id":"b","state":"normal","note":"A 待ち"}]"#;
    let svg = render_svg(TASKS, deps, s(OPTIONS), s(rows));
    let x_of = |text: &str| -> f64 {
        let at = svg
            .find(&format!(">{text}</text>"))
            .unwrap_or_else(|| panic!("{text}"));
        let tag = &svg[svg[..at].rfind("<text ").unwrap()..at];
        tag.split("x=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap()
            .parse()
            .unwrap()
    };
    let own = x_of("目安 11/9 を過ぎた");
    let c = x_of("C 待ち");
    let a = x_of("A 待ち");
    // 行の添えの字の後に、辺の添えの字が順に並び、重ならぬ
    assert!(own < c && c < a, "{own} {c} {a}");
    // 遅れの添えの字は遅れの色、普通は添えの字の色
    let fill_of = |text: &str| -> String {
        let at = svg.find(&format!(">{text}</text>")).unwrap();
        let tag = &svg[svg[..at].rfind("<text ").unwrap()..at];
        tag.split("fill=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap()
            .to_string()
    };
    assert_eq!(fill_of("C 待ち"), "#dd0000");
    assert_eq!(fill_of("A 待ち"), "#0000aa");
    // 帯を敷く（行の添えの字一つ、辺の添えの字二つ）
    assert_eq!(svg.matches("class=\"note-bg\"").count(), 3);
}

#[test]
fn an_edge_into_a_collapsed_group_ends_at_its_header_row_on_the_day_with_the_label() {
    let deps = r#"[{"blocker_task_id":"b","blocked_task_id":"x","state":"normal","blocked_at":{"row":4,"day":"2026-11-14","label":"DSK-2"}}]"#;
    let svg = svg(deps);
    let d = &svg_paths(&svg)[0].2;
    assert_eq!(*route(d).last().unwrap(), (day_x(12.0), row_mid(4)), "{d}");
    assert!(texts(&svg).contains(&"DSK-2".to_string()));
    assert_eq!(
        canvas(deps)["ops"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|op| op["StrokePath"]["d"] == *d)
            .count(),
        1
    );
}

#[test]
fn an_edge_to_a_task_outside_runs_along_the_blocker_s_row_to_the_edge_with_the_label() {
    let deps = r#"[{"blocker_task_id":"b","blocked_task_id":"x","state":"normal","blocked_at":{"edge":"end","label":"MOB-9 11/25"}}]"#;
    let svg = svg(deps);
    let d = &svg_paths(&svg)[0].2;
    let pts = route(d);
    // b の棒の右から、b の行の中ほどを、日の右の端まで
    assert_eq!(pts[0], (day_x(11.0), row_mid(2)), "{d}");
    assert_eq!(pts.last().unwrap().1, row_mid(2), "{d}");
    assert!(texts(&svg).contains(&"MOB-9 11/25".to_string()));
}

#[test]
fn an_edge_from_a_task_before_the_days_comes_in_from_the_left_edge() {
    let deps = r#"[{"blocker_task_id":"x","blocked_task_id":"c","state":"delayed","blocker_at":{"edge":"start","label":"OPS-1 10/20"}}]"#;
    let svg = svg(deps);
    let d = &svg_paths(&svg)[0].2;
    let pts = route(d);
    assert_eq!(pts[0], (LABEL_W, row_mid(3)), "{d}");
    assert_eq!(*pts.last().unwrap(), (day_x(1.0) - 2.0, row_mid(3)), "{d}");
    assert!(texts(&svg).contains(&"OPS-1 10/20".to_string()));
}

#[test]
fn every_point_of_every_styled_edge_stays_inside_the_chart() {
    let deps = r#"[
        {"blocker_task_id":"a","blocked_task_id":"c","state":"normal"},
        {"blocker_task_id":"x","blocked_task_id":"a","blocker_at":{"edge":"start"}},
        {"blocker_task_id":"b","blocked_task_id":"y","blocked_at":{"edge":"end"}}
    ]"#;
    for (_, _, d) in svg_paths(&render_svg(
        TASKS,
        deps,
        s(r#"{"legend":"hidden","row_labels":"hidden"}"#),
        s(ROWS),
    )) {
        for (x, _) in route(&d) {
            assert!(x >= 0.0, "{d}");
        }
    }
}

#[test]
fn a_note_widens_the_chart_to_hold_it() {
    let width = |deps: &str| -> f64 {
        let svg = svg(deps);
        svg.split("width=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap()
            .parse()
            .unwrap()
    };
    let plain = width(r#"[{"blocker_task_id":"a","blocked_task_id":"b","state":"normal"}]"#);
    let noted = width(
        r#"[{"blocker_task_id":"a","blocked_task_id":"b","state":"normal","note":"とても長い待ちの添えの字をここに書いて図の右へはみ出させる"}]"#,
    );
    assert!(noted > plain, "{plain} {noted}");
}

#[test]
fn the_new_keys_are_refused_where_the_chart_cannot_draw_them() {
    let refused = |deps: &str, mentions: &str| {
        let reported =
            render_svg_error(TASKS, deps, s(OPTIONS), s(ROWS)).unwrap_or_else(|| panic!("{deps}"));
        let v: serde_json::Value = serde_json::from_str(&reported).unwrap();
        assert_eq!(v["code"], CODE_PARSE_ERROR, "{deps}");
        assert!(v["error"].as_str().unwrap().contains(mentions), "{v}");
        let c = canvas(deps);
        assert_eq!(c["error"], v["error"], "{deps}");
    };
    refused(
        r#"[{"blocker_task_id":"a","blocked_task_id":"x","blocked_at":{"row":1,"day":"2026-11-14"}}]"#,
        "expected a group row",
    );
    refused(
        r#"[{"blocker_task_id":"a","blocked_task_id":"x","blocked_at":{"row":9,"day":"2026-11-14"}}]"#,
        "no such row",
    );
    refused(
        r#"[{"blocker_task_id":"a","blocked_task_id":"x","blocked_at":{"row":4,"day":"2040-01-01"}}]"#,
        "days from the first task",
    );
    let long = "あ".repeat(201);
    refused(
        &format!(r#"[{{"blocker_task_id":"a","blocked_task_id":"b","note":"{long}"}}]"#),
        "more than 200 characters",
    );
    refused(
        r#"[{"blocker_task_id":"a","blocked_task_id":"b","state":"late"}]"#,
        "parse error",
    );
}
