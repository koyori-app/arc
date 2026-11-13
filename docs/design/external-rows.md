# 使う側が図の行を並べる設計書

| 項目 | 値 |
|------|-----|
| 文書ID | `design/external-rows` |
| 関連 | `design/labels-opt-out`（この上に積む）、`design/legend-opt-out`、task の cmd_411（ガントの作り直し） |
| 状態 | 実装 |
| 基準 | `feat/gantt-labels-hidden` の `2eb94fa`（`master` の `f055a51` の上） |

## 1. 要約

使う側が、図の行の並びと高さを決められるようにする。

task のガントは、左に自分の一覧（群れの見出しの行と、課題の行）を描き、右に arc の図を並べる。
左の一覧と右の図の行がずれぬよう、行の並びと高さを使う側が一度だけ組み、両方に同じ物を渡す。
arc は渡された並びのとおりに描き、自分で並べ直さない。

| 問い | 採る答え |
|------|---------|
| 一、行の渡し方 | wasm の三つの入口に、省略できる第四の引数 `rows_json` を足す（3.1） |
| 二、行の形 | 課題の行（`{"kind":"task","task_id":…}`）と群れの行（`{"kind":"group"}`）の並び。どちらも `height` を持てる（3.2） |
| 三、寸法 | `options_json` に `row_height_px`、`header_height_px`、`bar_height_px`、`bar_align` を足す（3.3） |
| 四、並びの決め手 | 一覧がある時は一覧の順だけ。依存で並べ直す `assign_rows` は通さない（4.1） |
| 五、群れの行 | 日の幅いっぱいの帯だけを描き、棒は描かない（4.2） |

既定は一字も変えない。一覧も寸法も渡さぬ図は、今までの図と同じになる（5）。

## 2. 今の作りで確かめたこと

- 行の並びは `layout::assign_rows` が依存から決め、行の高さは `ROW_H`（40）、見出しは `HEADER_H`（30）、棒は `BAR_H`（20）の定数だけで決まる。
- 行の y は `HEADER_H + row * ROW_H` の式で、棒、依存の矢印、進捗の線、行の窓（仮想スクロール）のそれぞれに書かれていた。
- `options_json` は 4 KiB を上限にしている。scalar の鍵を数個持つための上限で、行の一覧は収まらない。

## 3. 口の形

### 3.1 行の渡し方

- wasm: `render_svg`、`render_svg_error`、`render_canvas_commands` に、末尾の `rows_json: Option<String>` を足す。
  JS から三つの引数で呼べば今まで通りで、四つ目を渡した時だけ一覧の図になる。
  `options_json` に入れないのは、その上限（4 KiB）が scalar の鍵のための物で、行の一覧はそれを越えるため。
- Rust: `build_display_list_with_rows(graph, rows, epoch, options)` と、`render_with_rows`、`render_canvas_with_rows` を足す。
  `build_display_list` などは `rows` に `None` を渡す形で残す。`GanttGraph` に欄を足さないので、組む所（33 か所）は変えない。
- arc-vue: prop `rows`。渡した時だけ四つ目の引数を付けて呼ぶ。

### 3.2 行の形

```json
[
  {"kind": "group"},
  {"kind": "task", "task_id": "a"},
  {"kind": "task", "task_id": "b", "height": 64}
]
```

- 一覧に無い課題は描かない（畳んだ群れの課題）。その課題に掛かる依存の矢印も描かない。
- 断る物（`parse_error`）: 読めない一覧、知らない `kind`、課題に無い `task_id`、同じ課題を二度、0 以下か `MAX_SIZE_PX`（1000）を越える `height`。
- 断る物（`input_limit`）: `MAX_ROWS`（課題の上限の二倍）を越える行、`MAX_ROWS_JSON_BYTES` を越える字。

### 3.3 寸法

