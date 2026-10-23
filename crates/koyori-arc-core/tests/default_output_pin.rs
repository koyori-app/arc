//! 既定の出力を、今の値で縛る。
//!
//! 凡例を消せるようにする変更（`docs/design/legend-opt-out.md` の 8）の前に置く試験である。
//! 指定が無い時の出力が一字も変わらないことを、SVG、canvas の JSON、IR の JSON の三つで縛る。
//! 小さい試料は全文を `tests/fixtures/pin/` に置き、大きい試料は長さと FNV-1a 64 の値で縛る。
//! 標準の `DefaultHasher` は版をまたいだ安定を約束しないので使わない。
//!
//! 入口の署名が変わった時に書き換えてよいのは、`entry` の中身だけである。
//! 試料と fixture は書き換えない。
//!
//! fixture を作り直す時は `ARC_PIN_WRITE=1 cargo test --test default_output_pin` を使う。
//! 作り直しは、出力が意図して変わった時に限る。

mod common;

use chrono::{Duration, NaiveDate};
use koyori_arc_core::bench_fixtures::{generate_deps, generate_tasks, DepDensity};
use koyori_arc_core::{GanttDep, GanttTask, ScrollViewport};
use std::fmt::Write as _;
use std::path::PathBuf;

/// 描く呼び出しを包む。署名が変わったら、書き換えるのはここだけ。
mod entry {
    use super::*;

    pub fn svg(
        tasks: &[GanttTask],
        deps: &[GanttDep],
        today: Option<NaiveDate>,
        viewport: Option<ScrollViewport>,
    ) -> String {
        koyori_arc_core::render(
            tasks,
            deps,
            &koyori_arc_core::ChartOptions {
                today,
                scroll_viewport: viewport,
                ..Default::default()
            },
        )
    }

    pub fn canvas_json(
        tasks: &[GanttTask],
        deps: &[GanttDep],
        today: Option<NaiveDate>,
        viewport: Option<ScrollViewport>,
    ) -> String {
        let buffer = koyori_arc_core::render_canvas(
            tasks,
            deps,
            &koyori_arc_core::ChartOptions {
                today,
                scroll_viewport: viewport,
                ..Default::default()
            },
        );
        serde_json::to_string(&buffer).unwrap()
    }

    pub fn ir_json(
        tasks: &[GanttTask],
        deps: &[GanttDep],
        today: Option<NaiveDate>,
        viewport: Option<ScrollViewport>,
    ) -> String {
        let graph = koyori_arc_core::GanttGraph {
            tasks: tasks.to_vec(),
            deps: deps.to_vec(),
        };
        let epoch = tasks.iter().map(|t| t.start).min().unwrap();
        let list = koyori_arc_core::build_display_list(
            &graph,
            epoch,
            &koyori_arc_core::ChartOptions {
                today,
                scroll_viewport: viewport,
                ..Default::default()
            },
        );
        serde_json::to_string(&list).unwrap()
    }

    /// wasm の入口。三つの結果を `(svg, svg_error, canvas)` で返す。
    pub struct Wire<'a> {
        pub tasks_json: &'a str,
        pub deps_json: &'a str,
        pub today_iso: Option<&'a str>,
        pub viewport_json: Option<&'a str>,
        pub tz_offset_minutes: Option<f64>,
    }

    pub fn wire(w: &Wire) -> (String, Option<String>, String) {
        let options = crate::common::options_json(
            w.today_iso.map(str::to_string),
            w.viewport_json.map(str::to_string),
            w.tz_offset_minutes,
        );
        let svg = koyori_arc_core::render_svg(w.tasks_json, w.deps_json, options.clone());
        let err = koyori_arc_core::render_svg_error(w.tasks_json, w.deps_json, options.clone());
        let canvas = koyori_arc_core::render_canvas_commands(w.tasks_json, w.deps_json, options);
        (svg, err, canvas)
    }
}

fn base() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 6, 1).unwrap()
}

fn day(offset: i64) -> NaiveDate {
    base() + Duration::days(offset)
}

fn task(id: &str, title: &str, pct: i16, start: i64, end: Option<i64>) -> GanttTask {
    GanttTask {
        id: id.to_string(),
        title: title.to_string(),
        progress_pct: pct,
        start: day(start),
        end: end.map(day),
    }
}

