# ディスプレイリスト + バックエンド抽象 設計書

| 項目 | 値 |
|------|-----|
| 文書ID | `design/display-list-backend` |
| 親cmd | cmd_264 |
| 状態 | 設計（実装なし） |
| 参照ベンチ | cmd_263 `benches/results/render-pipeline-3layer-report.md` @ `bench/render-pipeline-3layer` `4eddd96` |
| レビュー | 軍師設計レビュー反映済み（2026-06-17） |

## 1. 要約

koyori-arc の描画パイプラインを **幾何計算 → ディスプレイリスト（中間表現, IR）→ バックエンド** の3段に分離する。現行 `render()` は L1 で SVG 文字列を直接生成しており、L3 の `innerHTML` 挿入が支配的ボトルネックである（cmd_263）。同一 IR を **SVG バックエンド**（既存互換・印刷/SEO/a11y）と **Canvas バックエンド**（大規模時の DOM ノード排除）が消費する。

**対策は2軸で評価する（軍師指摘 #1）:**

| 軸 | 内容 | 主な効果 |
|----|------|----------|
| **軸A: 仮想化** | 表示行のみ SVG 描画 | DOM ノード数をビューポート比例に抑制。a11y・テキスト鮮明さ温存 |
| **軸B: Canvas** | IR を Canvas2D で replay | DOM ノード ≈ 0。大規模 dense で L3 壁を根本回避 |

Canvas は銀の弾丸ではない。まず軸Aで中規模（〜2000 tasks, L3 < 200 ms）を救済し、軸Bは実測クロスオーバー後に段階導入する。

---

## 2. 背景 — cmd_263 ベンチ引用

### 2.1 3層定義

1. **L1 Rust 幾何計算**: `render()` — 行割当・依存矢印・イナズマ線・SVG 文字列生成
2. **L2 Wasm/JS 境界**: `render_svg()` 呼び出し〜JS 文字列受領
3. **L3 ブラウザ SVG-DOM**: `innerHTML` 挿入〜レイアウト/描画完了

### 2.2 計測結果（抜粋）

| fixture | tasks | deps | L1 (ms) | L2 (ms) | L3 (ms) | SVG elems | 壁 (L3 %) |
|---------|------:|-----:|--------:|--------:|--------:|----------:|----------:|
| 100_sparse | 100 | 99 | 0.22 | 0.94 | 3.89 | 735 | 77.1% |
| 2000_dense | 2000 | 10184 | 10.23 | 19.3 | 181.03 | 22203 | 86.0% |
| **5000_dense** | **5000** | **25484** | **25.55** | **47.16** | **324.99** | **55473** | **81.7%** |

- **全8ケースで壁 = L3 DOM insert**。L1/L2 は規模にほぼ線形だが、合計の 76〜87% を L3 が占める。
- L3 は **SVG 要素数（DOM ノード数）と強く相関**。5000_dense では SVG 7.2 MB / 55473 elems。
- **解釈**: SVG 文字列生成の最適化だけでは頭打ち。対策は (a) 描画ノード数削減（仮想化）または (b) DOM 自体の排除（Canvas）。

### 2.3 現行アーキテクチャの限界

```text
GanttTask[] + GanttDep[]
        ↓ render_graph() [render.rs]
        ↓ svg.push_str(format!(...))  ← 幾何とシリアライズが密結合
        ↓ String (SVG)
        ↓ render_svg() [wasm]
        ↓ GanttChart.vue v-html="svg"  ← L3 壁
```

---

## 3. ディスプレイリスト（IR）スキーマ

### 3.1 設計原則

- **バックエンド非依存**: SVG / Canvas2D / 将来 WebGL2 が同一 IR を消費する。
- **決定論**: 座標は `Coord` 型で **小数1桁固定**（`round(x * 10) / 10`）。現行テストが `"210,"` 等に依存するため、IR 段階で丸め規約を固定しバックエンド間の座標乖離を防ぐ（軍師 #9）。
- **パレット参照**: 色は文字列の再掲を禁止し `ColorId` enum で参照（軍師 #2）。

### 3.2 ルート構造

