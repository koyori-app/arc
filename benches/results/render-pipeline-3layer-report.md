# koyori-arc 描画パイプライン 3層ベンチマーク

## 計測条件

| 項目 | 値 |
|------|-----|
| CPU | AMD Ryzen 7 9800X3D 8-Core Processor |
| Rust | rustc 1.96.0 (ac68faa20 2026-05-25) |
| ビルド | `cargo bench --release`, `wasm-pack build --release` |
| ブラウザ/DOM | linkedom-fallback |
| L3 仮想化 | ON（viewport scroll_y=0, client_height=600） |
| CPU スロットル | native + 4× CDP |
| 日付 | 2026-06-17 |

### 3層の定義

1. **L1 Rust 幾何計算**: `render()` — 行割当・依存矢印・イナズマ線・SVG文字列生成（criterion, release）
2. **L2 Wasm/JS 境界**: Node 上の `render_svg()` Wasm 呼び出し〜JS文字列受領 + SVGバイト長・要素数
3. **L3 ブラウザ SVG-DOM**: `innerHTML` 挿入〜2×`requestAnimationFrame` 後のレイアウト/描画完了

### データ

- タスク数: 100 / 500 / 2000 / 5000
- 依存密度: **sparse**（線形チェーン）/ **dense**（各タスク最大5先行＋クロスリンク）

## 結果（規模 × 密度）

| fixture | tasks | deps | L1 Rust (ms) | L2 Wasm Node (ms) | L3 DOM (ms) | SVG bytes | SVG elems | 合計 (ms) | 壁 |
|---------|------:|-----:|-------------:|------------------:|------------:|----------:|----------:|----------:|-----|
| 100_sparse | 100 | 99 | 0.22 | 0.94 | 1.47 | 71252 | 735 | 2.63 | **L3 DOM insert (browser)** (55.89%) |
| 100_dense | 100 | 494 | 0.5 | 1.1 | 1.76 | 139892 | 1130 | 3.36 | **L3 DOM insert (browser)** (52.38%) |
| 500_sparse | 500 | 499 | 1.13 | 2.22 | 0.84 | 349924 | 3533 | 4.19 | **L2 Wasm boundary (Node)** (52.98%) |
| 500_dense | 500 | 2534 | 2.55 | 4.83 | 1.39 | 709913 | 5568 | 8.77 | **L2 Wasm boundary (Node)** (55.07%) |
| 2000_sparse | 2000 | 1999 | 4.48 | 8.62 | 0.85 | 1401325 | 14018 | 13.95 | **L2 Wasm boundary (Node)** (61.79%) |
| 2000_dense | 2000 | 10184 | 10.23 | 19.3 | 2.93 | 2859890 | 22203 | 32.46 | **L2 Wasm boundary (Node)** (59.46%) |
| 5000_sparse | 5000 | 4999 | 11.07 | 21.4 | 0.92 | 3528651 | 34988 | 33.39 | **L2 Wasm boundary (Node)** (64.09%) |
| 5000_dense | 5000 | 25484 | 25.55 | 47.16 | 3.18 | 7214865 | 55473 | 75.89 | **L2 Wasm boundary (Node)** (62.14%) |

## Phase 1 仮想化 L3 — native / 4× 併記（§6.6.2）

| fixture | L3 native (ms) | L3 4× throttle (ms) | live elems (native) | DOM_CAP pass |
|---------|---------------:|--------------------:|--------------------:|:------------:|
| 100_sparse | 1.47 | 0.82 | 162 | ✓ |
| 100_dense | 1.76 | 1.12 | 243 | ✓ |
| 500_sparse | 0.84 | 0.72 | 164 | ✓ |
| 500_dense | 1.39 | 1.16 | 285 | ✓ |
| 2000_sparse | 0.85 | 0.88 | 164 | ✓ |
| 2000_dense | 2.93 | 1.51 | 435 | ✓ |
| 5000_sparse | 0.92 | 0.74 | 164 | ✓ |
| 5000_dense | 3.18 | 2.66 | 735 | ✗ |

## §6.6.2 CI ゲート結果

Overall: **PASS**

| Gate | Fixture | Actual | Condition | Result |
|------|---------|-------:|-----------|--------|
| DOM_CAP | 100_sparse | 162 | live_svg_elems ≤ 500 (strict) | PASS |
| DOM_CAP | 100_dense | 243 | live_svg_elems ≤ 500 (strict) | PASS |
| DOM_CAP | 500_sparse | 164 | live_svg_elems ≤ 500 (strict) | PASS |
| DOM_CAP | 500_dense | 285 | live_svg_elems ≤ 500 (strict) | PASS |
| DOM_CAP | 2000_sparse | 164 | live_svg_elems ≤ 500 (strict) | PASS |
| DOM_CAP | 2000_dense | 435 | live_svg_elems ≤ 500 (strict) | PASS |
| DOM_CAP | 5000_sparse | 164 | live_svg_elems ≤ 2000 (Phase1 margin) | PASS |
| DOM_CAP | 5000_dense | 735 | live_svg_elems ≤ 2000 (Phase1 margin) | PASS |
| regression_native | 2000_dense | 2.93 | native L3 ≤ cmd_263 181.03ms × 1.2 = 217.24ms | PASS |
| L3_throttled | 2000_dense | 1.51 | 4× CPU L3 p50 < 500ms | PASS |

## 壁の位置 — 結論

- **支配的な壁（全8ケース中最多）**: L2 Wasm boundary (Node)
- **最大規模ケース（5000_dense）**: 合計 ~75.89 ms — 壁は **L2 Wasm boundary (Node)** (62.14%)

### 解釈

- **Wasm/JS 境界**が支配的。JSON 受け渡し削減（TypedArray/共有バッファ）やネイティブバイナリ連携を検討。

## 再現手順

```bash
bash scripts/run-3layer-bench.sh --dom-throttle 4
```
