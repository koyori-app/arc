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
| 一、消した跡の 40px | 詰める。ただし消した時だけで、既定の寸法は動かさない（4） |
| 二、使う側に出す物 | 段の色と名と範囲、進捗の線の色と太さと破線と名を、一つの定義から出す。Rust の関数、wasm の関数、arc-vue の読み込み関数の三つで取れる（5） |
| 三、入口の形 | Rust は `_with` を付けた入口を足し、既存の入口は触らない。wasm は末尾に任意の `options_json` を一つ足す。Vue は任意の prop `legend` を足す（6） |

既定は一字も変えない。
指定が無い時に wasm へ渡る呼び出しそのものが、今と同じであることまで縛る（8）。

`master` の時点で、GitHub の Release も tag も 0 件で、`@koyori-app/arc` と `@koyori-app/arc-vue` と `koyori-arc-core` は、どれも公開されていない（3）。
この設計は、その事実を二か所で使う。
一つは、wasm の三つの関数の Rust 側の署名を今のうちに変えること。
もう一つは、設定の名前と、未知の鍵を断る厳しさを今のうちに決めることである。

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

## 3. 公開の事実とその使い方

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

### 3.2 窓がいつ閉じるか

窓は、最初の tag を押して release.yml が走った時に閉じる。
open の PR がそこへ向かっている。

- PR #46 は版を `0.1.0-alpha.1` に上げ、arc-vue の `peerDependencies` を `^0.1.0-alpha.1` にする。
- PR #48 は、`render_svg` と `render_canvas_commands` に `tz_offset_minutes` を足し、`render_svg_error` に `today_iso` と `tz_offset_minutes` を足す。
  本文に「0.1.0-alpha.1 を出す前に入れるのは、出した後だと版違いで壊れる期間ができるためです」とある。

この設計は、PR #48 と同じ考えに立つ。
wasm の署名を動かす変更は、最初の publish の前に入れる。

### 3.3 この事実で言えないこと

「公開されていない」は、使っている者がいないという意味ではない。
arc を git の参照や workspace で取り込んでいる利用側があるかどうかは、npm と crates.io と GitHub の公開情報からは見えない。
確かめていない。
だから、窓を使う判断は「外の使い手を壊さない」ではなく、「公開した後は、同じ変更が別の費用になる」に置く。

### 3.4 窓を何に使うか

- **Rust 側の署名を変える。**
  wasm の三つの関数は `pub fn` でもある。
  末尾に引数を足すと、Rust の呼び手は壊れる。
  公開した後なら、下流の Rust の呼び手を壊す変更になる。
  今なら呼び手は repo の中の試験と bench だけである（6.2）。
- **名前を決める。**
  `legend`、`LegendMode`、`options_json`、`legend_spec_json` は、公開した後に改めるほど高くつく。
- **未知の鍵を断る厳しさを決める。**
  後から断るようにするのは壊れる変更で、後から緩めるのは壊れない。
  だから厳しい側から始める。

窓を、これ以上の整理に使うことはしない。
たとえば `today` と `viewport` と `tz_offset_minutes` を `options_json` へ移す案は、呼び手を全面的に壊す。
この設計の `options_json` はそれを妨げないが、別に決める（11）。

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

### 4.3 詰めた時に動く物は図の高さと二つの縦線の終わりだけ

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

それより上の画素は、既定の図と同じである。
この関係は、試験の答えにそのまま使える（8.4）。

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
  凡例が無いので、幅を伸ばす理由が無い。
- canvas の上限の判定（`canvas_graph_limit_error`）は、消していても凡例の分の高さを足したままにする。
  上限を測る式を一つに保てるし、判定が最大で 1 行分だけ早く断るだけで、緩むことが無い。
  この選択は 6.3 で wasm の署名に響く。

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
| C | 設定の構造体を末尾の引数として足す |
| D | 設定を取る別の入口を足し、既存の入口は触らない |
| E | Rust は D、wasm は末尾に任意の `options_json` を一つ足す |

### 6.2 壊れる呼び手の数え上げ

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

形ごとに、壊れる件数を並べる。

