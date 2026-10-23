# koyori-arc 仕様書

Rust → Wasm + SVG の Gantt チャートレンダラー。

名前の由来は三重: 依存関係の弧（dependency arc）、アーク放電（arc discharge）、アーク炉（arc furnace）。

## パッケージ構成

| パッケージ | 役割 | 状態 |
|---|---|---|
| `koyori-arc-core` (crate) | Rust ネイティブ + Wasm コア。SVG 文字列を生成する | 実装済み |
| `@koyori-app/arc` (npm) | `koyori-arc-core` を wasm-pack でビルドした npm 配布物 | 実装済み |
| `@koyori-app/arc-vue` (npm) | Vue 3 ラッパーコンポーネント | 実装済み |
| `@koyori-app/arc-react` (npm) | React ラッパー | **凍結**。コントリビュート/メンテナ見込みができた場合、または需要が大きくなった場合のみ着手 |

## 互換性

0.1.0-alpha 系の間は、公開の形の互換性を約束しない。
公開の形とは、Rust の署名、wasm の関数、arc-vue の props である。
破壊的な変更は、commit の題に `!` を付けて changelog に出す。

## ライセンス

MPL-2.0 OR GPL-2.0-or-later（デュアルライセンス）。ライセンス全文は `LICENSE-MPL-2.0` / `LICENSE-GPL-2.0-or-later`（`/usr/share/common-licenses/` から取得）。

## 入力型

task プロジェクトの entity スキーマに合わせている。

```rust
pub struct GanttTask {
    pub id: String,
    pub title: String,          // tasks.title
    pub progress_pct: i16,      // tasks.progress_pct (0-100)
    pub start: NaiveDate,       // sprint.start_date から解決
    pub end: Option<NaiveDate>, // tasks.hard_deadline or soft_deadline
}

pub struct GanttDep {
    pub blocker_task_id: String,  // task_relations.blocker_task_id
    pub blocked_task_id: String,  // task_relations.blocked_task_id
}
```

### 日付と日時の字

`start`・`end`・`today` は、次のどれの字も受ける。

- 日付: `2026-06-01`
- 時差つきの日時（RFC 3339）: `2026-06-01T09:00:00Z`、`2026-06-08T09:00:00+09:00`、小数秒つき `2026-06-01T09:00:00.123Z`
- 時差なしの日時: `2026-06-01T09:00:00`、`2026-06-01 09:00:00`

task の `hard_deadline` と `soft_deadline` は timestamptz なので、JSON ではそのまま日時の字で届く。呼び手が日付へ直さなくても渡せる。

### 時差の指定（`tz_offset_minutes`）

日時の字がどの暦の日になるかは、呼び手が渡す時差で決まる。
`tz_offset_minutes` は、UTC から東へ何分ずれているかを表す、符号つきの整数である（JST は `540`、アメリカ東部の標準時は `-300`）。

| 字 | 指定なし | `540`（JST） | `0`（UTC） |
|---|---|---|---|
| `2026-06-08`（日付だけ） | 6 月 8 日 | 6 月 8 日 | 6 月 8 日 |
| `2026-06-08T15:30:00Z` | 6 月 8 日 | 6 月 9 日 | 6 月 8 日 |
| `2026-06-09T00:30:00+09:00` | 6 月 9 日 | 6 月 9 日 | 6 月 8 日 |
| `2026-06-08T23:30:00`（時差なしの日時） | 6 月 8 日 | 6 月 8 日 | 6 月 8 日 |

- **日付だけの字は換算しない。** 暦の日がもう書かれている。換算すると、書かれた日からかえってずれる。
- **時差なしの日時の字も換算しない。** どの時差の時刻か分からず、直す元が無い。書かれた日を取る。
- **時差つきの日時の字は、指定があればその時差の暦の日へ直す。** 指定がなければ、字が書いている暦の日をそのまま取る（`+09:00` の字なら、その `+09:00` の日）。
- **指定なしと `0` は同じではない。** `+09:00` の字を、指定なしなら書かれた日で、`0` なら UTC の日で読む。指定なしは、この口ができる前の振る舞いと 1 字も違わない。
- `start`・`end`・`today` の三つとも、同じ筋で読む。
- 整数でない値、NaN、無限大、絶対値が 1439 を超える値は、黙って 0 にせず `parse_error` で断る（メッセージに `tz_offset_minutes` と入る）。現実の時差は `-720` から `840` の間である。