- `row_height_px`: 行の高さ。既定は `ROW_H`。一覧の行は `height` で上書きできる。
- `header_height_px`: 日付の見出しの高さ。既定は `HEADER_H`。
- `bar_height_px`: 棒の高さ。既定は `BAR_H`。棒が課題の行のどれよりも高い時は断る（棒が上下の行に掛かるため）。省いた時も、既定の棒（`BAR_H`）を課題の行と比べて検める。
- `bar_align`: `"middle"`（既定）は棒を行の真ん中に置く。`"bottom"` は、標準の高さの行で真ん中に置いた時と同じ隙を下に残して、棒を下へ寄せる。高い行では棒の上に場が空き、使う側はそこに題を書ける（スマートフォンの一週の帯の形）。標準より低い行では、その行に収まる分（行の高さから棒の高さを引いた分）までしか隙を取らず、棒は行の中に収まる。
- いずれも 0 を越え `MAX_SIZE_PX` 以下の数でなければ断る。
- 左右の余白: マイルストーンの菱形は、日の位置から棒の高さの半分だけ左右へ張る。棒を高くした図でも菱形が欠けぬよう、次の二つを守る。
  - 左の余白は、少なくとも棒の高さの半分を取る。題を出す図は `LABEL_W` と棒の半分の大きい方、題を消す図は棒の半分（`label_width`）。足さずに大きい方を取るのは、題の列を広げるのを、菱形が欠ける図（棒が `2 * LABEL_W` より高い図）だけに留め、それ以外の図の x を動かさぬため。
  - 図の幅は、全てのマイルストーンの菱形の右端（日の x と棒の高さの半分の和）を覆う（`chart_width_for`）。
  - 既定の棒（`BAR_H`、20）では、左は `LABEL_W`（120）か `HIDDEN_LABELS_LEFT_PX`（10）が、右は右の余白（`CHART_RIGHT_PADDING_PX`、20）が菱形の半分（10）を覆うゆえ、どちらも今の値で足りる。既定の図の出力は変わらず、変わるのは棒を高くした図だけである。

## 4. 描き方

### 4.1 並び

- 行の計画（`display_list/rows.rs` の `RowPlan`）が、行ごとの上端と高さを一度だけ決める。棒、依存の矢印、進捗の線、群れの帯、行の窓は、どれもこの計画から y を読む。
- 一覧の無い図は、今までと同じく `assign_rows` の順で、高さは全て `row_height_px`（既定 `ROW_H`）。
- 棒の部品は、今まで通り課題を入れた順に描く。既定の図の出力の順を変えないため。

### 4.2 群れの行

- 群れの行には、題の列の右端から図の右端までの帯（`RectSemantic::GroupRow`、SVG の class は `group-row`、色は見出しの帯と同じ）を描く。
- 進捗の線は群れの行に点を足さない。帯の色を使う側の token から渡す口は、後の変更（色を外から渡す口）で足す。

### 4.3 高さと Canvas の上限

- 図の高さは、見出し、全ての行の高さの和、凡例（出す時）、下の余白。
- Canvas の上限の判定は、行の計画で凡例を出した時の高さを測り、一辺の上限も見る。行が高ければ、課題の数が少なくても断る。

### 4.4 行の窓

- 一覧が無く、行と見出しの高さも既定の図は、今までの `compute_row_window` の式のまま（出力を縛る試験があるため）。
- 一覧のある図と、`row_height_px` か `header_height_px` を渡した図は、既定の `ROW_H` と `HEADER_H` で割る式に合わぬ。見える範囲に掛かる行を各行の上端と下端から探し、前後に `ROW_BUFFER` 行を足す。

## 5. 試験

- `display_list/external_rows_tests.rs`: 既定の順の一覧が既定の図と同じ。一覧の順、群れの帯、描かぬ課題、図の高さ。進捗の線の点。寸法の鍵と行ごとの高さ。下寄せの棒（標準より低い行でも行に収まる）。描かぬ課題への依存。高さの揃わぬ行の窓。一覧なしで行や見出しの高さを替えた図の行の窓。
- `display_list/hidden_labels_tests.rs`: 棒を高くしたマイルストーンが、題を消した図の右の端でも、題を出した図の左の端でも欠けぬこと。
- `display_list/right_edge_tests.rs`: 棒を高くしたマイルストーンの `NN%` が菱形の右に在り、図の幅の中に収まること。
- `tests/rows_json.rs`: 三つの入口の読み方と断り方、寸法の鍵、棒が行より高い時（棒の高さを省いた時も）、行の数の上限、Canvas の上限が行の高さを測ること。
- `tests/options_json.rs`: 棒を高くしたマイルストーンが、SVG と canvas の入口で同じ幅と同じ菱形に描かれ、左右の端が図の中に在ること。
- 既存の試験は、入口の第四の引数に `None` を足しただけで、出力の縛りは変えていない。
- arc-vue: `options.test.ts`、`GanttChart.options.test.ts`（四つ目の引数は一覧を渡した時だけ、高さの取り置き）、`canvasFallback.test.ts`（`chartHeightForRows`）、`rows.wasm.test.ts`（本物の wasm で、取り置いた高さと描いた高さが一致する）。
- Storybook: `ExternalRows` と `ExternalRowsCanvas`。
