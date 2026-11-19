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

/// SVG の幅（`width="…"`）。
fn svg_width(svg: &str) -> f64 {
    svg.split("width=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap()
        .parse()
        .unwrap()
}

/// 字 `text` の下の帯（`<rect>`）の右端。
fn band_right(svg: &str, text: &str) -> f64 {
    let at = svg
        .find(&format!(">{text}<"))
        .unwrap_or_else(|| panic!("no {text}"));
    let rect = &svg[svg[..at].rfind("<rect ").unwrap()..at];
    let attr = |name: &str| -> f64 {
        rect.split(&format!(" {name}=\""))
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap()
            .parse()
            .unwrap()
    };
    attr("x") + attr("width")
}

#[test]
fn every_point_of_every_styled_edge_stays_inside_the_chart() {
    // 代わりの端に加え、課題の日の範囲より前と後の日を指す行の点を、両側と字の有無で並べる
    let deps = r#"[
        {"blocker_task_id":"a","blocked_task_id":"c","state":"normal"},
        {"blocker_task_id":"x","blocked_task_id":"a","blocker_at":{"edge":"start"}},
        {"blocker_task_id":"b","blocked_task_id":"y","blocked_at":{"edge":"end"}},
        {"blocker_task_id":"p","blocked_task_id":"c","blocker_at":{"row":4,"day":"2026-10-01","label":"OPS-1 10/1"}},
        {"blocker_task_id":"q","blocked_task_id":"b","blocker_at":{"row":0,"day":"2026-10-15"}},
        {"blocker_task_id":"b","blocked_task_id":"r","blocked_at":{"row":0,"day":"2026-12-20","label":"OPS-2 12/20"}},
        {"blocker_task_id":"a","blocked_task_id":"t","blocked_at":{"row":4,"day":"2026-12-31"}},
        {"blocker_task_id":"u","blocked_task_id":"a","blocker_at":{"row":4,"day":"2026-12-10","label":"OPS-3 阻む側の長い字をここまで書き足して右へ伸ばす"}},
        {"blocker_task_id":"c","blocked_task_id":"v","blocked_at":{"row":0,"day":"2026-09-20","label":"OPS-4 9/20"}}
    ]"#;
    // view の無い図と、課題の一部だけを映す view の図
    for options in [
        r#"{"legend":"hidden","row_labels":"hidden"}"#,
        r#"{"legend":"hidden","row_labels":"hidden","view":{"from":"2026-11-04","to":"2026-11-10"}}"#,
    ] {
        let svg = render_svg(TASKS, deps, s(options), s(ROWS));
        let width = svg_width(&svg);
        let paths = svg_paths(&svg);
        assert_eq!(paths.len(), 9, "{options}");
        for (_, _, d) in paths {
            for (x, _) in route(&d) {
                assert!(x >= 0.0 && x <= width, "{options} {width} {d}");
            }
        }
        for label in ["OPS-1 10/1", "OPS-2 12/20", "OPS-4 9/20"] {
            if svg.contains(&format!(">{label}<")) {
                assert!(band_right(&svg, label) <= width, "{options} {label}");
            }
        }
    }
}

#[test]
fn a_row_point_past_every_task_widens_a_chart_without_a_view() {
    // 全課題の終わり（11/13）より後の 12/20 の点も、課題の日と同じく図の日に入る
    let deps = r#"[
        {"blocker_task_id":"b","blocked_task_id":"r","blocked_at":{"row":4,"day":"2026-12-20"}},
        {"blocker_task_id":"a","blocked_task_id":"y","blocked_at":{"edge":"end"}}
    ]"#;
    let svg = svg(deps);
    let paths = svg_paths(&svg);
    assert_eq!(paths.len(), 2);
    let (tx, ty) = *route(&paths[0].2).last().unwrap();
    // 11/2 から 48 日目の左
    assert_eq!((tx, ty), (day_x(48.0), row_mid(4)), "{}", paths[0].2);
    // 図の日（右の端の代わりが引かれる所）も、その日まで延びる
    let (days_right, _) = *route(&paths[1].2).last().unwrap();
    assert!(days_right > tx, "{days_right} {tx}");
    assert!(svg_width(&svg) > tx, "{}", svg_width(&svg));
    // 前の側も同じ。10/1 の点は図の始まりを 10/1 へ寄せ、x は題の列の右に来る
    let before = render_svg(
        TASKS,
        r#"[{"blocker_task_id":"p","blocked_task_id":"c","blocker_at":{"row":4,"day":"2026-10-01"}}]"#,
        s(OPTIONS),
        s(ROWS),
    );
    let route_before = route(&svg_paths(&before)[0].2);
    assert_eq!(route_before[0], (LABEL_W, row_mid(4)), "{route_before:?}");
}

#[test]
fn a_long_label_at_the_blockers_row_point_widens_the_chart() {
    let label = "OPS-3 阻む側の長い字をここまで書き足して右へ伸ばす";
    let deps = format!(
        r#"[{{"blocker_task_id":"u","blocked_task_id":"b","blocker_at":{{"row":4,"day":"2026-11-12","label":"{label}"}}}}]"#
    );
    let svg = svg(&deps);
    assert!(
        band_right(&svg, label) <= svg_width(&svg),
        "{} {}",
        band_right(&svg, label),
        svg_width(&svg)
    );
}

