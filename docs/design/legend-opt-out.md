# 凡例を使う側が消せるようにする設計書

| 項目 | 値 |
|------|-----|
| 文書ID | `design/legend-opt-out` |
| 関連 | PR #46（0.1.0-alpha.1 の配布）、PR #47（canvas の HiDPI）、PR #48（描画結果の誤りと時差）、Issue #22、Issue #28 |
| 状態 | 設計（実装なし） |
| 基準 | `master` の `3f2d7f4`（2026-10-02 の merge）。以下の行番号と件数はすべてこの commit のもの |
| 確かめた日 | 2026-10-22 |

## 1. 要約

使う側が arc の凡例を消し、同じ凡例を自分で描けるようにする。
消す旗だけでは機能にならない。
使う側が自分で描くには、段の色と名、進捗の線の見本が公開の口から取れる必要がある。
今の公開の口からは、これが取れない（5.1）。

決めることは三つある。

| 問い | 採る答え |
|------|---------|
| 一、消した跡の 40px | 詰める。ただし消した時だけで、既定の寸法は動かさない。消した時は高さを 40 詰め、PR #48 の後は幅の下支えも外す。幅は、日数で決まる幅と、右へ伸びる札の右端の大きいほうになる（4） |
| 二、使う側に出す物 | 段の色と名と範囲、進捗の線の色と太さと破線と名を、一つの定義から出す。Rust の関数、wasm の関数、arc-vue の読み込み関数の三つで取れる（5） |
| 三、入口の形 | 位置の引数を、設定の構造体へ置き換える。Rust は `ChartOptions` を取り、wasm は三つの関数とも `(tasks_json, deps_json, options_json)` になる。今ある `today`、`viewport`、`tz_offset_minutes` も設定に移す。`_with` の入口は作らない。Vue は任意の prop `legend` を足す（6） |

既定は一字も変えない。
指定が無い時の出力が今と一字も違わないことを、現状を縛る試験で縛る（8）。
入口の形を変えても、同じ入力は同じ出力になる。

この設計は、0.1.0-alpha 系の間は公開の形の互換性を約束しない前提に立つ（3.2）。
書き換える呼び手は 153 件で、すべて repo の中にある（3.3、6.2）。
この前提のもとでは、入口を二重にする理由が無い。
二重の入口と、旗のたびに増える位置の引数は、どちらも負債として残るので、置き換える形を選んだ（6.3）。

`master` の時点で、GitHub の Release も tag も 0 件で、`@koyori-app/arc` と `@koyori-app/arc-vue` と `koyori-arc-core` は、どれも公開されていない（3.1）。

## 2. 今の master で確かめたこと

この設計を始める時に前提とした五つの見当を、`master` の `3f2d7f4` で確かめ直した。
読んだのは `origin/master` を取得し直した木で、前提は遅れた木で見たものだった。

| 見当 | 結果 | 今の master |
|------|------|-------------|
| 一、凡例は独立した層で `LayerKind::Legend` として組まれる | 合っている | `crates/koyori-arc-core/src/display_list/build.rs` の 422 行の `// Legend layer` から組み、519 行で `LayerKind::Legend` の層を積む。進捗の線の層の後ろに積まれ、`OverlayHints` はどこでも積まれないので、凡例は最後の層である。前提の「426 行あたり」は 422 から 519 の範囲に入る |
| 二、`LEGEND_H = 40.0` が `CHART_CHROME_H` に織り込まれる | 合っている。ただし file の場所が違う | `crates/koyori-arc-core/src/display_list/constants.rs` の 17 行に `LEGEND_H`、28 行に `CHART_CHROME_H = HEADER_H + LEGEND_H + CHART_BOTTOM_PADDING_PX`。前提の `constants.rs` は `display_list` の下にある |
| 三、公開の入口は六つで、引数はすべて位置で並ぶ | 合っている | `lib.rs` が `render`、`render_canvas`、`render_svg`、`render_svg_error`、`render_canvas_commands`、`build_display_list` を出す。ほかに引数のない `empty_svg_markup` と `empty_svg` も出る |
| 四、wasm の口が `render.rs` に四つ | 合っている | `#[wasm_bindgen]` は crate 全体で四つで、`render_svg`、`render_svg_error`、`empty_svg_markup`、`render_canvas_commands` |
| 五、Vue の props は五つ | 合っている | `packages/arc-vue/src/GanttChart.vue` の `tasks`、`deps`、`today`、`backend`、`deviceTier`。event は `taskClick` の一つ |

前提と違った所と、前提に無かった所を挙げる。

- **`COLOR_TIER_LOW` などの `pub const` は無い。**
  段の色は `crates/koyori-arc-core/src/display_list/types.rs` の `Palette::standard()` が持つ（`ColorId` から 16 進の色の字への対応）。
- **`lib.rs` が出す定数は三つで合っている。**
  `DOM_CAP`、`HEADER_H`、`ROW_H` だけである。
- **色の型は Rust からは取れる。**
  `ColorId`、`Palette`、`ProgressTier` は `lib.rs` から出ているので、Rust の呼び手は `Palette::standard().resolve(ColorId::TierLow)` で色を引ける。
- **段の名は取れない。**
  「未達」「低」「中」「高」「完了」は `build.rs` の中の文字列で、どこからも出ていない。
- **wasm からは、色さえ canvas の経路でしか取れない。**
  `render_canvas_commands` が返す `CommandBuffer` の `palette` に色が入る。
  `render_svg` が返す SVG には、色が属性に埋まるだけである。
- **SPEC の行が違う。**
  凡例の記述は 114 行ではなく 124 行にある。

凡例の高さを知っている箇所は、`master` で次の五つである。
どれかを漏らすと、高さの計算が食い違う。

| 箇所 | 何をしているか |
|------|----------------|
| `build.rs` の 62 行 | `chart_height(rows.len())` で図の高さを決める |
| `build.rs` の 423 行 | `chart_h - LEGEND_H + 14.0` で凡例の y を決める |
| `render.rs` の 37 行 | `MAX_CANVAS_ROWS` を `CHART_CHROME_H` から求める |
| `render.rs` の 169 行 | canvas の面積の上限を `chart_height` で測る |
| `packages/arc-vue/src/canvasFallback.ts` の 12 行 | wasm が答える前に確保する高さを、`RUST_LAYOUT.LEGEND_H` を含めて足す |

## 3. 公開の事実と互換性の前提

### 3.1 確かめた事実

2026-10-22 に、認証なしで次を取った。

| 取った物 | 結果 |
|----------|------|
| `https://api.github.com/repos/koyori-app/arc/releases` | HTTP 200、本文 `[]` |
| `https://api.github.com/repos/koyori-app/arc/tags` | HTTP 200、本文 `[]` |
| `https://registry.npmjs.org/@koyori-app%2Farc` | HTTP 404、本文 `{"error":"Not found"}` |
| `https://registry.npmjs.org/@koyori-app%2Farc-vue` | HTTP 404、本文 `{"error":"Not found"}` |
| `https://crates.io/api/v1/crates/koyori-arc-core` | HTTP 404、本文に `crate \`koyori-arc-core\` does not exist` |

`packages/arc-vue/package.json` の version は `0.1.0` である。
`.github/workflows/release.yml` が npm へ出すのは `@koyori-app/arc`（core）だけで、arc-vue を出す段は無い。
arc-vue の publish は Issue #22 が追っており、Trusted Publisher の登録を待っている。

### 3.2 互換性の前提

この設計は、0.1.0-alpha 系の間は、公開の形の互換性を約束しない前提に立つ。
公開の形とは、Rust の署名、wasm の関数、arc-vue の props である。

この前提は持ち主の判断で、repo の中の文書には書かれていない。
根拠を repo の外の物に置かないために、SPEC に書く案を 10.2 に載せた。

この文書の最初の版は、最初の publish の前に署名を変える窓が閉じかけていることを前提にして、入口を二重にする形を選んでいた。
既存の呼び手を壊さないことだけが、その形の理由だった。
前提が変わってその理由が消えたので、6 で選び直した。
選び直した筋は 6.5 に書く。
履歴は commit に残る。

### 3.3 壊れる呼び手はすべて repo の中にある

6.2 の件数 153 は、署名の変わる呼び出しの数である。
内訳は、Rust が Rust の入口を呼ぶ 73、Rust が wasm と同じ名の関数を呼ぶ 60、arc-vue が wasm を呼ぶ 4、scripts が wasm を呼ぶ 16 である。