| 形 | R1 | R2 | J1 | J2 | V | 以後の旗 |
|----|---:|---:|---:|---:|--:|----------|
| A 位置の引数を足す | 73 | 60 | 0 | 0 | 0 | 旗ごとに位置が一つ増える。`render_svg` はすでに 4 つ（PR #48 で 5 つ）で、使う側は途中を `undefined` で埋める |
| B 構造体に置き換える | 73 | 60 | 4 | 16 | 0 | 旗は構造体に足せる。ただし全呼び手が壊れる |
| C 構造体を末尾に足す | 73 | 60 | 0 | 0 | 0 | 旗は構造体に足せる。Rust は A と同じだけ壊れる |
| D 別の入口を足す | 0 | 0 | 0 | 0 | 0 | 入口が二重になる。旗ごとに両方へ通す |
| E Rust は D、wasm は末尾に任意の JSON | 0 | 60 | 0 | 0 | 0 | 旗は JSON の鍵に足せる。位置は増えない |

JS の呼び手が壊れないのは、wasm-bindgen が末尾の `Option` を任意の引数として出すからである。
確かめた。
基準の commit で `wasm-pack build --target web --dev` を走らせて出た宣言は、次の通りだった。

```
export function render_svg(tasks_json: string, deps_json: string, today_iso?: string | null, viewport_json?: string | null): string;
```

`scripts/bench-canvas-crossover.mjs` は `render_svg(tasksJson, depsJson, fx.today)` と三つしか渡しておらず、末尾を省いた呼び出しが既に動いている。
`render_svg_error` は今は必須の二つしか持たないが、末尾に任意の引数を足すのは同じく壊れない。

### 6.3 選ぶ形と理由

E を選ぶ。

- **Rust の 73 件を壊さない。**
  外部の backend を書く側が呼ぶのは `build_display_list` である（PR #34 で IR の型が公開された）。
  この入口を位置のまま保つ。
- **JS の呼び手を壊さない。**
  20 件（J1 と J2）は触らない。
- **R2 の 60 件は壊れるが、機械的である。**
  どれも repo の中の試験と bench で、末尾に `None` を足せば直る。
  コンパイラが全箇所を示す。
  PR #48 が `tz_offset_minutes` で同じ件数を同じ方法で直している。
- **D にしない理由。**
  wasm にも `_with` を足すと、三つの入口が六つになり、旗ごとに両方を通す。
  窓があるうちに、60 件の機械的な書き換えで済ませるほうが、二重の入口を永久に持つより安い。
  公開した後なら、この 60 件が下流の Rust の呼び手を壊す変更になり、D を選ぶほかなくなる。
- **JSON の字で渡す理由。**
  `viewport_json` が既に同じ形で、JSON の字を取る。
  旗が増えても位置は増えない。

`options_json` の決め:

| 入力 | 扱い |
|------|------|
| 省略、`undefined`、`null` | 既定。呼び出しは今と同じ |
| `"{}"` | 既定 |
| 未知の鍵 | `parse_error` で断る。メッセージに鍵の名を入れる |
| `legend` が `"shown"` か `"hidden"` 以外 | `parse_error` で断る |
| 長さが上限（1 KiB）を超える | `input_limit` で断る |

未知の鍵を断るのは、綴り違い（`lengend`）が黙って何も起こさないことを防ぐためである。
また arc と arc-vue の版が食い違った時に、新しい arc-vue が古い arc へ未知の鍵を送ると、黙って凡例が出続ける代わりに、図の上の警告で気づける。
`render_svg` は断る時に空の SVG を返し、理由は `render_svg_error` が返す。
この二つは同じ検めを共有する（`prepare_svg_input`）ので、`render_svg_error` にも `options_json` を末尾に足す。
PR #48 が `today_iso` で同じことをしている。

canvas の上限は、`options_json` を見ずに判定する（4.6）。
そのため、`canvas_graph_limit_error` の式は変えない。

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

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(default, deny_unknown_fields)]
#[non_exhaustive]
pub struct ChartOptions {
    pub legend: LegendMode,
}

impl ChartOptions {
    pub fn with_legend(mut self, legend: LegendMode) -> Self {
        self.legend = legend;
        self
    }
}
```

`#[non_exhaustive]` は、旗が増えても外部の呼び手を壊さないために付ける。
この属性が付くと、crate の外では構造体の式（`..Default::default()` を含む）で組めない。
そのため、外の呼び手は `ChartOptions::default().with_legend(LegendMode::Hidden)` の形で組む。
旗を足す時は、`with_` の関数を一つ足す。

- `build_display_list_with(graph, epoch, today, scroll_viewport, &ChartOptions)` を足す。
  `build_display_list` は `ChartOptions::default()` を渡して呼ぶだけにする。