```rust
pub struct DisplayList {
    pub viewport: Viewport,           // 論理座標系（scroll/zoom 前）
    pub palette: Palette,             // ColorId → #rrggbb
    pub layers: Vec<Layer>,           // z-order 昇順
    pub metadata: ChartMetadata,      // a11y, epoch, today, task_index
}

pub struct Viewport {
    pub width: Coord,
    pub height: Coord,
    pub label_width: Coord,           // LABEL_W = 120.0
    pub header_height: Coord,         // HEADER_H = 30.0
    pub row_height: Coord,            // ROW_H = 40.0
}

pub struct ChartMetadata {
    pub title: String,                // "Gantt chart"
    pub description: String,
    pub task_bboxes: Vec<TaskBBox>,   // ヒットテスト・仮想化用
    pub primitive_count: u32,
    pub element_count_estimate: u32,  // SVG 換算 elems（閾値判定用）
}
```

### 3.3 レイヤとプリミティブ

```rust
pub enum LayerKind {
    Background,      // ヘッダー背景
    Grid,            // 週グリッド線 + 日付ラベル
    Dependencies,    // 依存矢印（path）
    Bars,            // タスクバー / マイルストーン
    ProgressLine,    // イナズマ線（progress-status-line）
    TodayMarker,     // 今日縦線
    Legend,          // 凡例（progress-line + bar-tier）
    OverlayHints,    // ヒット領域ヒント（canvas 用、描画しない）
}

pub struct Layer {
    pub kind: LayerKind,
    pub primitives: Vec<Primitive>,
}

pub enum Primitive {
    Rect(RectPrim),
    RoundRect(RoundRectPrim),
    Line(LinePrim),
    Path(PathPrim),
    Polyline(PolylinePrim),
    Polygon(PolygonPrim),
    Text(TextPrim),
    Group(GroupPrim),   // task_id 付きグループ
}

pub struct GroupPrim {
    pub task_id: Option<String>,
    pub bbox: BBox,
    pub children: Vec<Primitive>,
}
```

### 3.4 要素マッピング（現行 SVG → IR）

| 現行 SVG / 概念 | IR プリミティブ | 備考 |
|-----------------|-----------------|------|
| ヘッダー背景 `<rect>` | `RoundRect` / `Rect` | `Background` レイヤ |
| 週グリッド `<line>` + `<text>` | `Line` + `Text` | `Grid` レイヤ |
| タスクバー背景 `bar-bg` | `RoundRect` | `fill: ColorId::BarBg` |
| 達成率塗り `bar-progress bar-tier-*` | `RoundRect` | tier に応じた `ColorId::TierLow..Done` |
| 進捗% テキスト | `Text` | `font_weight: 600`, anchor 動的 |
| 左ラベル（16文字省略） | `Text` | `anchor: End` |
| ゼロ期間タスク `◇` | `Polygon` | `bar-milestone` 相当 |
| 依存矢印 `<path>` | `Path` | frappe-gantt 互換 elbow |
| イナズマ線 `<polyline class="progress-status-line">` | `Polyline` | today アンカー仕様は `progress.rs` 踏襲 |
| 今日マーカー破線縦線 | `Line` | `stroke_dash: [4,3]` |
| 凡例（progress-line + tier swatches） | `Line` + `Rect` + `Text` | `Legend` レイヤ |
| `<g data-task-id="...">` | `Group { task_id: Some(...) }` | ヒットテスト・a11y ミラー用 |

### 3.5 達成率帯（塗り分け）

| 帯 | 進捗% | ColorId |
|----|------:|---------|
| 未達 | 0 | `BarBg`（進捗 rect なし） |
| 低 | 1–33 | `TierLow` `#f59e0b` |
| 中 | 34–66 | `TierMid` `#6366f1` |
| 高 | 67–99 | `TierHigh` `#0ea5e9` |
| 完了 | 100 | `TierDone` `#22c55e` |

`progress_line`（イナズマ線）は **設計意図どおり温存**。today 指定時は端点を today 縦線にアンカー（`progress.rs` 既存仕様）。

### 3.6 TaskBBox（ヒットテスト・仮想化）

```rust
pub struct TaskBBox {
    pub task_id: String,
    pub row: u32,
    pub bbox: BBox,          // ラベル + バー全体
    pub bar_bbox: BBox,      // バー部分のみ（クリック優先）
}
```

Canvas バックエンドでは `data-task-id` が存在しないため、**空間インデックス（行単位の矩形リスト + 必要時 R-tree）** を IR から構築する（軍師 #4）。

---

## 4. バックエンド抽象

### 4.1 トレイト

```rust
pub trait RenderBackend {
  fn render(&self, list: &DisplayList) -> BackendOutput;
  fn name(&self) -> &'static str;
}

pub enum BackendOutput {
  Svg(String),
  CanvasCommands(CommandBuffer),
}
```