呼び出しのある file を grep で列挙して確かめた。
12 file で、どれも `crates/koyori-arc-core/`、`packages/arc-vue/`、`scripts/` の下にある。

- `crates/koyori-arc-core/benches/render_pipeline.rs`
- `crates/koyori-arc-core/examples/preview.rs`
- `crates/koyori-arc-core/examples/write_golden_fixtures.rs`
- `crates/koyori-arc-core/src/render.rs`
- `crates/koyori-arc-core/tests/display_list_tests.rs`
- `crates/koyori-arc-core/tests/output_encoding.rs`
- `crates/koyori-arc-core/tests/wasm.rs`
- `packages/arc-vue/src/GanttChart.vue`
- `scripts/bench-canvas-crossover.mjs`
- `scripts/bench-dom.mjs`
- `scripts/bench-wasm-boundary.mjs`
- `scripts/generate-bench-report.mjs`

repo の外の使い手については、次のことが言える。

- 公開された配布物が無いので、npm か crates.io 経由の使い手は居ない（3.1）。
- `gh search code` で `koyori-arc-core`、`@koyori-app/arc`、`@koyori-app/arc-vue` を探したが、`koyori-app/arc` の外の結果は 0 件だった（2026-10-22）。
  ただし code search は private な repo と未索引の code を見ないので、これは外の使い手が居ない証拠ではない。
- git の参照や workspace で取り込む利用側が居るかどうかは、公開の情報からは見えない。
  確かめていない。

この設計は、外の使い手を確かめなくても成り立つように書く。
外に使い手が居ても、alpha の間は壊れてよいという前提だからである。
153 件がすべて repo の中にあることは、書き換えの大きさを測るために使う。

### 3.4 互換性を気にしないことで決まること

- **入口を二重にしない。**
  既存の呼び手を残すための入口は要らない（6）。
- **位置の引数を断つ。**
  旗が増えるたびに位置が増える形を、`today`、`viewport`、`tz_offset_minutes` も含めて改める（6.3、6.4）。
- **未知の鍵を断る。**
  厳しさの理由は互換性ではなく、綴り違いが黙って何も起こさないことを防ぐためである（6.4）。

互換性を気にしないのは、入口の形についてだけである。
出力を黙って変えてよいという意味ではない。
既定の出力は、8 の縛りで今のまま守る。

## 4. 問い一 消した跡の 40px

### 4.1 結論

消した時は 40px を詰める。
詰めるのは、凡例を消すと指定した時だけである。
指定が無い時の寸法と出力は、今と一字も変えない。

### 4.2 懸念が当たるのは、既定を動かす時だけ

最初に挙がった懸念は、詰めると viewBox と図の高さが変わり、既存の像の対照がすべて動くというものだった。
これは、詰めるのを既定にした場合の話である。
凡例を消すのが、使う側が選んだ時だけなら、消さない既存の図は一画素も動かない。
消す図は新しい像で、比べる元の画像が無い。

### 4.3 詰めた時に動く物

`build.rs` で `chart_h` を読む所は、`master` で次の六行だけである。

| 行 | 内容 | 消した時 |
|----|------|----------|
| 62 | `chart_height(rows.len())` | 40 減る |
| 66 | `Viewport.height` | 40 減る |
| 113 | 週の縦線の `y2` | 40 減る |
| 381 | 今日の縦線の `y2` | 40 減る |
| 423 | 凡例の y1 | 凡例ごと無くなる |
| 425 | 凡例の y2 | 凡例ごと無くなる |

行の y は `HEADER_H + row * ROW_H`、バーも矢印も進捗の線も `chart_h` を読まない。
だから、凡例を消して詰めた図は、既定の図から次を除いたものになる。

- 凡例の層。
- 図の高さ（`height` と `viewBox` の第四の数）を 40 減らす。
- 週の縦線と今日の縦線の終わりを 40 縮める。

それより上の画素は、`master` の `3f2d7f4` では既定の図と同じである。
PR #48 の後は、これが成り立つのは、凡例を消した図の幅が既定の図の幅と同じになる図に限る。
それは、日数で決まる幅が幅の下支えを上回り、しかも右へ伸びる札がその幅に収まる図である。
日数で決まる幅が下支えを上回るのは、10 日以上の図である（4.7）。
9 日以下の図では、下支えを外すので、幅も動く（4.7）。
最後の日に終わる一日の棒やマイルストーンがある図では、札の右端が幅を決めるので、幅が動く（4.7）。
その二つのどちらでもない図では、この関係を試験の答えにそのまま使える（8.4）。

### 4.4 空けたまま残す案を採らない理由

残す案は、「像の対照が動かない」利点を持つ。
しかし利点は、詰める案でも既定で同じに得られている（4.2）。
残す案に残る利点は無く、欠点だけが残る。

- 使う側が凡例を図の下に描くと、図の下に 40px の空白が挟まる。
  外から取り除く方法は無い。
  SVG の viewBox はこちらが決めており、使う側が CSS で切るのは壊れやすい。
- canvas では、backing store の面積の上限を空白が食う。

残す案が働く場面は、一つ考えられる。
使う側が、凡例と同じ場所に自分の凡例を重ねて描きたい時である。
この場合の使う側は、空白の寸法と位置を知る必要があり、その公開が別に要る。
しかも、図の下に CSS の padding を足せば、同じ空白は使う側が自分で作れる。
一方、図そのものを詰めることは、使う側が自分ではできない。

### 4.5 両方を選べる形にしない理由

三つ目の値（予約して残す）を足す案を、数えて退けた。

- 設定の値が増える分、試験の組が増える。
  詰める、残す、既定の三通りで、golden と canvas の上限の判定と Vue の高さが三通りになる。
- 残す値でしか得られないものは、上に書いた重ね描きだけで、それは padding で足りる。
- 後から足すのは壊れない。
  `LegendMode` に値を足すのは追加の変更で、今は足さなくてよい。

だから設定は二値（`shown` と `hidden`）にする。
型を `bool` でなく列挙にするのは、後から値が増える余地を残すためで、増やす予定があるからではない。

### 4.6 詰める時の細部

- 詰めるのは `LEGEND_H` の 40px だけである。
  下の余白 `CHART_BOTTOM_PADDING_PX`（10px）は残す。
  これは最後の行の下の余白で、凡例とは別のものである。
- PR #48 が入った後は、図の幅が凡例の右端で下支えされる。
  凡例を消した時は、その下支えも外す。
  凡例が無いので、凡例のために幅を伸ばす理由は無い。
  ただし、幅を伸ばす理由は凡例だけではなかった。
  下支えは、右へ伸びる札(マイルストーンの札、棒が狭い時の進捗の札、最後の日が月曜の時の日付の札)も覆い隠していた。
  下支えを外した図では、これらの札が図の右端から切れる。
  そこで、凡例を消した図の幅は、日数で決まる幅と、描いた札の右端の大きいほうとする。
  下支えが効く短い図と、札が右へ伸びる図では、幅などが動く。
  動く物と、外す理由は 4.7 で数える。
  凡例を出す図の幅は変えない(4.8)。
- canvas の上限の判定（`canvas_graph_limit_error`）は、実際に描く物より緩んではならない。
  厳しすぎる分には構わない。
  高さは、消していても凡例の分を足したままにする。
  消した図は 40px 低いので、判定は最大で 1 行分だけ早く断り、緩むことが無い。
  幅は、実際に描く幅を使う。
  凡例を消した図の幅は、札が右へ伸びる分だけ日数の幅を越えることがある(4.7)。
  日数だけで測ると、日数は上限に収まるのに、描く幅が上限を越える図が通ってしまう。
  例は、541 日のタスクと、その最終日の 100% のマイルストーンである。
  日数の幅は 16,370px で通るが、描く幅は 16,394.435px で、上限の 16,384px を越える。
  判定は `build_display_list` と同じ関数 `chart_width_for(tasks, epoch, legend)` で幅を求める。
  そのため、この判定は設定の `legend` を見る(6.4)。
  初めの設計は「設定を見ない」としたが、それは日数が幅を決める間だけ成り立った。

### 4.7 凡例を消すと短い図の幅が動く

PR #48 の先端 `8d1a5c3` の `build.rs` は、図の幅を次で決める。
(これは凡例を出す図の幅の式で、実装でもそのまま残る。凡例を消した図は、末尾に書く式を使う。)