- `render_with` と `render_canvas_with` を足す。
  `render` と `render_canvas` はそれを呼ぶだけにする。
- `build.rs` は、`Hidden` の時に `chart_h` を `chart_height_with` で求め、凡例の層を積まない。
  PR #48 の後は、図の幅の下支えも外す。
- `legend_spec()` を足す。
  凡例の層の組み立てと、この関数が、同じ並びを読む。
- `Hidden` の時の `metadata.primitive_count` は、凡例の分を数えない。

### 7.2 wasm

- `render_svg`、`render_svg_error`、`render_canvas_commands` の末尾に `options_json: Option<String>` を足す。
- `legend_spec_json() -> String` を足す。
- `render_canvas_commands` の `canvas_graph_limit_error` は変えない（4.6）。

### 7.3 Vue

- props に `legend?: 'shown' | 'hidden'` を足す。
- 指定が無い時は、`options_json` を渡さない（未指定のまま wasm を呼ぶ）。
  指定がある時だけ `JSON.stringify({ legend })` を渡す。
  boolean の prop にしない理由がここにある。
  Vue は、省略された boolean の prop を `false` に変える。
  `showLegend` を既定なしで宣言すると、既存の使い手の凡例が黙って消える。
  列挙の prop は、省略が `undefined` のまま残る。
- wasm を呼ぶ四か所のすべてに `options_json` を通す: `svgHtml`、`canvasCommandsJson`、`svgFailure`（`render_svg_error`）、`renderCanvasFallback`。
  四つ目を漏らすと、canvas が上限で SVG へ落ちた時に凡例が戻る。
- `chartHeightForTaskCount(taskCount, legend = 'shown')` の第二引数を足す。
  `legend` が `hidden` の時は `RUST_LAYOUT.LEGEND_H` を引く。
  `wasmContract.ts` の `RUST_LAYOUT` は変えない。
  `wasmContract.test.ts` が Rust と照らす値はそのままで、足す関数が `LEGEND_H` を使う。
- `index.ts` から `loadLegendSpec` と `LegendSpec` の型を出す。
- `GanttChart.stories.ts` に `LegendHidden`（SVG と canvas）の story を足す。
  既存の story は変えない。

## 8. 既定が一字も変わらないことの縛り

### 8.1 縛る対象

指定が無い時に、次の三つが一致する。

- Rust: `render`、`render_canvas`、`build_display_list` の戻り値。
- wasm: `render_svg`、`render_canvas_commands` の戻り値。
- Vue: 指定が無い時に wasm へ渡る呼び出しの引数。

### 8.2 今の golden では足りない

`crates/koyori-arc-core/tests/fixtures` の golden は、小さい図だけを byte で縛る。
SVG は `two_tasks.svg`、`two_tasks_today.svg`、`milestone.svg`、`empty.svg`、IR と canvas は `two_tasks.json` の一枚ずつである。
`display_list_tests.rs` の `p0_ir_golden_100_sparse` と `p0_ir_golden_2000_dense` は、二度組んで同じになることと個数を見るだけで、中身は縛らない。
つまり、100 タスクや 2000 タスク、仮想化の窓、日本語の長い題、五つの段のすべてがある図は、byte では縛られていない。

### 8.3 縛り方

1. **最初の commit で、現状を縛る試験だけを足す。**
   実装に入る前の `master` で通る試験である。
   - 試料は次を含む: 既存の golden の試料、五つの段がすべて出る図、`end` が無い図、今日が範囲の内と外にある図、仮想化の窓が上と中と下にある図、100 タスクの疎、2000 タスクの密、日本語の長い題、XML の特殊文字を含む題。
   - 各試料で SVG、canvas の JSON、IR の JSON の三つを縛る。
     小さい試料は全文を fixture に置く。
     大きい試料は、長さと FNV-1a 64 の値で縛る。
     標準の `DefaultHasher` は版をまたいだ安定を約束しないので使わない。
     FNV-1a は依存を足さずに書ける。
2. **以後の commit は、この fixture を一字も変えない。**
   確かめる手順を実装の作業の説明に書く。
   `git diff --diff-filter=MD --name-only <縛りの commit>..HEAD -- crates/koyori-arc-core/tests/fixtures` が空であること。
   新しい fixture の追加は許す。