### 4.2 SVG バックエンド（既存 `render()` の再構成）

- `SvgBackend::render(list)` が IR を走査し、現行と**バイト級互換**の SVG を生成する。
- 現行 `render_graph()` の `svg.push_str` 群を `build_display_list()` + `SvgBackend` に分割。
- **Phase 0 受入**: 既存ネイティブ 37 テスト + wasm 8 テストをグリーンのまま維持。

### 4.3 Canvas2D バックエンド（一次採用）

#### 4.3.1 境界コスト対策（軍師 #3 — ship-blocker）

Wasm から `CanvasRenderingContext2d` をプリミティブ毎に呼ぶと、55473 回の境界往復で L2(47 ms) 以上の新壁を作る。**逐次 web-sys 呼び出しは禁止**。

採用方式: **コンパクトなコマンドバッファ**

```rust
pub struct CommandBuffer {
    pub viewport: Viewport,
    pub ops: Vec<DrawOp>,           // 列指向でも可（将来最適化）
    pub palette: Palette,
}

pub enum DrawOp {
    FillRect { x, y, w, h, color_id, radius },
    StrokePath { path_id, color_id, width },
    FillPath { path_id, color_id },
    StrokePolyline { points_id, color_id, width, dash },
    FillPolygon { points_id, color_id },
    DrawText { x, y, text_id, color_id, anchor, size, weight },
}
```

- Wasm は `CommandBuffer` を **bincode または flat typed-array** でシリアライズし JS に渡す（**プリミティブ毎 JSON 厳禁**）。
- JS 側 `replayCommands(ctx, buffer)` が単一ループで Canvas2D API を呼ぶ。境界往復は **1回（deserialize + replay）**。
- 想定サイズ: 55473 ops × 約 24 B ≈ 1.3 MB（7.2 MB SVG より小）。実測は Phase 2 spike で検証。

#### 4.3.2 devicePixelRatio

`canvas.width = cssWidth * dpr` で Retina ぼやけを防止。テキストはオーバーレイ DOM で描画するため、canvas 上のラベルは Phase 2 では簡略化可（読み取り専用モード）。

### 4.4 WebGL2 バックエンド（二次・条件付き）

- **トリガ**: Canvas2D replay が Nk プリミティブでフレーム予算（16 ms）を超過し、かつテキストをオーバーレイ DOM に完全委譲した後も path/polyline 描画が不足する場合のみ検討。
- **制約**: WebGL2 はネイティブテキスト描画なし → グリフアトラス/SDF が別プロジェクト規模。無条件採用しない（軍師 #7）。

### 4.5 WebGPU 非採用根拠（軍師 #7）

| 観点 | 評価 |
|------|------|
| ブラウザ成熟度 (2026-06) | Chrome/Edge は可。Safari WebGPU は段階的だが Firefox は依然制約あり。Gantt 埋め込み用途でフォールバック連鎖が複雑化 |
| 初期化 | async adapter/device 取得。チャート初回表示の TTFP を悪化 |
| テキスト | WebGL2 同様、テキストは別パイプライン必須 |
| 開発コスト | 2D Gantt チャートに対し ROI が極めて低い |
| フォールバック負担 | WebGPU → WebGL2 → Canvas2D の3段フォールバックは npm ライブラリとして保守不能 |

**結論**: WebGPU は本プロジェクトのスコープ外。将来、10万タスク級で WebGL2 も不足した場合に再評価（別 design doc）。

---

## 5. IME・親 Vue 整合 — HTML オーバーレイ層

Canvas 採用時、SVG で無償だったテキスト編集・IME・ヒットテスト・ツールチップが消失する。**オーバーレイ層を第一級設計**とする（軍師 #4 — ship-blocker）。

### 5.1 レイヤ構成

```text
┌─────────────────────────────────────────┐
│  .koyori-gantt (Vue root)               │
│  ┌───────────────────────────────────┐  │
│  │ <canvas> — IR replay (図形のみ)    │  │
│  └───────────────────────────────────┘  │
│  ┌───────────────────────────────────┐  │
│  │ .koyori-overlay (position:absolute)│  │
│  │  - 行ラベル <span> (仮想化: 可視行) │  │
│  │  - 編集中 <input> 1個のみ          │  │
│  │  - ツールチップ <div>              │  │
│  └───────────────────────────────────┘  │
│  ┌───────────────────────────────────┐  │
│  │ .koyori-a11y-mirror (sr-only)      │  │
│  │  - role="img" + aria-label         │  │
│  │  - タスクリスト <ul> (仮想化)       │  │
│  └───────────────────────────────────┘  │
└─────────────────────────────────────────┘
```