**task に結ぶ時に渡す値**: `hard_deadline` と `soft_deadline` は timestamptz で、UTC の字（`...Z`）で届くことが多い。
指定なしのまま渡すと、日本の締切（6 月 9 日の 00:30 JST は UTC では 6 月 8 日の 15:30）が、黙って前の日に描かれる。
利用者の時差で締切の日を見せたいなら、その利用者の時差を `tz_offset_minutes` に渡す。
日付だけの字（sprint の `start_date` など）は、換算されずにそのまま使われるので、同じ呼び出しに混ぜてよい。

**なぜ時差を暗黙に拾わないか**: arc は利用者の時差を自分では決めない。ブラウザの時差（`Intl.DateTimeFormat().resolvedOptions().timeZone`）も読まない。
暗黙に拾うと、同じデータが、見る人の端末の設定によって違う日に描かれる。
スクリーンショットや VRT も、実行した環境で結果が変わる。
どの時差で見せるかは、呼び手が決めて明に渡す。たとえば task が利用者の設定にある時差を持っているなら、それを渡す。

**限界**: 一つの図には一つの固定の時差（分）しか渡せない。夏時間のある地域では、夏と冬で時差が変わるので、片方の季節の締切が 1 日ずれうる。
そういう呼び手は、渡す前に自分の暦で日付（`YYYY-MM-DD`）へ直す。日付だけの字は換算されない。
IANA の名前（`Asia/Tokyo`）を受ける案は採らなかった。wasm の束が約 4 倍に太るためである（下の「束の大きさ」）。

**束の大きさ（IANA の名前を受ける案との比較）**: `chrono-tz`（既定の機能、全地域）を足した試作の wasm は、`wasm-pack build --release` で 284,936 バイト（gzip 115,502）から 1,162,180 バイト（gzip 264,401）になった。
約 877 KB（gzip で約 149 KB）の増加である。alpha の段階では、分の整数で足りるうえに軽い。

読めない字は、黙って捨てずに明に断る。`start` と `end` は JSON 全体が `parse_error` になり（メッセージに `tasks[3].end` のように、何番目のタスクのどの欄かが入る）、`options_json` の `today` も同じく `parse_error` になる（メッセージに `today` と入る）。
`today` が無い時、`null` の時、空の字・空白だけの時は「今日の線なし」で、誤りではない。

`GanttTask` メソッド:
- `start_days(epoch)` — epoch からの経過日数
- `end_days(epoch)` — epoch から終了日までの経過日数。`end` が `None` の場合は `start + 1日` にフォールバック。`end` が `start` より前なら `start` に切る
- `progress()` — `progress_pct` (0-100) を 0.0-1.0 に正規化

## API

### ネイティブ (Rust)

```rust
pub fn render(
    tasks: &[GanttTask],
    deps: &[GanttDep],
    options: &ChartOptions,
) -> String

pub fn render_canvas(
    tasks: &[GanttTask],
    deps: &[GanttDep],
    options: &ChartOptions,
) -> CommandBuffer

pub fn build_display_list(
    graph: &GanttGraph,
    epoch: NaiveDate,
    options: &ChartOptions,
) -> DisplayList

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct ChartOptions {
    pub today: Option<NaiveDate>,
    pub scroll_viewport: Option<ScrollViewport>,
    pub legend: LegendMode, // Shown（既定）か Hidden
}

pub fn legend_spec() -> LegendSpec
```

`ChartOptions::default()` は、今日の線も仮想スクロールも無く、凡例を出す図を描く。
`ChartOptions { today: Some(day), ..Default::default() }` のように組む。

### Wasm

```rust
#[wasm_bindgen]
pub fn render_svg(tasks_json: &str, deps_json: &str, options_json: Option<String>) -> String

#[wasm_bindgen]
pub fn render_svg_error(tasks_json: &str, deps_json: &str, options_json: Option<String>) -> Option<String>

#[wasm_bindgen]
pub fn render_canvas_commands(tasks_json: &str, deps_json: &str, options_json: Option<String>) -> String
```

- `tasks_json` / `deps_json`: 上記構造体配列の JSON 文字列
- `options_json`: 描画の設定の JSON object の字。省略可
  - `today`: 今日マーカー用の ISO 8601 の日付（例: `"2026-06-16"`）または日時（例: `"2026-06-16T09:00:00+09:00"`）の字。読めない字は断る（上の「日付と日時の字」）
  - `tz_offset_minutes`: 日時の字を直す時差（UTC から東へ何分か。JST は `540`）。省略は「書かれた日をそのまま取る」で、`0` とは違う（上の「時差の指定」）
  - `scroll_viewport`: 仮想スクロール用の `{"scroll_y": f64, "client_height": f64}`
  - `legend`: `"shown"`（既定）か `"hidden"`。`"hidden"` で凡例を描かず、図の高さを詰める（下の「達成率の塗り分け」）
  - 省略、`undefined`、`null`、`{}` は、設定が無い時と同じ出力になる。値が `null` の鍵も、その鍵が無いのと同じ
  - object でない字、知らない鍵（メッセージに鍵の名が入る）、形の違う値、読めない `today`、使えない時差は `parse_error` で断る。読めない `scroll_viewport` も断る（以前は黙って無視していた）
  - 4 KiB を超える字は `input_limit` で断る
