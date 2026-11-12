# 題の列と進みの字を使う側が消せるようにする設計書

| 項目 | 値 |
|------|-----|
| 文書ID | `design/labels-opt-out` |
| 関連 | `design/legend-opt-out`（同じ形の旗）、task の cmd_411（ガントの作り直し） |
| 状態 | 実装 |
| 基準 | `master` の `f055a51`（0.1.0-alpha.3） |

## 1. 要約

使う側が、図の左の題の列（`LABEL_W`、120px）と、棒とマイルストーンの `NN%` を消せるようにする。

task のガントは、左に自分の一覧（キー、題、担当、厳守の期限）を描き、右に arc の図を並べる形にする。
その形では、arc が描く題の列は一覧と重なり、`NN%` は一覧の進みの欄と重なる。
凡例を消す旗（`design/legend-opt-out`）と同じ形で、二つの旗を足す。

| 問い | 採る答え |
|------|---------|
| 一、旗の形 | `options_json` に `row_labels` と `percent_labels` を足す。値は `legend` と同じ `"shown"` と `"hidden"`。Rust は `ChartOptions` の二つの欄（型 `LabelMode`）、arc-vue は prop `rowLabels` と `percentLabels`（3） |
| 二、題の列を消した跡 | 詰める。左に最初の日のマイルストーンの菱形が収まる余白（`HIDDEN_LABELS_LEFT_PX`、10）だけを残し、図の全ての x を 110（`LABEL_W - HIDDEN_LABELS_LEFT_PX`）左へ寄せ、幅も 110 詰める。高さは変えない（4.1） |
| 三、当たりの範囲 | 題の左へ伸びていた範囲（`LABEL_HIT_W`）も消し、棒の範囲だけにする（4.2） |
| 四、`NN%` を消した跡 | はみ出す `NN%` が広げていた幅も戻す（4.3） |

既定は一字も変えない。指定が無い時と `"shown"` の時の出力が既定と同じことを試験で縛る（5）。

## 2. 今の master で確かめたこと

- 題の列の幅は `display_list/constants.rs` の `LABEL_W` だけで決まり、`build.rs` の全ての x がこれを足す。
- 題の字は `TextSemantic::RowLabel`、進みの字は `TextSemantic::ProgressPercent` の `TextPrim` として、各タスクの `GroupPrim` に入る。
- タスクの当たりの範囲（`TaskBBox::bbox`）は、題の字の左 `LABEL_HIT_W`（100）から棒の右端まで。
- 幅は `chart_width_for` が、日数の幅、凡例の右端、右へ伸びる札（`NN%` と目盛りの字）の右端の大きいほうで決める。Canvas の上限の判定も同じ関数で測る。
- arc-vue の読み込み中の骨組みは、`RUST_LAYOUT.LABEL_W` の幅の題の欄を描く。

## 3. 旗の形

- Rust: `ChartOptions` に `row_labels: LabelMode` と `percent_labels: LabelMode` を足す。`LabelMode` は `Shown`（既定）と `Hidden`。
- wasm: `options_json` の鍵 `row_labels` と `percent_labels`。値は `"shown"`、`"hidden"`、`null`（省略と同じ）。他の値は `parse_error` で断り、メッセージに鍵の名を入れる。
- arc-vue: prop `rowLabels` と `percentLabels`（`'shown' | 'hidden'`）。省略した時、`options_json` はその鍵を持たない。真偽値にしない訳は `legend` と同じ（省略が `false` になるため）。

二つに分けたのは、題だけを消して `NN%` を残す使い手（自分の一覧に進みの欄を持たぬ使い手）が在り得るため。

## 4. 消した時に動く物

### 4.1 題の列

- 図を組む時の題の列の幅 `label_w` を、`Shown` なら `LABEL_W`、`Hidden` なら `HIDDEN_LABELS_LEFT_PX`（`BAR_H / 2`、10）とし、`LABEL_W` を足していた全ての x を `label_w` に替える。
  見出しの帯、目盛りの線と字、棒、マイルストーン、依存の矢印、今日の線、進捗の線、凡例が 110 左へ寄る。
