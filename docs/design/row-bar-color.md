# 行ごとの色で棒を塗る設計書

| 項目 | 値 |
|------|-----|
| 文書ID | `design/row-bar-color` |
| 関連 | `design/palette-opt-in`（色の検めを共に使う）、`design/bar-marks`（状態の縁と字）、`design/external-rows`、issue #83、task の TASK-591 |
| 状態 | 設計 |
| 基準 | `master` の `b1699db`（v0.1.0-alpha.9 の後） |

## 1. 要約

使う側が、課題の行ごとに棒の色を渡せるようにする。

task の課題の詳細は、ブロックの関係を小さなガントで出し、棒を課題のステータスの色で塗る。
ステータスの色はプロジェクトごとに利用者が決める任意の色である。
今の棒の色は行の `state`（`normal`、`done`、`warn`）と色の表で決まり、行ごとに色を渡す口が無い。

| 問い | 採る答え |
|------|---------|
| 一、口の形 | `rows_json` の課題の行の `color`。arc-vue は `GanttRow` の課題の行に `color?: string \| null`（3.1） |
| 二、渡せる色 | `palette` と同じ検め（`is_css_color`）。通らぬ値は図ごと断る（3.2） |
| 三、`state` との関係 | 塗りは `color` が勝つ。縁の色と添えの字の色は `state` のまま（4.1） |
| 四、進みの濃い所 | 同じ色の濃淡。進んだ所は `color` そのまま、残りは同じ色を薄く（4.2） |
| 五、薄い色の時の読み | arc は対比を測らない。色を塗った棒には必ず縁を引き、棒の中の字は縁取りを付けて描く（4.3） |
| 六、display list への載せ方 | 色の名に `ColorId::Row(n)` を足し、行の色を色の表の末に加える。図形は今までどおり色の名を持つ（4.4） |
| 七、背面 | SVG は属性へ、canvas は色の番号 256 以上で表を引く。`var()` は `palette` と同じ扱い（4.5） |
| 八、凡例 | 変えない（4.6） |

`color` を持たぬ行は、今までの行と一字も違わない図になる。
`rows_json` を渡さぬ図も変わらない。
出力を縛る試験の固定値（`tests/fixtures/pin/*`）は変えない（5）。

## 2. 今の作りで確かめたこと

- 棒は三つの図形で描く。
  - 地: `RectPrim`（`RectSemantic::BarBackground`）。色は `bar_fill_color(state)`（`BarBg`、`DoneBg`、`WarnBg`）。
  - 進み: `RoundRectPrim`（`RoundRectSemantic::BarProgress`）。色は進みの段の色（`TierLow` ほか）。
  - 縁: `state` を書いた行だけ `PathPrim`（`PathSemantic::BarOutline`）。色は `outline_color(state)`。
- 期間の無い課題の菱形（`PolygonSemantic::Milestone`）は段の色で塗り、`BarBg` で縁を引く。点の形（`RoundRectSemantic::Point`）は `outline_color(state)` で塗る。
- 棒の中の割合の字は、進みが棒の 45% 以上なら `ProgressTextOnFg`（白）、それ未満なら `ProgressTextOnBg`（濃い灰）で描く。棒が短い時は棒の右の外へ出し、`ProgressTextOnBg` で描く。
- 図形が持つのは色の名（`ColorId`）だけで、色の値は display list の `palette` にある。SVG の背面は `Palette::resolve` で値を引いて属性へ書く。canvas の背面は名を `u8` の番号（`color_id_tag`、255 は色無し）にし、色の表と共に JS へ渡す。arc-vue の `replayCommands` が番号から名を引き、名から値を引く。
- `rows_json` の課題の行は、値が `null` の鍵をその鍵が無いのと同じに読む。
- 知らぬ鍵は断らず、黙って捨てる（`GanttRow` と `TaskRow` に `deny_unknown_fields` が無い）。`design/bar-marks` の 2 は「断る」と書くが、今の作りとは違う。そのため v0.1.0-alpha.9 までの arc に `color` を渡しても、断られずに灰の棒で描かれる。
- 色の表の検め（`render.rs` の `is_css_color`）は SVG の属性と canvas の style に入る値を縛る。

固定の色で縁と字を描くだけでは、どんな色にも読める棒にはならない。
次の表は、縁（`#6b7280`）と濃い字（`#374151`）の対比を WCAG の比で測ったものである。

| 棒の色 | 縁と白い地 | 縁と棒 | 濃い字と棒 | 白い字と棒 |
|--------|-----------|--------|-----------|-----------|
| `#ffffff` | 4.83 | 4.83 | 10.31 | 1.00 |
| `#ffff00` | 4.83 | 4.50 | 9.60 | 1.07 |
| `#1e3a8a` | 4.83 | 2.14 | 1.00 | 10.36 |
| `#6366f1` | 4.83 | 1.08 | 2.31 | 4.47 |

濃い字は暗い棒で読めず、白い字は薄い棒で読めない。
どちらか一色の字では足りないので、4.3 で縁取りの字を採る。

## 3. 口の形

### 3.1 `color`