- `render_svg` は断る時に空 SVG を返し、`render_svg_error(tasks_json, deps_json, options_json)` から理由を取得できる。`render_svg` に渡したものと同じ `options_json` を渡す（読めない設定も断りの一つで、この経路でしか分からない）
- `render_canvas_commands` は Canvas2D 再生用 `CommandBuffer` の JSON を返し、エラー時も `{"error": ..., "code": ...}` の JSON を返す
- `legend_spec_json()`: 凡例の中身（段の `key`、名、色、範囲と、進捗の線の名、色、太さ、破線）の JSON を返す。凡例を消した使い手が、同じ凡例を描くために読む。範囲は `ProgressTier::from_pct` を 0 から 100 まで走らせて導き、色は図が使うパレットから引く

### arc-vue

```vue
<GanttChart
  :tasks="tasks"
  :deps="deps"
  :today="todayIso"
  :tz-offset-minutes="540"
  @task-click="onTaskClick"
/>
```

- `data-task-id` 属性によるクリックデリゲーションで `taskClick` イベントを emit

## SVG 出力仕様

### ルート要素

- `<svg xmlns="..." width="..." height="..." viewBox="0 0 W H" role="img" aria-label="Gantt chart">`
- アクセシビリティ: `<title>Gantt chart</title>` + `<desc>Task schedule with progress bars and dependency arrows</desc>`
- 空入力: `tasks` が空の場合 `width="0" height="0" viewBox="0 0 0 0"` の空 SVG（`role="img"` + `<title>`/`<desc>` 付き）

### チャート本体

- ヘッダー: 週単位のグリッド線 + 日付ラベル（`M/D` 形式、月曜始まり）
- タスクバー: `<g data-task-id="...">` 内に:
  - `<title>` ツールチップ（名前・開始日・終了日・進捗%）
  - 背景バー `class="bar-bg"`（未達部分、`#d1d5db`）
  - 進捗バー `class="bar-progress bar-tier-{low|mid|high|done}"`（達成部分、達成率帯で色分け）
  - 進捗% テキスト（任意表示だが現行実装ではバー上に表示）
  - 左ラベル: 長タイトルはラベル欄の幅（112px）に収まるまで切り `…` を付ける（全文は `<title>` で参照可）。幅は字ごとの見積もりで、和文は 1 字 12px、英数字は 12px の 0.3〜0.85 倍。字数でもバイト数でもない
- ゼロ期間タスク: `◇` ダイヤモンドマーカー（`<polygon class="bar-milestone bar-tier-...">`）
- `end < start` ガード: `end` を `start` に切る（幅 0 扱い）。幅 0 のタスクは `◇` で描くので、`end < start` でもタスクは消えない。進捗の点も `start` より左へ出ない
- 依存関係: frappe-gantt 互換の角丸エルボー矢印（`<path>` + オープンシェブロン）
- 進捗ステータスライン: 全タスクの進捗ポイントを結ぶ赤破線ジグザグ（`class="progress-status-line"`）— **設計意図どおり温存**
  - **1 タスク = 1 代表点**: `(progress_x, y_mid)`。`progress_x = start + (end - start) × progress`、`y_mid = (y_top + y_bottom) / 2`
  - **点の順は行の順**: 代表点は入力の順でなく、行（上から下）の順に結ぶ。依存で行が入れ替わっても、線は上から下へ単調に進む
  - **斜め直線結線**: 連続する代表点は斜め直線で結ぶ。バー縦貫（`y_top`→`y_bottom` の垂直線）や水平ジョグ（直角エルボー）は禁止
  - **today 指定かつチャート範囲内**: 端点を today 縦線上に固定。`(today_x, 最上段 y_top)` から開始 → 各タスクの代表点へ斜め接続 → `(today_x, 最下段 y_bottom)` で終端
  - **today 未指定、または today が範囲外**: レガシー。各タスクの代表点のみを斜め直線で順に結び、today 縦線へのアンカーは行わない