### 5.2 仮想化

- **縦スクロール**: 可視行 ± バッファ 2 行のみ DOM オーバーレイ要素を生成。
- **横スクロール/pan**: `transform: translate(-scrollX, -scrollY)` で canvas と overlay を同期。
- IR の `TaskBBox` + 行高さから可視行を O(log n) で判定。

### 5.3 IME 処理

編集モード時のみ `<input>` をタスクラベル位置に absolute 配置:

```typescript
input.addEventListener('compositionstart', onCompositionStart);
input.addEventListener('compositionupdate', onCompositionUpdate);
input.addEventListener('compositionend', onCompositionEnd);  // 確定後に emit
input.addEventListener('blur', onCommit);
```

- 変換中は `compositionend` まで親へ emit しない（日本語入力の文字化け防止）。
- 候補ウィンドウはブラウザネイティブに委譲（input の座標を viewport 内にクランプ）。

### 5.4 クリック → task-click 互換

現行:

```typescript
const el = (e.target as Element).closest('[data-task-id]');
```

Canvas バックエンド:

```typescript
function onCanvasClick(e: MouseEvent) {
  const { x, y } = clientToChartCoords(e, canvas, scroll);
  const hit = spatialIndex.hitTest(x, y);  // TaskBBox から構築
  if (hit) emit('taskClick', props.tasks.find(t => t.id === hit.task_id));
}
```

`spatialIndex` は IR の `task_bboxes` から構築し、pan/zoom 変更時に再生成しない（論理座標で保持、変換は hitTest 時に適用）。

### 5.5 アクセシビリティ（軍師 #5 — ship-blocker）

| モード | 方針 |
|--------|------|
| SVG バックエンド | 現行どおり `role="img"` + `<title>`/`<desc>` + テキストノード |
| Canvas バックエンド | **sr-only DOM ミラー**（タスク一覧 `ul/li`、進捗・日付を aria-label で提供） |
| 強制 SVG | prop `a11yMode="strict"` 時は canvas を選択せず SVG を強制 |

---

## 6. SVG → Canvas 切替閾値方針

### 6.1 指標

**タスク数ではなく推定 DOM 要素数（`element_count_estimate`）を主指標**とする（軍師 #6）。dense 依存は elems を 2〜3 倍にする。

cmd_263 実測の相関:

| fixture | tasks | elems | L3 (ms) |
|---------|------:|------:|--------:|
| 500_sparse | 500 | 3533 | 13.4 |
| 2000_dense | 2000 | 22203 | 181.0 |
| 5000_dense | 5000 | 55473 | 325.0 |

### 6.2 閾値（TBD — Phase 2 spike 後に確定）

現時点では **canvas 実測が無い**ため、数値閾値の断定は捏造となる。以下は**仮説**であり Phase 2 マイクロベンチで実クロスオーバーを測定して確定する:

| パラメータ | 仮説値 | 確定条件 |
|------------|--------|----------|
| `elems_switch_up` | ~15000 | canvas replay + overlay が L3 SVG より速い実測 |
| `elems_switch_down` | ~10000 | ヒステリシス下側（スラッシング防止） |
| `frame_budget_ms` | 16 | 60 fps 目標 |

### 6.3 API

```vue
<GanttChart
  :tasks="tasks"
  :deps="deps"
  backend="auto"   <!-- "svg" | "canvas" | "auto" -->
  a11yMode="auto"  <!-- "auto" | "strict" -->
/>
```

- `auto`: `element_count_estimate` と実測フレーム時間で判定。ヒステリシス付き。
- resize / データ更新時のバックエンド切替は **デバウンス 300 ms**。

### 6.4 軸A（仮想化）との関係

| 規模 | 推奨 |
|------|------|
| elems < 5000 | SVG + 仮想化なし（現行互換） |
| 5000 ≤ elems < 15000 | SVG + **行仮想化**（軸A）を先に適用 |
| elems ≥ 15000（仮説） | canvas バックエンド検討（軸B、要実測） |

---

## 7. Wasm/JS 境界 — エンコード方式