fn dep(blocker: &str, blocked: &str) -> GanttDep {
    GanttDep {
        blocker_task_id: blocker.to_string(),
        blocked_task_id: blocked.to_string(),
    }
}

struct Sample {
    name: &'static str,
    tasks: Vec<GanttTask>,
    deps: Vec<GanttDep>,
    today: Option<NaiveDate>,
    viewport: Option<ScrollViewport>,
    /// 全文を fixture に置く。置かない試料は長さと FNV-1a 64 の値で縛る。
    full: bool,
}

fn small(
    name: &'static str,
    tasks: Vec<GanttTask>,
    deps: Vec<GanttDep>,
    today: Option<NaiveDate>,
) -> Sample {
    Sample {
        name,
        tasks,
        deps,
        today,
        viewport: None,
        full: true,
    }
}

/// 日数 `days` の図。タスクは一つで、終わりが `days` 日目。
fn span(name: &'static str, days: i64, today: Option<NaiveDate>) -> Sample {
    small(name, vec![task("a", "A", 40, 0, Some(days))], vec![], today)
}

fn samples() -> Vec<Sample> {
    let two = || {
        (
            vec![
                task("task-1", "Design", 100, 0, Some(3)),
                task("task-2", "Build", 50, 3, Some(7)),
            ],
            vec![dep("task-1", "task-2")],
        )
    };
    let mut out = Vec::new();

    let (t, d) = two();
    out.push(small("two_tasks", t, d, None));
    let (t, d) = two();
    out.push(small("two_tasks_today", t, d, Some(day(4))));
    let (t, d) = two();
    out.push(small("today_before_range", t, d, Some(day(-3))));
    let (t, d) = two();
    out.push(small("today_on_last_day", t, d, Some(day(7))));
    let (t, d) = two();
    out.push(small("today_far_after_range", t, d, Some(day(40))));

    out.push(small(
        "milestone",
        vec![
            task("m", "Release", 0, 2, Some(2)),
            task("n", "Next", 30, 2, Some(6)),
        ],
        vec![dep("m", "n")],
        None,
    ));
    out.push(small(
        "five_tiers",
        vec![
            task("t0", "none", 0, 0, Some(4)),
            task("t1", "low", 10, 1, Some(5)),
            task("t2", "mid", 40, 2, Some(7)),
            task("t3", "high", 70, 3, Some(9)),
            task("t4", "done", 100, 4, Some(10)),
        ],
        vec![dep("t0", "t1"), dep("t1", "t2"), dep("t2", "t3")],
        Some(day(5)),
    ));
    out.push(small(
        "no_end",
        vec![
            task("a", "open", 20, 0, None),
            task("b", "open2", 60, 2, None),
        ],
        vec![],
        Some(day(1)),
    ));
    out.push(small(
        "japanese_long_titles",
        vec![
            task(
                "j1",
                "とても長い題名のタスクでどこまで表示されるかを確かめるための題",
                30,
                0,
                Some(5),
            ),
            task(
                "j2",
                "DBスキーマ変更とマイグレーション手順の整備",
                80,
                1,
                Some(8),
            ),
            task("j3", "短い", 100, 2, Some(6)),
        ],
        vec![dep("j1", "j2")],
        Some(day(3)),
    ));
    out.push(small(
        "xml_special_titles",
        vec![
            task("x&1", "<script>\"a\" & 'b'</script>", 10, 0, Some(4)),
            task("x<2", "a & b > c", 90, 1, Some(5)),
        ],
        vec![dep("x&1", "x<2")],
        None,
    ));
    out.push(small(
        "ambiguous_width_titles",
        vec![
            task("a1", "①②③④⑤⑥⑦⑧⑨⑩", 10, 0, Some(4)),
            task("a2", "※要確認", 40, 1, Some(5)),
            task("a3", "→ 次工程", 70, 2, Some(6)),
            task("a4", "■ 設計", 100, 3, Some(7)),
        ],
        vec![],
        None,
    ));

    // 幅の下支えが効く短い図と、その境の前後。
    out.push(span("span_1d", 1, None));
    out.push(span("span_3d_today_inside", 3, Some(day(2))));
    out.push(span("span_3d_today_after", 3, Some(day(5))));
    out.push(span("span_9d_today_day10", 9, Some(day(10))));
    out.push(span("span_9d_today_day11", 9, Some(day(11))));
    out.push(span("span_10d_today_day10", 10, Some(day(10))));
    out.push(span("span_10d_today_day11", 10, Some(day(11))));
    out.push(span("span_11d_today_day12", 11, Some(day(12))));
    out.push(span("span_12d", 12, None));

    // 大きい試料。
    let sparse = generate_tasks(100);
    let sparse_deps = generate_deps(&sparse, DepDensity::Sparse);
    let big_today = Some(NaiveDate::from_ymd_opt(2026, 1, 20).unwrap());
    for (name, vp) in [
        ("sparse_100", None),
        (
            "sparse_100_viewport_top",
            Some(ScrollViewport {
                scroll_y: 0.0,
                client_height: 600.0,
            }),
        ),
        (
            "sparse_100_viewport_middle",
            Some(ScrollViewport {
                scroll_y: 2000.0,
                client_height: 600.0,
            }),
        ),
        (
            "sparse_100_viewport_bottom",
            Some(ScrollViewport {
                scroll_y: 3900.0,
                client_height: 600.0,
            }),
        ),
    ] {
        out.push(Sample {
            name,
            tasks: sparse.clone(),
            deps: sparse_deps.clone(),
            today: big_today,
            viewport: vp,
            full: false,
        });
    }
    let dense = generate_tasks(2000);
    let dense_deps = generate_deps(&dense, DepDensity::Dense);
    out.push(Sample {
        name: "dense_2000",
        tasks: dense,
        deps: dense_deps,
        today: big_today,
        viewport: None,
        full: false,
    });
    out
}