#[test]
fn a_row_point_outside_a_view_stands_in_at_that_edge() {
    // view が映すのは 11/4〜11/10。前の日の点は左の端から、後の日の点は右の端まで、
    // 代わりの端（edge）と同じ道で引く
    let options = r#"{"legend":"hidden","view":{"from":"2026-11-04","to":"2026-11-10"}}"#;
    let deps = r#"[
        {"blocker_task_id":"p","blocked_task_id":"c","blocker_at":{"row":4,"day":"2026-10-01","label":"OPS-1 10/1"}},
        {"blocker_task_id":"c","blocked_task_id":"r","blocked_at":{"row":0,"day":"2026-12-20","label":"OPS-2 12/20"}},
        {"blocker_task_id":"x","blocked_task_id":"c","blocker_at":{"edge":"start"}},
        {"blocker_task_id":"c","blocked_task_id":"y","blocked_at":{"edge":"end"}}
    ]"#;
    let svg = render_svg(TASKS, deps, s(options), s(ROWS));
    let paths = svg_paths(&svg);
    assert_eq!(paths.len(), 4, "{svg}");
    let routes: Vec<Vec<(f64, f64)>> = paths.iter().map(|(_, _, d)| route(d)).collect();
    // 前の日の点は、左の端の代わりと同じ道（c の行に沿う）
    assert_eq!(routes[0], routes[2]);
    assert_eq!(routes[0][0], (LABEL_W, row_mid(3)), "{:?}", routes[0]);
    // 後の日の点は、右の端の代わりと同じ道
    assert_eq!(routes[1], routes[3]);
    let width = svg_width(&svg);
    assert!(routes[1].last().unwrap().0 <= width);
    // 字は端に描く
    assert!(texts(&svg).contains(&"OPS-1 10/1".to_string()));
    assert!(texts(&svg).contains(&"OPS-2 12/20".to_string()));
    assert!(band_right(&svg, "OPS-2 12/20") <= width);
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

#[test]
fn an_edge_whose_ends_land_on_the_same_row_is_not_drawn() {
    // 両端とも同じ畳んだ群れの見出しの行（4）へ引き込む: 同じ行に潰れた線になるので描かぬ
    let deps = r#"[{"blocker_task_id":"x","blocked_task_id":"y","state":"normal","blocker_at":{"row":4,"day":"2026-11-03","label":"DSK-1"},"blocked_at":{"row":4,"day":"2026-11-10","label":"DSK-2"}}]"#;
    let svg = svg(deps);
    assert!(svg_paths(&svg).is_empty(), "{svg}");
    assert!(!texts(&svg).iter().any(|t| t.starts_with("DSK-")));
    let ops = canvas(deps)["ops"].as_array().unwrap().clone();
    assert!(!ops
        .iter()
        .any(|op| op["StrokePath"].is_object() || op["StrokeDashedPath"].is_object()));
    // 別の行へ引く辺は描く（試験が描かぬ図を見ておるだけでないことの確かめ）
    let other = r#"[{"blocker_task_id":"x","blocked_task_id":"y","state":"normal","blocker_at":{"row":0,"day":"2026-11-03"},"blocked_at":{"row":4,"day":"2026-11-10"}}]"#;
    assert_eq!(svg_paths(&self::svg(other)).len(), 1);
}

/// 字 `text` の下の帯（`<rect>`）の左端と右端。
fn band_of(svg: &str, text: &str) -> (f64, f64) {
    let at = svg
        .find(&format!(">{text}<"))
        .unwrap_or_else(|| panic!("no {text}"));
    let rect = &svg[svg[..at].rfind("<rect ").unwrap()..at];
    let attr = |name: &str| -> f64 {
        rect.split(&format!(" {name}=\""))
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap()
            .parse()
            .unwrap()
    };
    (attr("x"), attr("x") + attr("width"))
}

/// 厳守の期限の菱形（`class="hard-deadline"` の `<polygon>`）の左端と右端。一つだけを見る。
fn diamond_of(svg: &str) -> (f64, f64) {
    let tags: Vec<&str> = svg
        .split("<polygon ")
        .skip(1)
        .filter(|t| t.split('>').next().unwrap().contains("hard-deadline"))
        .collect();
    assert_eq!(tags.len(), 1, "diamonds: {}", tags.len());
    let points = tags[0]
        .split("points=\"")
        .nth(1)
        .unwrap()
        .split('"')
        .next()
        .unwrap();
    let xs: Vec<f64> = points
        .split_whitespace()
        .map(|p| p.split(',').next().unwrap().parse().unwrap())
        .collect();
    (
        xs.iter().cloned().fold(f64::INFINITY, f64::min),
        xs.iter().cloned().fold(f64::NEG_INFINITY, f64::max),
    )
}