| 方式 | 境界往復 | サイズ | 判定 |
|------|----------|--------|------|
| プリミティブ毎 JSON | 大（配列 deserialize） | 大 | **禁止** |
| SVG 文字列（現行） | 1 | 7.2 MB @ 5000_dense | L3 で破綻 |
| CommandBuffer (bincode) | 1 | ~1.3 MB 推定 | **Canvas 推奨** |
| DisplayList (bincode) | 1 | ~1.5 MB 推定 | SVG バックエンドを JS 側で実行する場合 |

Phase 2 spike で L2 コストを計測し、L2 < 30 ms を gate とする。

---

## 8. 段階的移行ロードマップ

各 Phase は **受入テスト + ベンチゲート + SVG ロールバック可能** を条件とする。

### Phase 0: IR 抽出（挙動不変）

- `build_display_list(graph, epoch, today) -> DisplayList` を `render.rs` から分離。
- `SvgBackend` が現行 SVG を再生成。
- 既存全テストグリーン。ベンチ: L1/L3 が現行 ±5% 以内。

### Phase 1: SVG 仮想化（軸A）

- 可視行のみ IR 生成 → SVG 出力。
- `GanttChart.vue` に縦スクロールコンテナ追加。
- ベンチゲート: 2000_dense で L3 < 100 ms（目標）。

### Phase 2: Canvas 読み取り専用（軸B spike）

- `CanvasBackend` + `CommandBuffer` replay（JS 側）。
- feature flag / `backend="canvas"` opt-in。
- **マイクロベンチ**: canvas replay vs SVG DOM @ 100/500/2000/5000 × 疎/密。
- 閾値 `elems_switch_*` を実測で確定。

### Phase 3: オーバーレイ + IME + a11y

- 行ラベル DOM オーバーレイ、編集 input、ツールチップ。
- sr-only a11y ミラー。
- IME composition イベント処理。

### Phase 4: auto 切替

- `backend="auto"` + ヒステリシス。
- 性能回帰テストを CI に追加。

### SVG 温存方針

| 用途 | バックエンド |
|------|-------------|
| 印刷 / PDF 出力 | SVG（ベクター必須） |
| SEO / 静的エクスポート | SVG |
| 小規模（< 500 tasks） | SVG（オーバーヘッド最小） |
| a11y strict | SVG 強制 |
| 大規模インタラクティブ | Canvas + overlay |

---

## 9. テスト戦略

| 層 | 手法 |
|----|------|
| IR | ゴールデンスナップショット（決定論的座標） |
| SVG バックエンド | 現行文字列テストを維持 |
| Canvas バックエンド | CommandBuffer スナップショット（ピクセル diff は CI 不安定のため二次） |
| 統合 | Playwright: クリック → taskClick、仮想化スクロール |

---

## 10. メモリ見積（5000_dense）

| 形式 | サイズ |
|------|--------|
| SVG 文字列 | 7.2 MB |
| IR (推定) | ~2.0 MB（パレット共有・列指向化で ~1.5 MB まで圧縮可） |
| CommandBuffer (推定) | ~1.3 MB |

---

## 11. 軍師レビュー反映記録

| # | 指摘 | 反映箇所 |
|---|------|----------|
| 1 | 仮想化と canvas の2軸比較 | §1, §6.4 |
| 2 | IR 型定義・パレット化 | §3 |
| 3 | 境界コスト・CommandBuffer | §4.3.1, §7 |
| 4 | IME/オーバーレイ第一級設計 | §5 |
| 5 | a11y 退行防止 | §5.5 |
| 6 | 閾値 TBD + elems 指標 + ヒステリシス | §6 |
| 7 | WebGPU 非採用・WebGL2 条件 | §4.4, §4.5 |
| 8 | IR ゴールデンテスト | §9 |
| 9 | 座標丸め規約 | §3.1 |
| 10 | v-html 廃止（canvas 時） | §4.3（innerHTML 経路排除） |

---

## 12. 未決事項（Phase 2 まで）

1. `elems_switch_up/down` の確定値（canvas マイクロベンチ待ち）
2. CommandBuffer の bincode vs flat typed-array の実測比較
3. 横スクロール時の仮想化（列方向カリング）の要否

---

## 付録 A: 現行 `render()` との対応

```
render(tasks, deps, today)
  → build_display_list()     // 新規: 幾何のみ
  → SvgBackend::render()   // 新規: 既存 SVG と同等
```

Wasm API は Phase 0 で変更なし（`render_svg` → 内部で上記2段）。Phase 2 で `render_commands()` を追加予定。