```rust
let chart_w = chart_width(total_days as f64)
    .max(LABEL_W + LEGEND_LEFT_PAD_PX + legend.width + CHART_RIGHT_PADDING_PX);
```

`total_days` は、タスクの終わりの最大の日数を切り上げた値である。
`chart_width(d)` は `d * PX_PER_DAY + CHART_CHROME_W` で、`PX_PER_DAY` が 30、`CHART_CHROME_W` が `LABEL_W`(120)と `CHART_RIGHT_PADDING_PX`(20)の和なので、`30 * d + 140` になる。

下支えの値は、同じ commit の定数と凡例の文言から次のように出る。

- 進捗の線の見出しは、10px の和文 24 字で 240px。
- 見本の幅 28px と、見本と見出しの間 6px を足して、進捗の線の行は 274px になる。
- 段の行は 171px で、これより狭い。
  だから `legend.width` は 274px。
- 下支えの値は `120 + 8 + 274 + 20` で 422px。

`30 * d + 140` が 422 を下回るのは、`d` が 9 以下の時である。
9 日で 410px(下回る)、10 日で 440px(上回る)。
だから下支えが効くのは 9 日以下の図で、10 日以上の図では効かない。
この数は、見出しの文言や定数が変われば動く。
設計の規則は数でなく「日数で決まる幅が下支えを上回るか」で、数はその時点の例である。

下支えを外すと、9 日以下の図の `Shown` と `Hidden` は、次の四つが違う。
`Hidden` の幅は、日数で決まる幅と、右へ伸びる札の右端の大きいほうである。
札が日数の幅に収まる図では日数で決まる幅になるので、表にはその場合を書く。

| 所 | `Shown` | `Hidden` |
|----|---------|----------|
| ルートの `width` と `viewBox` の第三の数 | 422 | `chart_width(days)` |
| 見出しの帯の幅(`chart_w - LABEL_W`) | 302 | `chart_width(days) - LABEL_W` |
| 日付の範囲の後ろで、広げた幅の内側にある今日の線 | ある | 無い |
| その時の進捗の線の形 | 今日に繋ぐ形 | 今日が無い時の旧い形 |

今日の線は、`today_x = LABEL_W + (今日 - epoch の日数) * PX_PER_DAY` を求め、`x >= LABEL_W && x <= chart_w` の時だけ出す。
`today_x` は幅でなく日の目盛りで決まり、幅は出すかどうかの検めにだけ効く。
進捗の線は `progress_line(.., today_x)` で作るので、今日の線が出ない時は `None` の旧い形になる。
今日が日付の範囲の中にある時(今日の日数が `days` 以下)は、`x` が `120 + 30 * days` 以下で、`Hidden` の幅 `30 * days + 140` の内側に収まる。
だから `Shown` と `Hidden` で同じ今日の線が出る。
差が出るのは、今日の日数が `days` より大きく、10 以下の時だけである。
`Shown` では `x` が 422 以内で線が出る。
`Hidden` では、`x` が `150 + 30 * days` 以上になり、幅 `30 * days + 140` を越えるので出ない。

**今日の印が消えうる。**
使う側は「凡例を消しただけ」のつもりなので、これは明に書く。
消えるのは、最後のタスクの終わりより後の今日の線に限る。
それを許す筋は、源から次のように立つ。

- 週の縦線と日付の見出しは、`while (grid_day - epoch).num_days() <= total_days` の間しか引かれない。
  差が出る今日は、日付の目盛りが終わった後にあり、元より日付の範囲の外にある。
- 凡例のために広げた幅が今日まで届いた時だけ、その線が出ていた。
  日数が `days` の図で今日が 11 日目なら、`Shown` でも出ない。
  出るかどうかは、下支えの値と今日の位置が偶然に合うかで決まっていた。
- 10 日以上の図では、範囲の後ろ 1 日以上の今日は、`Shown` でも既に出ない(`x` が `150 + 30 * days` で、幅 `30 * days + 140` を越える)。
  `Hidden` の短い図は、これと同じ振る舞いになる。

**札が右へ伸びる図では、`Hidden` の幅は日数で決まる幅にならない。**
日数 0 のマイルストーン一つの図は、日数で決まる幅が 140px しかない。
100% の札は、菱形の右に 6px 空けて置かれ、約 28px の幅を持つので、右端が 162px 近くになる。
`Hidden` の幅は、この右端まで伸びる。
最後の日に終わる一日の棒も、棒が札より狭いので、札が棒の右に置かれ、右端が幅を越える。
右の余白は 20px しかなく、札の幅はそれより広い。
この伸びは凡例とは関係が無く、下支えが効く 9 日以下の図に限らない。
10 日以上の図でも、最後に終わるタスクが一日の棒かマイルストーンなら、`Hidden` の幅は伸びる。
最後の日が月曜の図では、日付の目盛りの札(`6/15` で約 20.7px)が余白を 0.7px 越える。
伸びた幅の内側にある今日の線は、`Hidden` でも出る。

幅を決める式は、`Hidden` の時に限り次の通りとする。

```rust
let chart_w = chart_width(total_days as f64)
    .max(labels_right_edge(graph, epoch, total_days));
```

`labels_right_edge` は、全てのタスクの進捗の札と、日付の目盛りの札の右端の最大である。
窓の外の行も数えるので、スクロールで幅は変わらない。
描く側と同じ `percent_label` と `grid_days` を読むので、幅と描いた物が食い違わない。

札の幅は、字数を数える見積もり `text_width_px` に、太字の分として 1.1 を掛けて求める。
`str::len()` のバイト数は使わない。
`100%` は 25.85px が 28.4px になり、Arial Bold の約 28.1px を下回らない。
DejaVu Sans Bold のような広い字体では約 34px になり、この見積もりを越える。
表は実際の字体では確かめられないので、これは見積もりの限界である。
`progress_label_style` の `label.len() * 6.5` は、札を棒の内に置くか外に置くかの判定だけに使う。
この判定を変えると既定の図の札の位置が動くので、変えていない。

**下支えを残す道(b)を採らない理由。**
`Hidden` でも下支えを残せば、4.3 の関係と 8.4 の byte の一致が、すべての図で成り立つ。
今日の線も、`Shown` と `Hidden` で必ず同じになる。
試験は単純になる。
しかし次の欠点が上回ると判じた。

- 下支えは、凡例が右端から出ないように PR #48 が足したものである。
  凡例の無い図には、守る物が無い。
- 使う側の凡例は SVG の外に描かれるので、残した余白は使われない。
- 最も短い図(日数 0)の自然な幅は 140px だが、`Hidden` が 422px のままだと、282px の空白が残る。
  凡例を消したのに幅が縮まず、幅を決めた枠に入れる使い方で効く。
- 凡例を消す理由の一つは、図の寸法を使う側の都合に寄せることである。
  高さを詰めて幅を詰めないのは、この目的に一貫しない。

(b) の利点は認める。
(a) の費用は、短い図で試験の答えが `Shown` から導けず、4.3 の関係が 10 日以上に限られることと、今日の線が消えうることである。
この費用は 8.4 の試験で縛れるので払う。
(b) へ倒す道は 11.2 に挙げる。

### 4.8 凡例を出す図にも、札が図の外へ出る入力がある

`Shown` では、札が viewBox の外へ出る入力がある。
札は三箇所で、マイルストーンの札と、棒が狭い時の進捗の札と、最後の日が月曜の日付の目盛りの札である。
測った 1722 通りの図(日数 0 から 40、開始日を 7 通り)のうち、1036 通り(約 6 割)で、`Shown` の幅が札の右端に届かない。
この 1722 通りは、最後の日に終わる一日の棒やマイルストーンを多く含むので、実際の入力の割合とは限らない。
`Shown` の幅は、一日の棒が最後に終わる図で 12.4px、マイルストーンが最後の日にある図で 24.4px、最後の日が月曜の図で 0.7px から 2.7px、札の右端に届かない。
9 日以下の図では、凡例の下支えが札を隠すので、届かない量が小さいか、無い。

これは凡例とは別の、元からある瑕疵である。
PR #48 の `master` に既にある。
待っても悪化しない。

この設計は直さない。
直すと `Shown` の幅が動き、「指定が無い時の出力は今と寸分違わぬ」(8)という約束が破れる。
`default_output_pin` の試料は、その入力を覆っていないので、動かないまま約束だけが破れる。
それは偽の証になる。

