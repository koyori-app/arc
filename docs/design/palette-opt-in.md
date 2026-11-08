# 使う側の色で図を描く設計書

| 項目 | 値 |
|------|-----|
| 文書ID | `design/palette-opt-in` |
| 関連 | `design/external-rows`（この上に積む）、task の cmd_411（ガントの作り直し） |
| 状態 | 実装 |
| 基準 | `feat/gantt-external-rows` の `94481c6` |

## 1. 要約

使う側が、図の色を自分の色（task なら library の design token）に替えられるようにする。

task のガントは、左の一覧を ui-vue の部品と token で描く。右の arc の図が arc の固定の色のままだと、左右で色が揃わず、暗い表示にも付いて来ない。
`options_json` に `palette` を足し、色の名ごとに CSS の色を渡せるようにする。

| 問い | 採る答え |
|------|---------|
| 一、口の形 | `options_json` の `palette`。色の名（`ColorId`、例 `"HeaderBg"`）から CSS の色への object（3.1） |
| 二、渡せる色 | `#hex`、`rgb()`/`hsl()`/`oklch()` などの関数、`var(--name)`。それ以外は断る（3.2） |
| 三、canvas と `var()` | SVG はそのまま書く。canvas は arc-vue が canvas の要素の computed style から読み解く（4.2） |
| 四、新しい色の名 | 既定の色の表に入れず、既にある名へ落とす。最初は群れの帯の `GroupRow`（落ち先は `HeaderBg`）（4.1） |

既定は一字も変えない。`palette` を渡さぬ図は今までの図と同じで、出力を縛る試験の固定値（色の表を含む）も変えない（5）。

## 2. 今の作りで確かめたこと

- 色は `display_list/types.rs` の `Palette::standard()` の 13 色で、display list が `palette` として持つ。
- SVG、canvas、native の背面は、どれも display list の `palette` から色を読む。canvas は色の表を command buffer に入れて JS へ渡し、`replayCommands` が名から色を引く。
- 出力を縛る試験の固定値（`tests/fixtures/pin/*.json` など）は、色の表をまるごと含む。既定の色の表に一つでも足せば、全ての縛りが崩れる。

## 3. 口の形

### 3.1 `palette`

```json
{"palette": {"HeaderBg": "var(--koyori-color-surface)", "TierMid": "#6d28d9"}}
```

- 鍵は `ColorId` の名（`BarBg`、`TierLow`、`TierMid`、`TierHigh`、`TierDone`、`Dep`、`Progress`、`Grid`、`Today`、`HeaderBg`、`GridLabel`、`ProgressTextOnFg`、`ProgressTextOnBg`、`GroupRow`）。知らぬ名は断る。
- 渡した名だけを替え、他は既定の色のまま。図の配置は何も動かさない。色の表は図を組んだ後に差し替える。
- arc-vue: prop `palette`（型 `PaletteOverrides`）。

### 3.2 渡せる色

値は SVG の属性と canvas の style にそのまま入るので、属性を閉じたり別の物を開いたりできる字を通さない。

- `#rgb`、`#rgba`、`#rrggbb`、`#rrggbbaa`
- `var(--name)`。名は英数字、`-`、`_`
- `rgb()`、`rgba()`、`hsl()`、`hsla()`、`oklch()`、`oklab()`。引数は数字、小文字の英字、`.`、`,`、`%`、`/`、`+`、`-`、空白
- 100 字まで

色の名（`red` など）は通さない。必要が出れば後で足す。

## 4. 描き方

### 4.1 新しい色の名と落ち先

- `ColorId::GroupRow` を足し、群れの帯（`RectSemantic::GroupRow`）を描く色にした。
- `GroupRow` は既定の色の表に入れない。表に無い名は `ColorId::fallback` の落ち先（`GroupRow` なら `HeaderBg`）の色で描く。
  使う側が `palette` で `GroupRow` を渡した時だけ、表の末に加わる。
- この形なら、既定の色の表も、それを含む固定値も変わらない。後の変更で足す色（土日の地、厳守の菱形、警告など）も同じ形で足す。
- canvas の色の番号は `GroupRow` を 13 とする。arc-vue の `COLOR_ID_NAMES` と落ち先の表も同じにする。

### 4.2 canvas と `var()`

- canvas の `fillStyle` は `var(--name)` を読めない。arc-vue の `replayCommands` が、色が `var(--name)` の時は canvas の要素の computed style からその名の値を読む。使う側の token は、canvas の祖先に置かれていれば読める。値が無ければ黒（色の無い時と同じ）。
- SVG は `var(--name)` をそのまま属性に書き、ブラウザが解く。

## 5. 試験

- `tests/palette.rs`: 渡さぬ時と空の時は既定と同じ。替えた色が SVG と canvas の色の表に出る。群れの帯は渡すまで見出しの帯の色で、渡せばその色だけ変わる。渡せる色の形の全て。断る名と色。`Palette::with_overrides` の並び。
- 既存の試験と固定値は変えていない。
- arc-vue: `palette.test.ts`（鍵の書き方、`var()` の読み解き、群れの帯の落ち先）、`palette.wasm.test.ts`（本物の wasm で SVG と canvas の色の表）。
- Storybook: `PaletteFromTokens` と `PaletteFromTokensCanvas`（包みの要素に置いた CSS 変数で描く）。