3. **同じ試料で、指定の三通りが一致することを縛る。**
   戻り値が、(1) 既存の入口、(2) `_with` に `ChartOptions::default()`、(3) `_with` に `legend: Shown` の三つで同一であること。
   三つとも 1 で縛った値と比べる。
   既存の入口どうしの比べだけでは、共通の部分を直した時に二つとも動いて気づけない。
4. **wasm の入口でも縛る。**
   `options_json` が省略、`null`、`"{}"`、`{"legend":"shown"}` の四通りで、`render_svg` と `render_canvas_commands` の戻り値が、現状の値と一致すること。
5. **Vue で、呼び出しの引数を縛る。**
   `GanttChart.virtualization.test.ts` が既に wasm の関数を mock して引数を見ている。
   同じ型で、`legend` を省略した時に `render_svg` と `render_canvas_commands` に渡る引数が、今の引数の列と同じ長さと値であること（末尾に `undefined` を足すことも断る）。
   `chartHeightForTaskCount(n)` の既存の試験（408 で 16 400）はそのまま残す。
6. **像で縛る。**
   Chromatic と VRT の既存の story が、差分 0 であること。

### 8.4 消した時の試験

消した時の出力は、既定の出力から機械的に導ける（4.3）。
これを試験の答えにする。

- `Hidden` の SVG は、`Shown` の SVG から、凡例の二つの `<g>` を取り除き、ルートの `height` と `viewBox` の第四の数と、週の縦線と今日の縦線の `y2` を 40 減らしたものに、byte で一致する。
- `Hidden` の canvas の JSON と IR も、同じ操作で `Shown` から導く。
- `Hidden` で、凡例の層が無いこと、`primitive_count` が凡例の分だけ減ること。
- `Hidden` の `viewport.height` は `chart_height(rows) - LEGEND_H`。
- 上限の判定は、`Hidden` でも `Shown` と同じ入力で同じ結果になること。
- 不正な `options_json` が、`render_svg_error` と `render_canvas_commands` で `parse_error` になること。
  `render_svg` が空の SVG を返し、`render_svg_error` が同じ理由を返すこと。
- `legend_spec()` が、`Shown` の図の凡例の層から読み取った字と色と一致すること。
  凡例が描く物と、使う側へ出す物が食い違わない。
- `legend_spec()` の `min_pct` と `max_pct` が、0 から 100 のすべての値で `ProgressTier::from_pct` と一致すること。

## 9. PR #48 との順序

PR #48 は、同じ場所を大きく動かす。

- 凡例の字送りを字ごとの幅で数え、`LEGEND_TIER_ITEMS` などの定数に切り出す（Issue #28 の項目 1）。
- 凡例の右端で図の幅を下支えする。
- `render_svg`、`render_svg_error`、`render_canvas_commands` の署名を変える。
- SVG と IR と canvas の golden を作り直す。

この設計を実装する作業は、PR #48 が merge された後の `master` から切る。
理由は三つある。

- 8.3 の「現状」は、実装の土台の `master` の値で縛る。
  PR #48 の前に縛ると、PR #48 が golden を作り直した時に、縛りが壊れる。
- `legend_spec()` が読む並びは、PR #48 が切り出す `LEGEND_TIER_ITEMS` である。
  先に作ると、二つの PR が同じ定数を作る。
- 署名を変える三つの関数に、PR #48 と同じ場所で末尾の引数を足す。
  順序を逆にすると衝突する。

PR #48 が merge されないうちに実装を始める場合は、PR #48 の枝から切り、PR #48 が merge された後に土台を付け替える。

## 10. SPEC の書き換え案

書き換えは実装の作業で行う。
ここでは触れる場所と案を示す。
行番号は `master` の `3f2d7f4` のものである。
PR #48 も `SPEC.md` を書き換えるので、実装の時は見出しで探す。

### 10.1 触れる場所

| 場所 | 見出し | 変更 |
|------|--------|------|
| 124 行 | 「達成率の塗り分け（P1）」の表の下の一文 | 凡例を消せることと、消した時の高さを足す |
| 57 から 71 行 | 「API」の「Wasm」 | 三つの関数の署名に `options_json` を足し、`legend_spec_json` を足す |
| 73 から 84 行 | 「arc-vue」 | prop `legend` と `loadLegendSpec` を足す |
| 94 行から | 「SVG 出力仕様」の「チャート本体」 | 凡例の項を足し、既定で出ることを書く |
| 142 行から | 「レイアウト定数」の表 | 表の下に、図の高さに凡例が 40px を占めることを足す |