別の変更で直し、その時に `default_output_pin` の試料と golden を、意図して焼き直す。
今回の走査の試験(`right_edge_tests`)と `labels_right_edge` は、そのままその変更で使える。
`Shown` に札の右端の下支えを足せば、`Hidden` と同じ式になる。
`Shown` が溢れることは、`right_edge_tests` の `shown_is_known_to_overflow_until_it_is_fixed_in_its_own_change` が記録している。
溢れが消えるとこの試験が落ちるので、直した時に気づける。

## 5. 問い二 使う側に出す物

### 5.1 今の公開の口で取れる物

| 要る物 | Rust | wasm と JS |
|--------|------|-----------|
| 段の色 | `Palette::standard()` で取れる | canvas の `CommandBuffer.palette` に入るが、SVG の経路では取れない |
| 段の名 | 取れない（`build.rs` の文字列） | 取れない |
| 段の範囲（1 から 33 など） | `ProgressTier::from_pct` を走らせれば導ける | 取れない |
| 進捗の線の色 | `Palette` で取れる | canvas の経路だけ |
| 進捗の線の太さと破線 | 取れない（`PROGRESS_LINE_STROKE_W` と `PROGRESS_LINE_DASH` は `constants.rs` の中で、`lib.rs` が出さない） | 取れない |
| 進捗の線の名 | 取れない（文字列） | 取れない |

つまり、消す旗だけを入れると、使う側が同じ凡例を描ける材料は、Rust で引ける色と段の範囲のほかに何も無い。
wasm と Vue の使い手には、段の名も破線の指定も渡らない。

### 5.2 出す物

凡例の中身を一つの定義にして、組み立てる側と使う側がそれを読む。

```rust
pub struct LegendSpec {
    pub tiers: Vec<LegendTier>,
    pub progress_line: LegendProgressLine,
}

pub struct LegendTier {
    pub key: &'static str,      // "none" | "low" | "mid" | "high" | "done"
    pub label: String,          // 「未達」「低」「中」「高」「完了」
    pub color: String,          // 16 進の色
    pub min_pct: i16,
    pub max_pct: i16,
}

pub struct LegendProgressLine {
    pub label: String,
    pub color: String,
    pub stroke_width: f64,
    pub dash: String,           // SVG の stroke-dasharray の字
}

pub fn legend_spec() -> LegendSpec;
```

今の値で作ると、次の JSON になる。

```json
{
  "tiers": [
    { "key": "none", "label": "未達", "color": "#d1d5db", "min_pct": 0,   "max_pct": 0 },
    { "key": "low",  "label": "低",   "color": "#f59e0b", "min_pct": 1,   "max_pct": 33 },
    { "key": "mid",  "label": "中",   "color": "#6366f1", "min_pct": 34,  "max_pct": 66 },
    { "key": "high", "label": "高",   "color": "#0ea5e9", "min_pct": 67,  "max_pct": 99 },
    { "key": "done", "label": "完了", "color": "#22c55e", "min_pct": 100, "max_pct": 100 }
  ],
  "progress_line": {
    "label": "進捗ステータスライン（各タスクの完了位置を結ぶ）",
    "color": "#ef4444",
    "stroke_width": 2,
    "dash": "6,3"
  }
}
```

- `key` は `ProgressTier::css_suffix` の値で、SVG の `bar-tier-low` などの class と同じ名である。
  使う側は、すでに出ている class と同じ名で段を指せる。
- `label` は日本語の文字列のままで出す。
  翻訳が要る使う側は、`key` で引いて自分の字に置き換える。
- 一つの定義から作る。
  `min_pct` と `max_pct` は、`ProgressTier::from_pct` を 0 から 100 まで走らせて導く。
  範囲を別の表に書き写さない。
  色は `Palette::standard()`、太さと破線は `PROGRESS_LINE_STROKE_W` と `PROGRESS_LINE_DASH` を読む。
- 組み立てる側も同じ定義を読む。
  凡例の層の組み立ては、この定義の段の並びを回して swatch と字を作る。
  PR #48 は、この並びを `LEGEND_TIER_ITEMS` として切り出している。
  この設計はその定数を土台にする（9）。
- 凡例の寸法（swatch の大きさ、字の大きさ、間隔）は出さない。
  使う側は自分の配置で描く。
  必要になれば、JSON に鍵を足すのは壊れない追加である。

### 5.3 三つの口

| 層 | 出す口 |
|----|--------|
| Rust | `koyori_arc_core::legend_spec() -> LegendSpec` と、型 `LegendSpec`、`LegendTier`、`LegendProgressLine` |
| wasm | `#[wasm_bindgen] pub fn legend_spec_json() -> String`（引数なし。上の JSON を返す） |
| arc-vue | `loadLegendSpec(): Promise<LegendSpec>` と型 `LegendSpec` の re-export。wasm の初期化を済ませてから `legend_spec_json()` を呼び、parse して返す |

arc-vue に、`wasmContract.ts` のような書き写した定数を足すことはしない。
`GanttChart.vue` のコメントが述べる通り、arc と arc-vue は別の package で、別の版で入り得る。
書き写しは、版の組み合わせの食い違いに気づけない。
実行時に wasm へ問うのが、正しさを保つ唯一の方法である。

### 5.4 出さないもの

- 凡例を描く Vue の部品（`GanttLegend`）は足さない。
  部品を足すと、描き方（配置、字体、間隔）まで公開の約束になる。
  使う側が自分で描きたいから消すのであり、材料の公開で足りる。
- 今日のマーカーの色は出さない。
  凡例が描いていないので、凡例の材料ではない。

## 6. 問い三 入口の形

### 6.1 並べる形

| 形 | 内容 |
|----|------|
| A | 既存の入口に位置の引数を足す（`hide_legend: bool` など） |
| B | 位置の引数を、設定の構造体を取る入口に置き換える |
| C | 設定の構造体を末尾の引数として足す（位置の引数は残る） |
| D | 設定を取る別の入口を足し、既存の入口は触らない |
| E | Rust は D、wasm は末尾に任意の JSON の字を足す |

この文書の最初の版は E を選んでいた。
今は B を選ぶ。
理由は 6.3 に書く。

### 6.2 書き換える呼び手の数え上げ

件数は 2026-10-22 に `master` の `3f2d7f4` を grep して数えた呼び出し式の数である。
コメントと文字列の中の同名は区別していない。
PR #48 が入ると、同じ呼び出し式を触るので数が動く。

呼び手を、次の五つに分けて数える。

| 呼び手 | 件数 |
|--------|-----:|
| (R1) Rust の呼び手が、署名の変わる Rust の入口を呼ぶ | `build_display_list` 27（example 1、`render.rs` 2、`display_list_tests.rs` 24）、`render` 44（bench 2、example 5、`display_list_tests.rs` 3、`output_encoding.rs` 6、`render.rs` 28）、`render_canvas` 2（`render.rs`）。計 73 |
| (R2) Rust の呼び手が、wasm と同じ名の三つの関数を呼ぶ | `render_svg` 32（bench 1、`render.rs` 15、`output_encoding.rs` 5、`wasm.rs` 11）、`render_svg_error` 6、`render_canvas_commands` 22。計 60 |
| (J1) arc-vue が wasm を呼ぶ | `GanttChart.vue` に 4（`render_svg` 2、`render_svg_error` 1、`render_canvas_commands` 1） |
| (J2) scripts が wasm を呼ぶ | `scripts/*.mjs` に 16（`render_svg` 10、`render_canvas_commands` 6） |
| (V) arc-vue を使う側 | `GanttChart` の使い手。props を足すだけなら 0 |

どの呼び手も repo の中にある（3.3）。
互換性を約束しないので、件数は「壊れる数」ではなく「一度書き換える量」として読む。

形ごとの書き換える件数と、形の残す負債を並べる。