- 今日マーカー: オレンジ色の破線縦線（`today` が範囲内の場合のみ）
- XML 特殊文字（`&`, `<`, `>`）はタイトル等でエスケープ済み

### 達成率の塗り分け（P1）

| 帯 | 進捗% | 達成部分の色 | CSS class |
|---|---|---|---|
| 未達 | 0 | （進捗 rect なし、背景のみ） | `bar-tier-none` |
| 低 | 1–33 | `#f59e0b` | `bar-tier-low` |
| 中 | 34–66 | `#6366f1` | `bar-tier-mid` |
| 高 | 67–99 | `#0ea5e9` | `bar-tier-high` |
| 完了 | 100 | `#22c55e` | `bar-tier-done` |

未達部分は常に `#d1d5db`（`bar-bg`）。凡例は既定で SVG 下部に `bar-tier-legend` として表示する。字送りは字ごとの幅の見積もり（和文 1 字は文字の大きさと同じ）で数え、UTF-8 のバイト数では数えない。図の幅が凡例の幅に満たない時（日数の短い図）は、図の幅を凡例の右端まで広げる。

`options_json` の `legend` に `"hidden"` を渡すと、凡例を描かず、図の高さを `LEGEND_H`（40）だけ詰める。
下の余白（`CHART_BOTTOM_PADDING_PX`、10）は残す。
その時の図は、既定の図から凡例を除き、高さと縦線の終わりを 40 縮めたものと同じになる。
日数で決まる幅が凡例の幅より狭い短い図（凡例のために幅を広げていた図）では、広げた幅も戻る。
その時、広げた幅の内側にだけあった今日の線（最後のタスクの終わりより後の今日）は出なくなり、進捗の線は今日が無い時の形になる。
Canvas の上限の判定は `legend` を見ず、凡例の分の高さを足したままにする。

### 色定数

| 用途 | 色 |
|---|---|
| バー未達（背景） | `#d1d5db` |
| 達成帯・低 | `#f59e0b` |
| 達成帯・中 | `#6366f1` |
| 達成帯・高 | `#0ea5e9` |
| 達成帯・完了 | `#22c55e` |
| 依存矢印 | `#9ca3af` |
| 進捗ステータスライン | `#ef4444` |
| グリッド線 | `#e5e7eb` |
| 今日マーカー | `#f59e0b` |
| ヘッダー背景 | `#f3f4f6` |
| グリッドラベル | `#6b7280` |

## レイアウト定数

| 定数 | 値 |
|---|---|
| `ROW_H` | 40.0 |
| `BAR_H` | 20.0 |
| `PX_PER_DAY` | 30.0 |
| `LABEL_W` | 120.0 |
| `HEADER_H` | 30.0 |
| `LEGEND_H` | 40.0 |
| `CHART_BOTTOM_PADDING_PX` | 10.0 |

図の高さは `HEADER_H`、行の高さの合計、`CHART_BOTTOM_PADDING_PX` に、凡例を出す時だけ `LEGEND_H` を足した値である。

`layout::assign_rows` の返却順は入力タスク順を保つが、各要素の `row` は依存関係を反映する。
Tarjan 法で強連結成分を縮約し、成分間をトポロジカル順に並べる。循環依存は同じ成分として扱い、
入力位置を使って決定的に並べる。

## テスト

- ネイティブ: `cargo nextest run`（`crates/koyori-arc-core/src/render.rs` 内の `#[cfg(test)] mod tests`、24本 + `progress.rs` 9本）
- Wasm: `wasm-pack test --node`（`crates/koyori-arc-core/tests/wasm.rs`、`wasm_bindgen_test_configure!(run_in_node_experimental)`、8本）
- ビジュアル確認: `cargo run --example preview`（`target/preview/index.html` を生成し、WSL2 では `explorer.exe` で自動オープン）

## CI / リリース

- `ci.yml`: `rust` job (nextest + wasm-pack test + npm パッケージビルド) → `vue` job (pnpm install + typecheck + build)
- タグ push (`v[0-9]*`) → `tag-release.yml` が git-cliff で changelog 生成 → GitHub Release 作成 → `release.yml` が npm (`@koyori-app/arc`, OIDC Trusted Publishing) + crates.io (`koyori-arc-core`, Trusted Publishing) へ publish
- changelog は Conventional Commits ベース。`chore`/`ci`/`style`/`test` は `<details>Other changes</details>` にまとめて表示

## 既知の未実装・将来課題

### P2

- arc-vue: 表示モード切り替え UI、ツールチップ（ネイティブ SVG `<title>` 以外のリッチツールチップ）

### P3

- arc-react（凍結中）
