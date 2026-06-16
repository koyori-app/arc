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

`GanttTask` メソッド:
- `start_days(epoch)` — epoch からの経過日数
- `end_days(epoch)` — epoch から終了日までの経過日数。`end` が `None` の場合は `start + 1日` にフォールバック
- `progress()` — `progress_pct` (0-100) を 0.0-1.0 に正規化

## API

### ネイティブ (Rust)

```rust
pub fn render(tasks: &[GanttTask], deps: &[GanttDep], today: Option<NaiveDate>) -> String
```

### Wasm

```rust
#[wasm_bindgen]
pub fn render_svg(tasks_json: &str, deps_json: &str, today_iso: Option<String>) -> String
```

- `tasks_json` / `deps_json`: 上記構造体配列の JSON 文字列
- `today_iso`: 今日マーカー用の ISO 8601 日付文字列（例: `"2026-06-16"`）。省略可
- パースエラー時は `<!-- parse error: ... -->` を返す（例外を投げない）

### arc-vue

```vue
<GanttChart
  :tasks="tasks"
  :deps="deps"
  :today="todayIso"
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
  - 左ラベル: 長タイトルは 16 文字で `…` 省略（全文は `<title>` で参照可）
- ゼロ期間タスク: `◇` ダイヤモンドマーカー（`<polygon class="bar-milestone bar-tier-...">`）
- `end < start` ガード: 負の幅バーを生成しない（幅 0 扱い）
- 依存関係: frappe-gantt 互換の角丸エルボー矢印（`<path>` + オープンシェブロン）
- 進捗ステータスライン: 全タスクの進捗ポイントを結ぶ赤破線ジグザグ（`class="progress-status-line"`）— **設計意図どおり温存**
  - **1 タスク = 1 代表点**: `(progress_x, y_mid)`。`progress_x = start + (end - start) × progress`、`y_mid = (y_top + y_bottom) / 2`
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

未達部分は常に `#d1d5db`（`bar-bg`）。凡例は SVG 下部に `bar-tier-legend` として表示。

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

行割り当ては現状タスク配列の順序通り（`layout::assign_rows`）。トポロジカルソートによる依存関係を考慮した行割り当ては未実装。

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
- 依存関係を考慮したトポロジカル行割り当て

### P3

- arc-react（凍結中）