| 形 | 書き換える件数 | 描画の入口の数（Rust と wasm の合計） | `render_svg` の位置の引数 | 旗が増えた時 |
|----|---------------:|--------------------------------------:|--------------------------:|--------------|
| A 位置の引数を足す | R1 73 + R2 60 = 133 | 6 | 5（PR #48 の後は 6） | 旗ごとに位置が一つ増える |
| B 構造体に置き換える | R1 73 + R2 60 + J1 4 + J2 16 = 153 | 6 | 3 | 構造体の field が一つ増える |
| C 構造体を末尾に足す | 133 | 6 | 6 | field は増えるが、位置の引数が残る |
| D 別の入口を足す | 0 | 12 | 5（PR #48 の後は 6） | 旗を両方の入口に通す |
| E Rust は D、wasm は末尾に JSON | R2 60 | 9 | 5（PR #48 の後は 6） | JSON の鍵が増える。Rust は両方の入口に通す |

### 6.3 選ぶ形と理由

B を選ぶ。
理由は、どれも互換性とは別のところにある。

- **旗が増えても、入口も位置も増えない。**
  A は旗のたびに位置が増える。
  C は構造体に field を足せるが、位置の引数が残り続ける。
  D と E は旗のたびに二つの入口へ通す。
  B は構造体に field を足すだけである。
- **入口が一組である。**
  二重の入口は、旗を片方にだけ通し忘れると、その旗が片方でだけ効く。
  値を二か所に持つと食い違うことは、`display_list/constants.rs` の冒頭のコメントが同じ形で述べている（「a restated number is a second definition that can drift while every test stays green」）。
  入口の二重もこれと同じ形の負債である。
- **wasm の三つの関数が同じ設定を共有する。**
  `render_svg`、`render_svg_error`、`render_canvas_commands` は同じ `options_json` を同じ関数で読む。
  `render_svg` と `render_svg_error` が食い違わないという今の約束が、検めの場所を一つに保つことで守られる。
- **使う側が途中を埋めない。**
  `render_svg` は今の位置の引数が 4 つで、PR #48 の後は 5 つになる。
  `today` だけ渡したい呼び手は、途中を `undefined` で埋める。
- **互いに依る値を離さない。**
  `tz_offset_minutes` は、`start`、`end`、`today` の字をどの暦の日に読むかを決める。
  別々の位置の引数に置くと、片方だけ渡す誤りが起きる。

B の費用も挙げる。

- **153 件を一度に書き換える。**
  Rust は compiler が全箇所を示す。
  arc-vue は typecheck が示す。
- **`scripts/*.mjs` には型の検めが無い。**
  古い呼び方 `render_svg(tasksJson, depsJson, today)` は、第三引数が日付の字になる。
  これは JSON の object ではないので、`parse_error` で断られる。
  ただし、第三引数が `undefined` で第四引数に viewport を渡す古い呼び方は、余分な引数を wasm-bindgen が捨てるので、黙って窓の無い図になる。
  scripts の 16 件は書き換えの時に全件を見る（8.4）。
- **既定の呼び出しが長くなる。**
  Rust の呼び手は `&ChartOptions::default()` を渡す。

B が悪い理由を、互換性以外の言葉で探した。
定型の増加と、wasm の設定が文字列で型が弱いことの二つが見つかった。
後者は、E の `options_json` も同じで、`viewport_json` が今もそうである。
arc-vue が TypeScript の型 `ChartOptions` を出すことで補う（7.3）。
どちらも B を退ける重さではない。

### 6.4 設定の形

Rust は構造体を取る。

```rust
pub struct ChartOptions {
    pub today: Option<NaiveDate>,
    pub scroll_viewport: Option<ScrollViewport>,
    pub legend: LegendMode,
}
```

wasm は一つの `options_json`（JSON の字）を取る。

```json
{
  "today": "2026-06-16",
  "scroll_viewport": { "scroll_y": 0, "client_height": 600 },
  "tz_offset_minutes": 540,
  "legend": "hidden"
}
```

- 鍵は、`today`（今の `today_iso`）、`scroll_viewport`（今の `viewport_json`。
  JSON の中に JSON の字を入れず object にする）、`tz_offset_minutes`（PR #48 が足す）、`legend`（新しい）の四つである。
- `tz_offset_minutes` は wasm の設定にだけある。
  日時の字を解くのは wasm の入口で、Rust の呼び手は日付を決めてから `today` に渡す（PR #48 の決めと同じ）。
  wasm の入口は、`options_json` を読んで `ChartOptions` に直してから描く。

`options_json` の決め:

| 入力 | 扱い |
|------|------|
| 省略、`undefined`、`null` | 既定。設定が無い時と同じ |
| `"{}"` | 既定 |
| JSON の object でない字（配列、日付の字など） | `parse_error` で断る |
| 未知の鍵 | `parse_error` で断る。メッセージに鍵の名を入れる |
| `today` が読めない | `parse_error` で断る（PR #48 と同じ） |
| `tz_offset_minutes` が整数でない、または絶対値が 1439 を超える | `parse_error` で断る（PR #48 と同じ） |
| `scroll_viewport` が `{scroll_y, client_height}` の形でない | `parse_error` で断る（8.5） |
| `legend` が `"shown"` か `"hidden"` 以外 | `parse_error` で断る |
| 長さが上限（4 KiB）を超える | `input_limit` で断る |

未知の鍵を断るのは、綴り違い（`lengend`）が黙って何も起こさないことを防ぐためである。
arc と arc-vue の版が食い違った時に、新しい arc-vue が古い arc へ未知の鍵を送っても、黙って凡例が出続ける代わりに、図の上の警告で気づける。
`render_svg` は断る時に空の SVG を返し、理由は `render_svg_error` が返す。
この二つは同じ検めを共有する（`prepare_svg_input`）。

canvas の上限は、`options_json` の `legend` を見て判定する（4.6）。
幅は描く幅(`chart_width_for`)で測り、高さは凡例のある図の高さで測る。
`legend` が `shown` の図の判定は、これまでと変わらない（4.6）。

### 6.5 E を選ばなかった理由

E の理由は、既存の呼び手を壊さないことだけだった。
互換性を約束しない前提で、その理由は消えた。
残るのは、入口が二重になる負債である。

- Rust の描画の入口が 3 つから 6 つになり、旗ごとに両方へ通す。
- wasm は `today`、`viewport`、`tz_offset_minutes` が位置の引数のまま残り、`options_json` と二系統になる。
- 一つの意味の値を、位置の引数と JSON の鍵の二か所で受けるので、どちらが優先かを決める必要が出る。

B はこの三つをすべて避ける。

## 7. 三層の変更

### 7.1 Rust

- `display_list/constants.rs` に `chart_chrome_h(legend)` と `chart_height_with(rows, legend)` を足す。
  `CHART_CHROME_H` と `chart_height` は今のまま残す（`LegendMode::Shown` の値）。