課題の行 `{"kind":"task","task_id":…}` に `color` を足す。省ける。`null` は無いのと同じ。

```json
[{"kind": "task", "task_id": "a", "color": "var(--status-in-progress)", "state": "warn"}]
```

- 群れの行には持たせない。群れの帯の色は `palette` の `GroupRow` で一つに決まる。
- arc-vue: `GanttRow` の課題の行に `color?: string | null`。

`tasks_json`（arc-vue の `GanttTask`）ではなく行に持たせる。
`state` や `note` と同じく、色は課題そのものではなく使う側が行に添える見せ方だからである（`design/bar-marks` の 1）。
そのため、行の色を使うには `rows` で行を並べる必要がある。

### 3.2 渡せる色

`palette` の値と同じ検め（`is_css_color`）を通す。字の縛りも同じ（`#hex`、`rgb()`/`rgba()`/`hsl()`/`hsla()`/`oklch()`/`oklab()`、`var(--name)`、100 字まで）。

通らぬ値は、図ごと断る（`parse error: rows: color of `<task_id>`: …`）。
既定の色へ黙って落とさない。

- 値は SVG の属性と canvas の style に入る。属性を閉じる字（`"`、`>`）や `javascript:`、`url(` などは、どの形にも当たらないので断られる。
- 落とせば、書き損じた色が灰の棒になって気づかれない。`palette` の値と、`state` の知らぬ値も断っているので、それに揃える。
- 使う側は、利用者の決めた色を渡す前に形を検められる。断られた時に既定の色で描き直すかどうかは使う側が決める。

色の名（`red` など）を通さぬのも `palette` と同じである。

## 4. 描き方

### 4.1 `state` との関係

`color` を持つ行は、次のように描く。

| 部分 | `color` を持たぬ行（今まで） | `color` を持つ行 |
|------|-------------------------|----------------|
| 棒の地 | `bar_fill_color(state)` | `color` を薄く（4.2） |
| 進み | 段の色 | `color`（4.2） |
| 縁 | `state` を書いた時だけ `outline_color(state)` | いつも引く。色は `outline_color(state)`、`state` 無しは `BarStroke` |
| 点の形 | `outline_color(state)` で塗る | `color` で塗り、`outline_color(state)` の縁を引く |
| 期間の無い課題の菱形 | 段の色で塗り、`BarBg` の縁 | `color` で塗り、`outline_color(state)` の縁 |
| 添えの字 | `state` に従う（`warn` は `WarnStroke`） | 変えない |
| 厳守の期日の菱形 | `HardDeadline` | 変えない |
| 割合の字 | 4.3 の表 | 4.3 |

- 塗りは `color` が勝つ。`DoneBg` と `WarnBg` は使わない。
- `state` の印は縁の色と添えの字の色に残す。警告の棒は、利用者の色で塗っても `WarnStroke` の縁と字で警告と分かる。
- 縁をいつも引くのは、白い地に白い棒を塗っても棒の形が見えるようにするためである。`BarStroke` の落ち先 `GridLabel`（`#6b7280`）は白い地に対し 4.83 で、WCAG の非文字の対比（1.4.11）の 3 を超える。
- 期間の無い課題の菱形の縁を `BarBg` から縁の色に替えるのも同じ理由である。

### 4.2 進みの濃い所

進んだ所は `color` そのままで塗り、残り（棒の地）は同じ色を不透明度 `ROW_COLOR_REST_OPACITY`（0.35）で塗る。

- 渡される色は一つなので、別の色を選ぶ手がかりが無い。同じ色の濃淡なら、利用者の色のまま進みが読める。
- 濃淡は不透明度で作る。`var(--name)` の色でも arc が値を知らずに済み、SVG では `fill-opacity`、canvas では `globalAlpha` で描ける。`color-mix()` は使わない。canvas では `var()` を中に含む式を arc-vue が読み解く手間が増え、検めの字の縛り（3.2）にも新しい形を足すことになる。
- 100% の課題は棒の全てが `color` になる。0% の課題は全てが薄い色になる。
- 段（`ProgressTier`）は class 名と凡例のために今までどおり図形に持たせる。

`RectPrim` に `opacity: Option<f64>` を足す。`None` は今までの描き方で、出力に書かない（4.4）。

### 4.3 薄い色の時の読み

arc は描く時に対比を測らない。
どの色でも読める描き方を、色を見ずに選ぶ。

- 測れぬ色がある。`var(--name)` の値は、SVG ではブラウザが描く時に解くので core は知らない。canvas では arc-vue が描く時に読めるが、SVG と答えが割れる。
- 測れても、薄い地は図の地の色と重ねた色になる。図の地は使う側の CSS が決めるので、core は知らない。

そこで、`color` を持つ行の棒の中の割合の字は、字の色の周りに縁取り（halo）を付けて描く。