- `Hidden` で 0 にしないのは、最初の日のマイルストーンの菱形が x = `label_w` を中心に左へ `BAR_H / 2` 伸びるため。0 では菱形の左半分が図の外へ切れる（arc PR #58 の review）。
- 幅は `chart_width_with_label(days, label_w)`（日数 × 30 + `label_w` + 右の余白）と、凡例の右端、札の右端の大きいほう。どの項も `label_w` を足すので、幅もちょうど 110 詰まる。
- `Viewport::label_width` は `HIDDEN_LABELS_LEFT_PX` になる。
- 今日の線は、x が `label_w` 以上で図の幅以内の時だけ描く（今までの `LABEL_W` 以上と同じ決まり）。

### 4.2 当たりの範囲

- 題を描かぬ時は、タスクの当たりの範囲を描いた形の範囲にする。棒は棒の範囲（`bar_bbox`）、マイルストーンは菱形の左の端（中心から `BAR_H / 2` 左）から。
  見えておる菱形の左半分を押しても応えぬことが無いようにする（arc PR #58 の review）。
- 題を出す時も同じ式で左の端を求め、題の左（`LABEL_HIT_W`）と比べて左のほうを取る。題の左は菱形より左にあるので、題を出す図の当たりの範囲は変わらない。
- 題の列の無い図の左は使う側の一覧なので、図が押しを取ってはいけない。

### 4.3 進みの字

- `NN%` の `TextPrim` を描かない。
- 幅を決める札の右端から、`NN%` を外す。目盛りの字の右端は残る。

### 4.4 Canvas の上限の判定

- 幅は、旗を見て実際に描く幅で測る（`chart_width_for` に旗を渡す）。
- 日数の上限（`MAX_CANVAS_DATE_SPAN_DAYS`）は題の列を出す図から逆算したままにする。題の列を消した図は 110（`LABEL_W - HIDDEN_LABELS_LEFT_PX`）狭いので、判定が緩むことは無い。

## 5. 試験

- `display_list/hidden_labels_tests.rs`
  - `"shown"` を明に渡した図が既定の図と同じ。
  - 題を消すと `RowLabel` が無くなり、`NN%` は残る（逆も同じ）。
  - 題を消すと、字を除く全ての部品の x がちょうど `LABEL_W - HIDDEN_LABELS_LEFT_PX` 左へ寄り、幅も同じだけ詰まる。高さは同じ。凡例の有無と進みの字の有無の全ての組で確かめる。
  - 題を消すと当たりの範囲が描いた形の範囲になる（棒は棒の範囲、マイルストーンは菱形の左の端から）。
  - 最初の日のマイルストーンの菱形が、題を消しても図の左の端で切れない。
  - 菱形の左半分を押すと、題を出す時も消す時も、その課題に当たる。
  - 題を出す図の出力は、既定の出力の固定（`tests/default_output_pin.rs`）で一字も変わらない。
  - 進みの字を消すと、描いた部品はどれも図の外へ出ず、幅は日数の幅を下回らない。最後の日にはみ出す `NN%` の図では幅が詰まる。
- `tests/options_json.rs`: 綴りの違う既定、鍵の組み合わせ、読めない値の断り、SVG と canvas の幅の差。
- arc-vue: `options.test.ts`（鍵を書く時と書かぬ時）、`GanttChart.options.test.ts`（SVG と canvas の両方の経路に鍵が届く）、`labelsHidden.wasm.test.ts`（本物の wasm で幅の差と字の有無）。
- Storybook: `LabelsHidden` と `LabelsHiddenCanvas`（凡例も消した、使う側の一覧と並べる時の形）。

## 6. 後の変更へ

task の cmd_411 の案（arc の PR の分け方）では、この後に次を足す。
行を外から渡す口（群れの行、行の高さ、棒の縦の置き所）、色を外から渡す口、日、週、月の目盛りと曜日と土日の地、棒の形と印（厳守の菱形、警告、範囲の外へ続く印）、稲妻線の決まりと出し入れ。