struct WireCase {
    name: &'static str,
    tasks_json: &'static str,
    deps_json: &'static str,
    today_iso: Option<&'static str>,
    viewport_json: Option<&'static str>,
    tz_offset_minutes: Option<f64>,
}

const TWO: &str = r#"[{"id":"task-1","title":"Design","progress_pct":100,"start":"2026-06-01","end":"2026-06-04"},{"id":"task-2","title":"Build","progress_pct":50,"start":"2026-06-04","end":"2026-06-08"}]"#;
const TWO_DEPS: &str = r#"[{"blocker_task_id":"task-1","blocked_task_id":"task-2"}]"#;
const UTC_DATES: &str = r#"[{"id":"t","title":"T","progress_pct":50,"start":"2026-06-01T15:30:00Z","end":"2026-06-08T15:30:00Z"}]"#;
const FIVE: &str = r#"[{"id":"a","title":"A","progress_pct":0,"start":"2026-06-01","end":"2026-06-03"},{"id":"b","title":"B","progress_pct":30,"start":"2026-06-02","end":"2026-06-05"},{"id":"c","title":"C","progress_pct":60,"start":"2026-06-03","end":"2026-06-06"},{"id":"d","title":"D","progress_pct":90,"start":"2026-06-04","end":"2026-06-07"},{"id":"e","title":"E","progress_pct":100,"start":"2026-06-05","end":"2026-06-08"}]"#;

