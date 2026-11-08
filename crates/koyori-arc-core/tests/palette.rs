//! `options_json` の `palette`。使う側の色で描くための上書き。
//! 設計書 docs/design/palette-opt-in.md の 3 と 4 にあたる。

use koyori_arc_core::{
    render_canvas_commands, render_svg, render_svg_error, ColorId, Palette, CODE_PARSE_ERROR,
};

const TASKS: &str = r#"[{"id":"a","title":"A","progress_pct":50,"start":"2026-06-01","end":"2026-06-08"},{"id":"b","title":"B","progress_pct":10,"start":"2026-06-02","end":"2026-06-05"}]"#;
const ROWS: &str =
    r#"[{"kind":"group"},{"kind":"task","task_id":"a"},{"kind":"task","task_id":"b"}]"#;

fn s(json: &str) -> Option<String> {
    Some(json.to_string())
}

fn canvas_palette(options: Option<String>, rows: Option<String>) -> Vec<(String, String)> {
    let v: serde_json::Value =
        serde_json::from_str(&render_canvas_commands(TASKS, "[]", options, rows)).unwrap();
    v["palette"]["colors"]
        .as_array()
        .unwrap()
        .iter()
        .map(|e| {
            (
                e[0].as_str().unwrap().to_string(),
                e[1].as_str().unwrap().to_string(),
            )
        })
        .collect()
}

#[test]
fn absent_or_empty_palette_draws_the_standard_colours() {
    let plain = render_svg(TASKS, "[]", None, None);
    assert_eq!(render_svg(TASKS, "[]", s(r#"{"palette":{}}"#), None), plain);
    assert_eq!(
        render_svg(TASKS, "[]", s(r#"{"palette":null}"#), None),
        plain
    );
    assert_eq!(
        canvas_palette(None, None),
        canvas_palette(s(r#"{"palette":{}}"#), None)
    );
    // 群れの帯の色は既定の色の表に無く、見出しの帯の色に落ちる
    assert!(canvas_palette(None, s(ROWS))
        .iter()
        .all(|(id, _)| id != "GroupRow"));
    let grouped = render_svg(TASKS, "[]", None, s(ROWS));
    let header = Palette::standard().resolve(ColorId::HeaderBg).to_string();
    assert!(grouped.contains(r#"class="group-row""#));
    assert!(grouped.contains(&format!(r#"fill="{header}""#)));
}

#[test]
fn a_colour_in_the_palette_replaces_the_standard_one() {
    let options =
        s(r##"{"palette":{"HeaderBg":"var(--koyori-color-surface)","TierMid":"#6d28d9"}}"##);
    let svg = render_svg(TASKS, "[]", options.clone(), None);
    assert!(svg.contains(r#"fill="var(--koyori-color-surface)""#));
    assert!(svg.contains(r##"fill="#6d28d9""##));
    let header = Palette::standard().resolve(ColorId::HeaderBg).to_string();
    assert!(!svg.contains(&format!(r#"fill="{header}""#)));
    let colors = canvas_palette(options, None);
    assert!(colors.contains(&("HeaderBg".into(), "var(--koyori-color-surface)".into())));
    assert!(colors.contains(&("TierMid".into(), "#6d28d9".into())));
    // 位置は変えぬ
    assert_eq!(colors.len(), Palette::standard().colors.len());
}

#[test]
fn a_group_row_colour_is_its_own_once_set() {
    let options = s(r#"{"palette":{"GroupRow":"var(--koyori-color-surface-hover)"}}"#);
    let svg = render_svg(TASKS, "[]", options.clone(), s(ROWS));
    let header = Palette::standard().resolve(ColorId::HeaderBg).to_string();
    // 群れの帯だけが変わり、見出しの帯は既定のまま
    assert!(svg.contains(r#"fill="var(--koyori-color-surface-hover)""#));
    assert!(svg.contains(&format!(r#"fill="{header}""#)));
    let colors = canvas_palette(options, s(ROWS));
    assert_eq!(
        colors.last().unwrap(),
        &(
            "GroupRow".to_string(),
            "var(--koyori-color-surface-hover)".to_string()
        )
    );
}

#[test]
fn every_colour_form_on_offer_is_taken() {
    for color in [
        "#abc",
        "#abcd",
        "#a1b2c3",
        "#a1b2c3d4",
        "var(--koyori-color-accent)",
        "var(--x_1)",
        "rgb(10, 20, 30)",
        "rgba(10 20 30 / 50%)",
        "hsl(240deg 50% 50%)",
        "oklch(0.7 0.1 250)",
        "oklab(0.5 -0.1 0.1)",
    ] {
        let options = format!(r#"{{"palette":{{"Grid":"{color}"}}}}"#);
        assert_eq!(
            render_svg_error(TASKS, "[]", s(&options), None),
            None,
            "{color}"
        );
        assert!(
            render_svg(TASKS, "[]", s(&options), None).contains(&format!(r#"stroke="{color}""#))
        );
    }
}

#[test]
fn a_name_or_colour_the_chart_cannot_use_is_refused() {
    let refused = |options: &str, mentions: &str| {
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
    };
    refused(
        r##"{"palette":{"Header":"#fff"}}"##,
        "unknown colour name `Header`",
    );
    refused(r##"{"palette":"#fff"}"##, "palette");
    refused(r#"{"palette":[]}"#, "palette");
    for bad in [
        r#""red""#,
        r##""#12""##,
        r##""#gggggg""##,
        r#""var(--a b)""#,
        r#""var(--)""#,
        r#""url(x)""#,
        r#""rgb(1,2,3) x""#,
        r#""rgb(1;2)""#,
        r#""\"><script>""#,
        r#""""#,
        "1",
    ] {
        refused(&format!(r#"{{"palette":{{"Grid":{bad}}}}}"#), "`Grid`");
    }
    let long = format!(r#"{{"palette":{{"Grid":"var(--{})"}}}}"#, "a".repeat(100));
    refused(&long, "`Grid`");
}

#[test]
fn with_overrides_replaces_in_place_and_adds_new_ids_in_order() {
    let p = Palette::standard().with_overrides(&[
        (ColorId::GroupRow, "#000".to_string()),
        (ColorId::Grid, "#111".to_string()),
        (ColorId::GroupRow, "#222".to_string()),
    ]);
    let standard = Palette::standard();
    assert_eq!(p.colors.len(), standard.colors.len() + 1);
    assert_eq!(p.resolve(ColorId::Grid), "#111");
    assert_eq!(p.resolve(ColorId::GroupRow), "#222");
    for ((id, _), (sid, _)) in p.colors.iter().zip(&standard.colors) {
        assert_eq!(id, sid);
    }
    // 上書きの無い新しい名は、落ち先の色を返す
    assert_eq!(
        standard.resolve(ColorId::GroupRow),
        standard.resolve(ColorId::HeaderBg)
    );
}