- `display_list/types.rs` に次を足し、`lib.rs` から出す。

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LegendMode {
    #[default]
    Shown,
    Hidden,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChartOptions {
    pub today: Option<NaiveDate>,
    pub scroll_viewport: Option<ScrollViewport>,
    pub legend: LegendMode,
}
```

- 三つの入口の署名を置き換える。
  `_with` は作らない。

```rust
pub fn build_display_list(graph: &GanttGraph, epoch: NaiveDate, options: &ChartOptions) -> DisplayList;
pub fn render(tasks: &[GanttTask], deps: &[GanttDep], options: &ChartOptions) -> String;
pub fn render_canvas(tasks: &[GanttTask], deps: &[GanttDep], options: &ChartOptions) -> CommandBuffer;
```

- `ChartOptions` に `#[non_exhaustive]` は付けない。
  互換性を約束しないので、外の呼び手は `ChartOptions { legend: LegendMode::Hidden, ..Default::default() }` で素直に組める。
  field を足すと、全 field を列挙して組んでいた呼び手は壊れる。
  alpha の間はそれを許す。
- `build.rs` は、`Hidden` の時に `chart_h` を `chart_height_with` で求め、凡例の層を積まない。
  PR #48 の後は、図の幅の下支えも外し、幅を日数で決まる幅と札の右端の大きいほうとする(4.7)。
  `Shown` の幅は変えない(4.8)。
  今日の線を出すかの検めには、下支えを外した後の幅を使う。
- `legend_spec()` を足す。
  凡例の層の組み立てと、この関数が、同じ並びを読む。
- `Hidden` の時の `metadata.primitive_count` は、凡例の分を数えない。

### 7.2 wasm

- `render_svg`、`render_svg_error`、`render_canvas_commands` の署名を、`(tasks_json, deps_json, options_json)` に置き換える。
  `options_json` は `Option<String>` で、省略できる。
- 今の `today_iso`、`viewport_json` と、PR #48 が足す `tz_offset_minutes` は、位置の引数から `options_json` の鍵に移る（6.4）。
- 入口は、非公開の型 `WireOptions`（`#[serde(deny_unknown_fields)]`）で `options_json` を読み、日時の字を解いて `ChartOptions` に直す。
  三つの関数が同じ関数でこれを行う。
- `legend_spec_json() -> String` を足す。
- `canvas_graph_limit_error` は `legend` を受け取り、幅を描く幅で測る(4.6)。
  `legend` が `shown` の図の判定は変えない。

### 7.3 Vue

- props に `legend?: 'shown' | 'hidden'` を足す。
- 設定を一つの関数 `buildOptionsJson` で組む。
  `today`、`scroll_viewport`、`tz_offset_minutes`（PR #48 が足す prop `tzOffsetMinutes`）、`legend` のうち、値のある鍵だけを持つ object を `JSON.stringify` する。
  wasm を呼ぶ四か所（`svgHtml`、`canvasCommandsJson`、`svgFailure`、`renderCanvasFallback`）のすべてがこれを通す。
  `renderCanvasFallback` は `scroll_viewport` だけを上書きする（高さを 600 に切る今の動きのまま）。
  四つ目を漏らすと、canvas が上限で SVG へ落ちた時に凡例が戻る。
- `legend` は boolean の prop にしない。
  Vue は、省略された boolean の prop を `false` に変える。
  `showLegend` を既定なしで宣言すると、既存の使い手の凡例が黙って消える。
  列挙の prop は、省略が `undefined` のまま残り、`buildOptionsJson` が `legend` の鍵を出さない。
- `chartHeightForTaskCount(taskCount, legend = 'shown')` の第二引数を足す。
  `legend` が `hidden` の時は `RUST_LAYOUT.LEGEND_H` を引く。
  `wasmContract.ts` の `RUST_LAYOUT` は変えない。
  Vue は図の幅を写していない。
  canvas の幅は wasm の戻り値の `viewport_width` を読むので、幅の下支えを外しても Vue は変わらない。
- `index.ts` から、型 `ChartOptions`（`options_json` の object の型）、`loadLegendSpec`、型 `LegendSpec` を出す。
- `GanttChart.stories.ts` に `LegendHidden`（SVG と canvas）の story を足す。
  既存の story は変えない。

## 8. 既定が一字も変わらないことの縛り

互換性を気にしないのは入口の形についてだけで、出力を黙って変えてよいという意味ではない。
入口の形が変わっても、同じ入力が同じ出力になることを縛る。

### 8.1 縛る対象

指定が無い時に、次の三つが今と一致する。

- Rust: `ChartOptions::default()` を渡した `render`、`render_canvas`、`build_display_list` の戻り値。
- wasm: 設定が省略、`null`、`"{}"`、`{"legend":"shown"}` の時の `render_svg`、`render_canvas_commands` の戻り値。
- Vue: `legend` を省略した時に wasm へ渡る設定の object に、`legend` の鍵が無いこと。

今まで位置の引数で渡していた値(`today`、`viewport`、`tz_offset_minutes`)は、設定の鍵に移った後も、同じ値が同じ出力になる。

### 8.2 今の golden では足りない

`crates/koyori-arc-core/tests/fixtures` の golden は、小さい図だけを byte で縛る。
SVG は `two_tasks.svg`、`two_tasks_today.svg`、`milestone.svg`、`empty.svg`、IR と canvas は `two_tasks.json` の一枚ずつである。
`display_list_tests.rs` の `p0_ir_golden_100_sparse` と `p0_ir_golden_2000_dense` は、二度組んで同じになることと個数を見るだけで、中身は縛らない。
つまり、100 タスクや 2000 タスク、仮想化の窓、日本語の長い題、五つの段のすべてがある図は、byte では縛られていない。

### 8.3 縛り方

1. **最初の commit で、現状を縛る試験だけを足す。**
   実装に入る前の `master`(PR #48 の後のもの)で通る試験である。
   - 試料は次を含む: 既存の golden の試料、五つの段がすべて出る図、`end` が無い図、今日が範囲の内と外にある図、9 日以下の図(幅の下支えが効く図)、その図で日付の範囲の後ろにある今日(たとえば 3 日の図の 5 日目、9 日の図の 10 日目)、10 日の図の 11 日目の今日(`Shown` でも出ない境界)、仮想化の窓が上と中と下にある図、100 タスクの疎、2000 タスクの密、日本語の長い題、XML の特殊文字を含む題。
   - 各試料で SVG、canvas の JSON、IR の JSON の三つを縛る。
     小さい試料は全文を fixture に置く。
     大きい試料は、長さと FNV-1a 64 の値で縛る。
     標準の `DefaultHasher` は版をまたいだ安定を約束しないので使わない。
     FNV-1a は依存を足さずに書ける。
   - 試験は、描く呼び出しを一つの小さな関数(たとえば `render_pinned(tasks, deps, today, viewport)`)に包んで呼ぶ。
     入口の署名が変わった時に書き換えるのは、この関数の中身だけにするためである。
2. **以後の commit は、この fixture を一字も変えない。**
   `git diff --diff-filter=MD --name-only <縛りの commit>..HEAD -- crates/koyori-arc-core/tests/fixtures` が空であること。
   新しい fixture の追加は許す。
   試験の側で書き換えてよいのは、1 の包む関数の中身だけである。
3. **設定の移動を、縛った値で確かめる。**
   署名の置き換え(B)は、凡例の機能を足す前に、それだけで一つの commit にする。
   この commit の前後で、1 の試験が一字も動かないことが、設定の移動が出力を変えていない証拠になる。
4. **既定の設定が縛った値と一致することを縛る。**
   `ChartOptions::default()` と `legend: Shown` を明示した場合の戻り値が、1 で縛った値と一致すること。
   wasm では、設定が省略、`null`、`"{}"`、`{"legend":"shown"}` の四通りで、`render_svg` と `render_canvas_commands` の戻り値が一致すること。
5. **Vue で、設定の object を縛る。**
   `GanttChart.virtualization.test.ts` が既に wasm の関数を mock して引数を見ている。
   同じ型で、wasm に渡った `options_json` を parse し、次を確かめる。
   - `legend` を省略した時に、`legend` の鍵が無いこと。
   - `today`、`scroll_viewport`、`tz_offset_minutes` が、props の値と同じ値で入ること。
   - 呼び出しが `(tasks_json, deps_json, options_json)` の三つの引数であること。
   `chartHeightForTaskCount(n)` の既存の試験(408 で 16 400)はそのまま残す。
6. **像で縛る。**
   Chromatic と VRT の既存の story が、差分 0 であること。

### 8.4 消した時と設定の試験

10 日以上の図で、右へ伸びる札が日数の幅に収まる図では、消した時の出力は、既定の出力から機械的に導ける(4.3)。
これを試験の答えにする。
9 日以下の図と、札が日数の幅を越える図は、幅が動くので導けない。
その図は、4.7 の性質で縛る。

- 10 日以上の図で、`Hidden` の SVG は、`Shown` の SVG から、凡例の二つの `<g>` を取り除き、ルートの `height` と `viewBox` の第四の数と、週の縦線と今日の縦線の `y2` を 40 減らしたものに、byte で一致する。
- 10 日以上の図で、`Hidden` の canvas の JSON と IR も、同じ操作で `Shown` から導く。
- 9 日以下の図の `Hidden` は、次の性質を縛る。
  - `viewport.width` が、`chart_width(days)` と札の右端の大きいほうであり、ルートの `width` と `viewBox` の第三の数がそれと一致する。
  - 見出しの帯の幅が、その幅から `LABEL_W` を引いた値である。
  - 今日の日数が `days` より大きく 10 以下で、`Hidden` の幅の外にある時、今日の線の層が無い(`Shown` には有る)。
  - その時、進捗の線の点列が、今日を渡さない `progress_line(.., None)` の点列と一致する。
  - 今日の日数が `days` 以下の時は、`Shown` と `Hidden` で今日の線の `x` が一致する。
- `Hidden` の図の全ての部品の右端が、`viewport.width` を超えない。
  図の部品を全て走査し、部品の種類を網羅して縛る。
  右へ伸びる新しい種類の部品を足すと、右端の求め方を決めるまでコンパイルが通らない。
- 開始と終了が同じマイルストーン一つだけで `Hidden` の時、札の全体が `viewport.width` に収まる。
  最後の日に終わる一日の棒も同じく縛る。
- `Shown` の幅が、日数で決まる幅と下支えの大きいほうのままであること。
  札が溢れる図も含めて縛る(4.8)。
- `Shown` が溢れることを、「今は溢れる」と記録する試験を置く。溢れが消えると落ちる。
- 境界として、9 日と 10 日の図を試す。
  10 日では `Shown` と `Hidden` の幅がどちらも 440 で一致し、9 日では 422 と 410 に分かれる。
  この数は、PR #48 の先端の定数のものである。
- `Hidden` で、凡例の層が無いこと、`primitive_count` が凡例の分だけ減ること。
- `Hidden` の `viewport.height` は `chart_height(rows) - LEGEND_H`。
- 上限の判定が、実際に描く図より緩まないこと。
  上限の際(日数の上限とその前後)に、最終日のマイルストーンと一日の棒を置いて、通る図と断られる図の両側を縛る。
  境の日数は定数から導き、数を決め打たない。
  行数を変えた面積の上限でも同じに縛る。
- `Shown` の図の上限の判定が、札によらず日数で決まること(4.6)。
- 不正な `options_json` が `parse_error` になること。
  対象は、object でない字、未知の鍵、読めない `today`、範囲外の `tz_offset_minutes`、形の違う `scroll_viewport`、知らない `legend` の値である。
  `render_svg` が空の SVG を返し、`render_svg_error` が同じ理由を返すこと。
- **古い呼び方が、日付の字を渡す形では断られること。**
  `options_json` の位置に日付の字(`"2026-06-16"`)が来たら、`parse_error` で断ること。
  型の検めが無い `scripts/*.mjs` の書き換え漏れを、実行時に見つけるための試験である。
  第三引数が `undefined` で、第四引数に viewport の字が来る古い呼び方は、余分な引数を wasm-bindgen が捨てるので検出できない。
  そのため、`scripts/*.mjs` の 16 件は、書き換えの時に全件を目で確かめる。
  今日と viewport を両方渡す `bench-dom.mjs` の呼び出しは、特に確かめる。
- `legend_spec()` が、`Shown` の図の凡例の層から読み取った字と色と一致すること。
  凡例が描く物と、使う側へ出す物が食い違わない。
- `legend_spec()` の `min_pct` と `max_pct` が、0 から 100 のすべての値で `ProgressTier::from_pct` と一致すること。
- PR #48 の `tz_offset.rs` と `alpha_gate_1.rs` が、呼び出しの形の書き換えだけで通ること。
  期待値は動かさない。

### 8.5 出力が変わる入力は一つだけ

不正な `scroll_viewport` の扱いだけは、意図して変える。
今の `render_svg` と `render_canvas_commands` は、読めない `viewport_json` を黙って無視して、窓の無い図を描く(`.and_then(|s| serde_json::from_str(&s).ok())`)。
新しい設定は、読めない `scroll_viewport` を `parse_error` で断る。
PR #48 が読めない `today_iso` を黙って捨てることをやめたのと同じ理由である。
ただし、読める入力の出力は変わらない。
縛りの試料(8.3 の 1)は、すべて読める入力である。
この決めは 11 で、覆せる決めとして挙げる。

## 9. PR #48 との順序

PR #48 は、同じ場所を大きく動かす。

- 凡例の字送りを字ごとの幅で数え、`LEGEND_TIER_ITEMS` などの定数に切り出す（Issue #28 の項目 1）。
- 凡例の右端で図の幅を下支えする。
- `render_svg`、`render_svg_error`、`render_canvas_commands` に `today_iso` と `tz_offset_minutes` を位置の引数として足す。
- SVG と IR と canvas の golden を作り直す。

### 9.1 最初の tag の前に入れる必要は無い

この文書の最初の版は、最初の publish の前に入れないと、下流の Rust の呼び手を壊すと書いていた。
互換性を約束しない前提(3.2)で、その切迫は無くなった。
この設計の実装は、`0.1.0-alpha.1` の tag の前でも後でもよい。
PR #46 と PR #48 の merge や tag の押し方に、この設計の都合を合わせる必要は無い。

### 9.2 PR #48 の後の master から切る理由は残る

互換性とは別の理由が三つあり、どれも残ることを確かめた。

- **同じ定数を二つの PR が作る。**
  `legend_spec()` が読む並びは、PR #48 が切り出す `LEGEND_TIER_ITEMS` である。
  先に作ると、二つの PR が同じ定数を作る。
- **同じ三つの関数の署名が衝突する。**
  この設計は、PR #48 が位置の引数として足す `today_iso` と `tz_offset_minutes` を、`options_json` の鍵へ移す。
  PR #48 の前に実装すると、PR #48 が移した先の引数を再び足して衝突する。
  PR #48 の `tz_offset.rs` と `alpha_gate_1.rs` と `wasm.rs` の呼び出しも、この設計が書き換える対象になる。
  PR #48 の後なら、書き換えるのは一度で済む。
- **golden の作り直し。**
  8.3 の縛りは、実装の土台の `master` の値で取る。
  PR #48 の前に縛ると、PR #48 が golden を作り直した時に、縛りが壊れる。

### 9.3 二重の手間を避ける別の道

PR #48 が `today_iso` と `tz_offset_minutes` を、位置の引数でなく最初から `options_json` の鍵として足せば、この設計が移す手間は半分になる。
これは PR #48 の持ち主が決めることで、この設計は前提にしない。
PR #48 が今の形で merge されても、この設計は成り立つ。

PR #48 が merge されないうちに実装を始める場合は、PR #48 の枝から切り、PR #48 が merge された後に土台を付け替える。

## 10. SPEC の書き換え案

書き換えは実装の作業で行う。
ここでは触れる場所と案を示す。
行番号は `master` の `3f2d7f4` のものである。
PR #48 も `SPEC.md` を書き換えるので、実装の時は見出しで探す。

### 10.1 触れる場所

| 場所 | 見出し | 変更 |
|------|--------|------|
| 7 行の表の下 | 「パッケージ構成」 | alpha の間は公開の形の互換性を約束しないことを足す(10.2) |
| 46 から 55 行 | 「API」の「ネイティブ (Rust)」 | `render` の署名を `options: &ChartOptions` に置き換え、`render_canvas` と `build_display_list` も並べる |
| 57 から 71 行 | 「API」の「Wasm」 | 三つの関数の署名を `(tasks_json, deps_json, options_json)` に置き換え、`today_iso`、`viewport_json` の項を `options_json` の鍵の説明に置き換え、`legend_spec_json` を足す |
| 73 から 84 行 | 「arc-vue」 | prop `legend` と `loadLegendSpec` を足す |
| 94 行から | 「SVG 出力仕様」の「チャート本体」 | 凡例の項を足し、既定で出ることを書く |
| 124 行 | 「達成率の塗り分け(P1)」の表の下の一文 | 凡例を消せることと、消した時の高さを足す |
| 142 行から | 「レイアウト定数」の表 | 表の下に、図の高さに凡例が 40px を占めることを足す |
| PR #48 が足す「日付と日時の字」と「時差の指定(`tz_offset_minutes`)」の節 | PR #48 の後の SPEC | `tz_offset_minutes` を、引数でなく `options_json` の鍵として書き直す |

### 10.2 案

「パッケージ構成」の表の下に、次を足す。

```
## 互換性

0.1.0-alpha 系の間は、公開の形の互換性を約束しない。
公開の形とは、Rust の署名、wasm の関数、arc-vue の props である。
破壊的な変更は、commit の題に `!` を付けて changelog に出す。
```

`cliff.toml` は `commit.breaking` を読んで `**BREAKING**` を付けるので、題に `!` を付ければ changelog に出る。

「ネイティブ (Rust)」の節を、次に置き換える。

```rust
pub fn render(
    tasks: &[GanttTask],
    deps: &[GanttDep],
    options: &ChartOptions,
) -> String

pub struct ChartOptions {
    pub today: Option<NaiveDate>,
    pub scroll_viewport: Option<ScrollViewport>,
    pub legend: LegendMode,
}
```

「Wasm」の節の署名と項を、次に置き換える。

```rust
#[wasm_bindgen]
pub fn render_svg(tasks_json: &str, deps_json: &str, options_json: Option<String>) -> String

#[wasm_bindgen]
pub fn render_svg_error(tasks_json: &str, deps_json: &str, options_json: Option<String>) -> Option<String>

#[wasm_bindgen]
pub fn render_canvas_commands(tasks_json: &str, deps_json: &str, options_json: Option<String>) -> String

#[wasm_bindgen]
pub fn legend_spec_json() -> String
```

```
- `options_json`: 描画の設定の JSON 文字列。省略可
  - `today`: 今日マーカー用の日付か日時の字(例: `"2026-06-16"`)
  - `scroll_viewport`: 仮想スクロール用の `{"scroll_y": f64, "client_height": f64}`
  - `tz_offset_minutes`: 日時の字を暦の日に直す時差(UTC から東へ何分か)
  - `legend`: `"shown"`(既定)か `"hidden"`
  - 省略した時と `{}` は、設定が無い時と同じ出力になる
  - object でない字、知らない鍵、知らない値は `parse_error` で断る
- `legend_spec_json()`: 凡例の中身(段の key、名、色、範囲と、進捗の線の名、色、太さ、破線)の JSON を返す。凡例を消した使い手が、同じ凡例を描くために読む
```

「arc-vue」の節に、次を足す。

```
- `legend`: `'shown'`(既定)か `'hidden'`。`'hidden'` で凡例を描かず、高さも詰める
- `loadLegendSpec()`: 凡例の中身を返す。wasm の初期化を済ませて読む
```

124 行を次に置き換える。

```
未達部分は常に `#d1d5db`(`bar-bg`)。
凡例は既定で SVG 下部に `bar-tier-legend` として表示する。
`options_json` の `legend` に `"hidden"` を渡すと、凡例を描かず、図の高さを `LEGEND_H`(40)だけ詰める。
その時の図は、既定の図から凡例を除き、高さと縦線の終わりを 40 縮めたものと同じになる。
日数で決まる幅が凡例の幅より狭い短い図(日数が 9 以下)では、凡例のために広げた幅も戻る。
その時、広げた幅の内側にだけあった今日の線(最後のタスクの終わりより後の今日)は出なくなる。
戻る先は、日数で決まる幅と、右へ伸びる札の右端の大きいほうである。
最後の日に終わる一日の棒やマイルストーンがある図では、札が収まる幅まで広がる。
凡例を出す図の幅は変わらない。
```

### 10.3 alpha の関門の約束に触れる所

触れる所を名指しする。

- **PR #48 の「契約が変わります」の節。**
  wasm の公開の形を変える変更は、0.1.0-alpha.1 の前に入れるという理由が書かれている。
  この設計の前提(3.2)では、その理由は要らなくなる。
  PR #48 の本文をどう直すかは、持ち主が決める。
- **PR #46 の版の決め。**
  arc-vue の版を `0.1.0-alpha.1` にし、`peerDependencies` を `^0.1.0-alpha.1` にする。
  arc-vue の新しい prop と `loadLegendSpec` は、その版以降に入ればよい。
  arc-vue の publish は Issue #22 が追っている。
- **Issue #28 の項目 1。**
  凡例の字送りの誤りで、PR #48 が塞ぐ。
  この設計の `Hidden` は、PR #48 の後の図の幅の下支えを外し、幅を日数と札の右端の大きいほうとする。
  9 日以下の図で、幅が動き、今日の線が消えうる(4.7)。
  凡例を出す図でも札が図の外へ出る入力があるが、この設計では直さない(4.8)。

## 11. 決めなかったことと、覆せる決め

### 11.1 決めなかったこと

- **`GanttLegend` の部品。**
  5.4 で足さないと決めた。
  使い手の様子を見て決める。
- **canvas の上限を、消した時だけ緩めるか。**
  緩めるのは 1 行分で、得るものが小さい。
  入力ごとに判定の式が分かれる費用に見合わない。

### 11.2 覆せる決め

- **不正な `scroll_viewport` を断ること(8.5)。**
  今は黙って無視している。
  断るほうが一貫するが、無視を保つなら、`scroll_viewport` だけ読めない時に無視する例外を書く。
- **`ChartOptions` に `#[non_exhaustive]` を付けないこと(7.1)。**
  付けると、外の呼び手は構造体の式で組めなくなり、builder が要る。
  alpha の間は素直に組めるほうを選んだ。
- **凡例を出す図の札の溢れを、別の変更で直すこと(4.8)。**
  この設計は、既定の出力を動かさない約束のために直さない。
  直す変更は、`Shown` の幅に札の右端の下支えを足し、`default_output_pin` の試料と golden を意図して焼き直す。
- **canvas の上限の判定が `legend` を見ること(4.6)。**
  別の道に、判定が両方の姿の取りうる最大の幅(日数の幅、凡例の下支え、札の右端の最大)を使う道がある。
  それは設定を見ずに済むが、凡例を出す図の判定が厳しくなる。
  日数の幅は上限に収まり、札だけが越える入力(541 日のタスクと最終日のマイルストーン)が、凡例を出す図で断られるようになる。
  この設計は凡例を出す図の出力を動かさないので、その道は採らなかった。
- **消した時に幅の下支えも外すこと(4.7)。**
  外すと、9 日以下の図で幅が動き、今日の線が消えうる。
  残す道に倒すなら、4.6 と 7.1 と 10.3 の「下支えも外す」を消し、4.7 を書き換える。
  札の右端で幅を決める式は残す。残さないと、(b) でも札が溢れる図が `Shown` と同じ幅で溢れる。
  8.4 の 9 日以下の試験は、札が日数の幅に収まる図で `Shown` から導く形に戻る。
  採らなかった理由は 4.7 に書いた。
- **`options_json` の上限 4 KiB(6.4)。**
  四つの鍵に十分な値として置いた。
  測った値ではない。

## 12. 実装の分け方

実装は、次の順に分ける。
1 と 2 を別の commit にするのは、2 が出力を変えないことを、1 の試験で確かめるためである。

1. **現状を縛る試験。**
   8.3 の 1。
   実装に入る前の `master`(PR #48 の後のもの)で通る試験だけの commit。
2. **設定の移動。**
   署名を `options` に置き換え(B)、`today`、`viewport`、`tz_offset_minutes` を設定に移す。
   凡例の機能は入れない。
   この commit の前後で、1 の試験が一字も動かないこと。
   書き換えるのは 6.2 の 153 件と、PR #48 の試験の呼び出しである。
3. **凡例を消す機能と、凡例の中身。**
   `LegendMode`、`legend_spec`、`legend_spec_json`、8.4 の試験(9 日以下の図の幅の試験を含む)、SPEC の Wasm と Rust の節。
   ここまでが core である。
4. **arc-vue。**
   core の wasm を取り込んだ後に、prop、高さ、`loadLegendSpec`、story、試験。
   SPEC の arc-vue の節。

1 から 3 は core の一つの PR にまとめてよい。
4 は、arc-vue が wasm の宣言に依るので、core が先に入ってから出す。

## 13. 確かめていないこと

- 実装していないので、`Hidden` の出力が 4.3 の通りになることは、コードの読みからの予測である。
  8.4 の試験が、実装の時にこれを確かめる。
- 設定の移動(B)が出力を変えないことは、8.3 の 1 の試験が実装の時に確かめる。
  この設計では実装していない。
- Chromatic と VRT の既存の story が差分 0 であることは、実装の時に確かめる。
  この設計では走らせていない。
- 外の使い手が居ないことは確かめていない(3.3)。
  この設計は、それを確かめなくても成り立つ。
- 余分な引数を wasm-bindgen が捨てることは、wasm-bindgen の動きとして知られているが、この設計では確かめていない。
  `scripts/*.mjs` の書き換え漏れを実行時に見つけられない場合がある、という 8.4 の記述はこれに依る。
- 4.7 の数(274px、422px、9 日と 10 日)は、PR #48 の先端 `8d1a5c3` の定数と文言から、読んで算出した。
  実装の試験(`hidden_legend_tests`)は、同じ数を定数から別に求めて縛っている。
- 札の幅の見積もり(4.7)は、実際の字体では確かめていない。
  DejaVu Sans Bold のような広い字体では、100% の札が約 34px で、見積もりの 28.4px を越える。
  そのとき `Hidden` の図でも、札の末尾が数 px だけ図の右端から切れうる。
- PR #46、#47、#48 は open で、この設計の後に内容が動き得る。