fn wire_cases() -> Vec<WireCase> {
    let case = |name, tasks_json, today_iso, viewport_json, tz_offset_minutes| WireCase {
        name,
        tasks_json,
        deps_json: if tasks_json == TWO { TWO_DEPS } else { "[]" },
        today_iso,
        viewport_json,
        tz_offset_minutes,
    };
    vec![
        case("wire_plain", TWO, None, None, None),
        case("wire_today", TWO, Some("2026-06-04"), None, None),
        case("wire_today_blank", TWO, Some("  "), None, None),
        case("wire_today_empty", TWO, Some(""), None, None),
        case(
            "wire_today_datetime_with_tz",
            TWO,
            Some("2026-06-03T15:30:00Z"),
            None,
            Some(540.0),
        ),
        case(
            "wire_today_datetime_no_tz",
            TWO,
            Some("2026-06-03T15:30:00+09:00"),
            None,
            None,
        ),
        case(
            "wire_datetime_tasks_jst",
            UTC_DATES,
            None,
            None,
            Some(540.0),
        ),
        case("wire_datetime_tasks_utc", UTC_DATES, None, None, Some(0.0)),
        case("wire_datetime_tasks_no_tz", UTC_DATES, None, None, None),
        case(
            "wire_viewport",
            FIVE,
            Some("2026-06-04"),
            Some(r#"{"scroll_y":0.0,"client_height":100.0}"#),
            None,
        ),
        case(
            "wire_viewport_and_tz",
            FIVE,
            Some("2026-06-04T20:00:00Z"),
            Some(r#"{"scroll_y":40.0,"client_height":80.0}"#),
            Some(540.0),
        ),
        case("wire_today_unreadable", TWO, Some("not-a-date"), None, None),
        case("wire_tz_out_of_range", TWO, None, None, Some(2000.0)),
        case("wire_tz_fractional", TWO, None, None, Some(30.5)),
        case("wire_tasks_malformed", "{", None, None, None),
        case("wire_tasks_empty", "[]", None, None, None),
        case("wire_deps_malformed", TWO, None, None, None),
    ]
}

fn fnv1a64(bytes: &[u8]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for b in bytes {
        h ^= u64::from(*b);
        h = h.wrapping_mul(0x0000_0100_0000_01b3);
    }
    h
}

fn pin_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/pin")
}

fn writing() -> bool {
    std::env::var_os("ARC_PIN_WRITE").is_some()
}

/// `(名, 種, 長さ, FNV-1a 64)` の行。全文を置く試料にも、念のため置く。
fn digest_line(name: &str, kind: &str, text: &str) -> String {
    format!(
        "{name} {kind} {} {:016x}\n",
        text.len(),
        fnv1a64(text.as_bytes())
    )
}

#[test]
fn the_default_output_is_pinned() {
    let mut digests = String::new();
    let dir = pin_dir();
    if writing() {
        std::fs::create_dir_all(&dir).unwrap();
    }
    for s in samples() {
        let outputs = [
            ("svg", entry::svg(&s.tasks, &s.deps, s.today, s.viewport)),
            (
                "canvas.json",
                entry::canvas_json(&s.tasks, &s.deps, s.today, s.viewport),
            ),
            (
                "ir.json",
                entry::ir_json(&s.tasks, &s.deps, s.today, s.viewport),
            ),
        ];
        for (kind, text) in outputs {
            digests.push_str(&digest_line(s.name, kind, &text));
            if s.full {
                let path = dir.join(format!("{}.{kind}", s.name));
                if writing() {
                    std::fs::write(&path, &text).unwrap();
                } else {
                    let want = std::fs::read_to_string(&path)
                        .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
                    assert!(
                        text == want,
                        "{}.{kind} drifted from the pinned output",
                        s.name
                    );
                }
            }
        }
    }
    finish_digests("samples.txt", &digests);
}

#[test]
fn the_default_wasm_output_is_pinned() {
    let mut digests = String::new();
    for c in wire_cases() {
        let (svg, err, canvas) = entry::wire(&entry::Wire {
            tasks_json: c.tasks_json,
            deps_json: if c.name == "wire_deps_malformed" {
                "["
            } else {
                c.deps_json
            },
            today_iso: c.today_iso,
            viewport_json: c.viewport_json,
            tz_offset_minutes: c.tz_offset_minutes,
        });
        digests.push_str(&digest_line(c.name, "svg", &svg));
        digests.push_str(&digest_line(
            c.name,
            "svg_error",
            err.as_deref().unwrap_or("<none>"),
        ));
        digests.push_str(&digest_line(c.name, "canvas", &canvas));
    }
    finish_digests("wire.txt", &digests);
}

fn finish_digests(file: &str, digests: &str) {
    let path = pin_dir().join(file);
    if writing() {
        std::fs::write(&path, digests).unwrap();
        return;
    }
    let want = std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
    if digests != want {
        let mut diff = String::new();
        let want_lines: Vec<&str> = want.lines().collect();
        for line in digests.lines() {
            if !want_lines.contains(&line) {
                let _ = writeln!(diff, "now:    {line}");
            }
        }
        for line in &want_lines {
            if !digests.lines().any(|l| l == *line) {
                let _ = writeln!(diff, "pinned: {line}");
            }
        }
        panic!("the default output drifted from {file}:\n{diff}");
    }
}