- 字の色は `ColoredBarText`、縁取りの色は `ColoredBarHalo`。どちらも既定の色の表に入れない新しい名で、落ち先は `ProgressTextOnBg`（`#374151`）と `ProgressTextOnFg`（`#ffffff`）。
- 字は縁取りの上に載るので、字の読みは棒の色によらず字と縁取りの対比で決まる。既定の二色では 10.31 で、WCAG の文字の対比（1.4.3）の 4.5 を超える。
- 縁取りの幅は `COLORED_BAR_HALO_PX`（3px。字の外へ 1.5px）。
- 棒の右の外へ出した割合の字と、期間の無い課題の菱形の右の字は、図の地の上にあるので今までどおり描く。
- 暗い表示の使う側は、`palette` で `ColoredBarText` と `ColoredBarHalo` を入れ替えられる。

`TextPrim` に `halo: Option<ColorId>` を足す。`None` は今までの描き方で、出力に書かない。

### 4.4 display list への載せ方

色の名に `ColorId::Row(u16)` を足し、行の色を色の表に加える。

- 図を組む時に、`color` を持つ行の色を出た順に集め、同じ字の色は一つにまとめる。n 番目の色を `ColorId::Row(n)` とし、色の表の末に `(Row(n), color)` を加える。
- 図形は今までどおり色の名を持つ。地は `Row(n)` に `opacity` を添え、進みと点と菱形の塗りは `Row(n)` を持つ。
- `color` を持つ行が無ければ、色の表は一つも増えない。既定の色の表も、それを含む固定値も変わらない。
- `ColorId::Row(n)` は `palette` の鍵にならない。`palette` の鍵は名の字から読むので、`"Row"` は知らぬ名として断られる。
- 色の数は行の数（`MAX_ROWS`、20,000）を超えないので `u16` に収まる。
- 足す欄（`RectPrim::opacity`、`TextPrim::halo`）は `None` の時に書き出さず、読む時は無ければ `None` とする。固定値の IR（`*.ir.json`）は変わらない。

図形に色の値を直に持たせる形（`fill_css: Option<String>` など）は採らない。

- 色の値が図形ごとに散る。今は値が色の表の一か所にだけあり、検めも、SVG の書き出しも、canvas の `var()` の読み解きも、表を通る。行の色もそこを通せば、危うい値の通り道が増えない。
- 地、進み、点、菱形、縁取りの字と、値を持つ図形の種類が多い。

`ColorId` は値を持つ variant を持つことになる。`with_overrides` の並べ替えが使う `*id as u8` は、variant の順を返す関数に替える。

### 4.5 SVG、canvas、native の背面

- SVG: 今までどおり `Palette::resolve` で値を引いて書く。`var(--name)` はそのまま書き、ブラウザが解く。`opacity` は `fill-opacity`、`halo` は `stroke`、`stroke-width`、`stroke-linejoin="round"`、`paint-order="stroke"` で描く。
- canvas: 色の番号の型を `u8` から `u16` に広げる。`Row(n)` の番号は `256 + n`。255 は今までどおり色無しである。JSON の数は変わらぬので、固定値の command buffer も変わらない。
  - `FillRect` に `alpha`、`DrawText` に `halo` を足す（どちらも `None` の時は書き出さない）。
  - arc-vue の `replayCommands` は 256 以上の番号を、色の表の `{"Row": n}` の鍵から引く。`var(--name)` は `palette` と同じく canvas の要素の computed style から読む（`design/palette-opt-in` の 4.2）。
  - `alpha` は `globalAlpha` で、`halo` は `strokeText` の後に `fillText` で描く。
  - `GanttChart` は、`palette` か行の `color` に `var()` を含み canvas で描いている時に、表示の切り替えを見張って描き直す（同 4.2 の見張りの条件に行の色を足す）。
- native: canvas と同じ番号と欄を持たせる。

### 4.6 凡例

凡例は変えない。

- 凡例の段の見本は、`color` を持たぬ行の進みの色の説明である。
- arc は行の色が何を表すか（ステータスの名）を知らない。行の色の凡例は、使う側が `legend: hidden` にして自分で描く。

## 5. 試験

- `default_output_pin`: 既存の固定値が一字も変わらない（`color` を持たぬ行、`rows_json` を渡さぬ図）。
- `tests/row_bar_color.rs`（新設）
  - SVG と canvas で、地が `color` の薄い色、進みが `color`、縁がいつも引かれ、割合の字に縁取りが付く。
  - `state` の縁と添えの字の色が残る。
  - 同じ色の行は色の表の一つを共にする。
  - `color: null` は無いのと同じ。
  - 渡せる色の形の全てを通し、危うい値（`"`、`>`、`javascript:`、`url(`、色の名、101 字）を断る。`palette` の鍵の `"Row"` を断る。
  - 点の形と期間の無い課題の菱形。
- arc-vue
  - `replayCommands` の 256 以上の番号、`alpha`、`halo`、`var()` の読み解き。
  - 本物の wasm で SVG と canvas の色の表を突き合わせる。
  - 見張り: 行の色の `var()` で描き直す。
- 読める確かめ: 薄い色（`#ffffff`、`#ffff00`）と濃い色（`#1e3a8a`、`#000000`）の棒を SVG と canvas で撮り、縁と地、字と縁取りの WCAG の比を測る。
- Storybook: 行ごとの色の物語（薄い色、濃い色、`var()`、`state` との重ね）。