/// b（11/9〜11/12、棒の右は 11/13 の頭）の行に、厳守の期限と行の添えを持たせた図。
fn chart_with_deadline(hard_deadline: &str, row_note: Option<&str>, deps: &str) -> String {
    let note = row_note.map_or(String::new(), |n| format!(r#","note":"{n}""#));
    let rows = format!(
        r#"[{{"kind":"group"}},{{"kind":"task","task_id":"a"}},{{"kind":"task","task_id":"b","hard_deadline":"{hard_deadline}"{note}}},{{"kind":"task","task_id":"c"}},{{"kind":"group"}}]"#
    );
    render_svg(TASKS, deps, s(OPTIONS), Some(rows))
}

const WAITING: &str =
    r#"[{"blocker_task_id":"a","blocked_task_id":"b","state":"delayed","note":"OPS-9 待ち"}]"#;

#[test]
fn an_edge_note_moves_past_the_hard_deadline_it_would_run_over() {
    // 行の添えは無い。菱形（11/14 の真ん中）は、棒の右から始まる辺の添えの字の幅の内に落ちる
    let svg = chart_with_deadline("2026-11-14", None, WAITING);
    let (dl, dr) = diamond_of(&svg);
    let (bl, br) = band_of(&svg, "OPS-9 待ち");
    assert!(bl >= dr, "band {bl}..{br} runs over the diamond {dl}..{dr}");
    // 図の幅は、退いた後の字の右端まで数える
    assert!(br <= svg_width(&svg), "{br} > {}", svg_width(&svg));
    // SVG と canvas は同じ置き場を描く
    let svg_x: f64 = {
        let at = svg.find(">OPS-9 待ち<").unwrap();
        let tag = &svg[svg[..at].rfind("<text ").unwrap()..at];
        tag.split(" x=\"")
            .nth(1)
            .unwrap()
            .split('"')
            .next()
            .unwrap()
            .parse()
            .unwrap()
    };
    let canvas = canvas_with_deadline("2026-11-14", None, WAITING);
    let canvas_x = canvas["ops"]
        .as_array()
        .unwrap()
        .iter()
        // 欄を持たぬ命令（字だけの命令）は飛ばす
        .filter_map(|op| op.as_object())
        .flat_map(|fields| fields.values())
        .find(|inner| inner["text"] == "OPS-9 待ち")
        .and_then(|inner| inner["x"].as_f64())
        .expect("canvas text op");
    assert_eq!(canvas_x, svg_x);
}

#[test]
fn an_edge_note_after_the_rows_own_note_moves_past_the_diamond_once() {
    // 行の添え（短い）は菱形（11/15）の手前で終わり、続く辺の添えが菱形に掛かる。辺の添えだけが退く
    let svg = chart_with_deadline("2026-11-15", Some("短い"), WAITING);
    let (dl, dr) = diamond_of(&svg);
    let (rl, rr) = band_of(&svg, "短い");
    let (bl, br) = band_of(&svg, "OPS-9 待ち");
    assert!(
        rr <= dl,
        "the row's note {rl}..{rr} moved: diamond {dl}..{dr}"
    );
    assert!(bl >= dr, "band {bl}..{br} runs over the diamond {dl}..{dr}");
    assert!(br <= svg_width(&svg));
    // 行の添えが既に菱形を越えておれば、辺の添えはその後ろに続き、もう一度は退かない
    let long_row_note = "目安を過ぎて遅れておる長い添えの字";
    let svg = chart_with_deadline("2026-11-14", Some(long_row_note), WAITING);
    let (_, dr) = diamond_of(&svg);
    let (rl, rr) = band_of(&svg, long_row_note);
    let (bl, _) = band_of(&svg, "OPS-9 待ち");
    assert!(
        rl >= dr,
        "the row's note {rl} should be past the diamond {dr}"
    );
    assert!(
        bl - rr < 12.0,
        "the edge's note {bl} moved again after the row's note {rr}"
    );
}

#[test]
fn a_chart_without_edge_notes_draws_its_row_notes_where_it_did() {
    // 辺の添えを持たぬ図は、行の添えの置き場が変わらない（菱形を避けるのは行の添えの幅だけ）
    let plain = chart_with_deadline(
        "2026-11-15",
        Some("短い"),
        r#"[{"blocker_task_id":"a","blocked_task_id":"b","state":"delayed"}]"#,
    );
    let none = chart_with_deadline("2026-11-15", Some("短い"), "[]");
    assert_eq!(band_of(&plain, "短い"), band_of(&none, "短い"));
}

fn canvas_with_deadline(
    hard_deadline: &str,
    row_note: Option<&str>,
    deps: &str,
) -> serde_json::Value {
    let note = row_note.map_or(String::new(), |n| format!(r#","note":"{n}""#));
    let rows = format!(
        r#"[{{"kind":"group"}},{{"kind":"task","task_id":"a"}},{{"kind":"task","task_id":"b","hard_deadline":"{hard_deadline}"{note}}},{{"kind":"task","task_id":"c"}},{{"kind":"group"}}]"#
    );
    serde_json::from_str(&render_canvas_commands(TASKS, deps, s(OPTIONS), Some(rows))).unwrap()
}