### 10.2 案

124 行を次に置き換える。

```
未達部分は常に `#d1d5db`（`bar-bg`）。
凡例は既定で SVG 下部に `bar-tier-legend` として表示する。
`options_json` の `legend` に `"hidden"` を渡すと、凡例を描かず、図の高さを `LEGEND_H`（40）だけ詰める。
その時の図は、既定の図から凡例を除き、高さと縦線の終わりを 40 縮めたものと同じになる。
```

「Wasm」の節に、次の項を足す。

```
- `options_json`: 描画の設定の JSON 文字列。省略可。`{"legend": "shown" | "hidden"}`
  - 省略した時と `{}` は、設定が無かった時と同じ出力になる
  - 知らない鍵と、知らない値は `parse_error` で断る
- `legend_spec_json()`: 凡例の中身（段の key、名、色、範囲と、進捗の線の名、色、太さ、破線）の JSON を返す。凡例を消した使い手が、同じ凡例を描くために読む
```

「arc-vue」の節に、次を足す。

```
- `legend`: `'shown'`（既定）か `'hidden'`。`'hidden'` で凡例を描かず、高さも詰める
- `loadLegendSpec()`: 凡例の中身を返す。wasm の初期化を済ませて読む
```

### 10.3 alpha の関門の約束に触れる所

触れる所を名指しする。

- **PR #48 の「契約が変わります」の節。**
  wasm の公開の形を変える変更は、0.1.0-alpha.1 の前に入れるという約束である。
  この設計の wasm の変更（三つの関数の末尾に `options_json`、新しい `legend_spec_json`）は、その約束に入る。
  実装は alpha.1 の前に終える。
- **PR #46 の版の決め。**
  arc-vue の版を `0.1.0-alpha.1` にし、`peerDependencies` を `^0.1.0-alpha.1` にする。
  arc-vue の新しい prop と `loadLegendSpec` は、その版から入る。
  arc-vue の publish は Issue #22 が追っており、arc-vue の窓は core より長く開いている可能性があるが、core が先に出れば、wasm 側の約束は先に閉じる。
- **Issue #28 の項目 1。**
  凡例の字送りの誤りで、PR #48 が塞ぐ。
  この設計の `Hidden` は、PR #48 の後の図の幅の下支えを外す。

## 11. 決めなかったこと

- **`today`、`viewport`、`tz_offset_minutes` を `options_json` へ移すか。**
  一つの入口に集まって整うが、全呼び手が壊れる（6.2 の B）。
  この設計は妨げない。
  `options_json` の鍵に足すだけで始められる。
- **`GanttLegend` の部品。**
  5.4 で足さないと決めた。
  使い手の様子を見て決める。
- **canvas の上限を、消した時だけ緩めるか。**
  緩めるのは 1 行分で、得るものが小さい。
  入力ごとに判定の式が分かれる費用に見合わない。

## 12. 実装の分け方

実装は、次の順に二つへ分ける。

1. **core。**
   8.3 の 1（現状を縛る試験だけの commit）を最初に置く。
   次に `ChartOptions`、`LegendMode`、`legend_spec`、`_with`、wasm の `options_json` と `legend_spec_json`、8.4 の試験。
   SPEC の Wasm の節。
2. **arc-vue。**
   core の wasm を取り込んだ後に、prop、高さ、`loadLegendSpec`、story、試験。
   SPEC の arc-vue の節。

分ける理由は、arc-vue が wasm の宣言に依るからである。
core が先に入らないと、arc-vue の typecheck が通らない。

## 13. 確かめていないこと

- 実装していないので、`Hidden` の出力が 4.3 の通りになることは、コードの読みからの予測である。
  8.4 の試験が、実装の時にこれを確かめる。
- Chromatic と VRT の既存の story が差分 0 であることは、実装の時に確かめる。
  この設計では走らせていない。
- wasm-bindgen の宣言は、末尾の `Option` が任意の引数になることを確かめた（6.2）。
  `render_svg_error` に末尾の引数を足した時の宣言は、足していないので見ていない。
  同じ規則で出るはずだが、実装の時に宣言を見る。
- PR #46、#47、#48 は open で、この設計の後に内容が動き得る。
- task など、arc を取り込む利用側の呼び方は見ていない（3.3）。
