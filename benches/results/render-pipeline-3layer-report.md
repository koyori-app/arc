# koyori-arc 描画パイプライン 3層ベンチマーク

## 計測条件

| 項目 | 値 |
|------|-----|
| CPU | AMD Ryzen 7 9800X3D 8-Core Processor |
| Rust | rustc 1.96.0 (ac68faa20 2026-05-25) |
| ビルド | `cargo bench --release`, `wasm-pack build --release` |
| ブラウザ/DOM | linkedom-fallback |
| 日付 | 2026-06-16 |

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
| 100_sparse | 100 | 99 | 0.22 | 0.94 | 3.89 | 71252 | 735 | 5.05 | **L3 DOM insert (browser)** (77.06%) |
| 100_dense | 100 | 494 | 0.5 | 1.1 | 5.14 | 139892 | 1130 | 6.74 | **L3 DOM insert (browser)** (76.25%) |
| 500_sparse | 500 | 499 | 1.13 | 2.22 | 13.35 | 349924 | 3533 | 16.7 | **L3 DOM insert (browser)** (79.94%) |
| 500_dense | 500 | 2534 | 2.55 | 4.83 | 23.9 | 709913 | 5568 | 31.28 | **L3 DOM insert (browser)** (76.4%) |
| 2000_sparse | 2000 | 1999 | 4.48 | 8.62 | 85.67 | 1401325 | 14018 | 98.77 | **L3 DOM insert (browser)** (86.74%) |
| 2000_dense | 2000 | 10184 | 10.23 | 19.3 | 181.03 | 2859890 | 22203 | 210.56 | **L3 DOM insert (browser)** (85.98%) |
| 5000_sparse | 5000 | 4999 | 11.07 | 21.4 | 190.84 | 3528651 | 34988 | 223.31 | **L3 DOM insert (browser)** (85.46%) |
| 5000_dense | 5000 | 25484 | 25.55 | 47.16 | 324.99 | 7214865 | 55473 | 397.7 | **L3 DOM insert (browser)** (81.72%) |

## 壁の位置 — 結論

- **支配的な壁（全8ケース中最多）**: L3 DOM insert (browser)
- **最大規模ケース（5000_dense）**: 合計 ~397.7 ms — 壁は **L3 DOM insert (browser)** (81.72%)

### 解釈

- SVG文字列が大きくなるほど **ブラウザ DOM 挿入・レイアウト** が支配的。Rust/Wasm 最適化だけでは頭打ち。
- 対策候補: 仮想化（表示行のみ描画）、Canvas/SVG チャンク分割、インクリメンタル更新。

## 再現手順

```bash
bash scripts/run-3layer-bench.sh
```
