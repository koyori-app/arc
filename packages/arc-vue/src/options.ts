import type { ColorIdName } from './replayCommands';

/**
 * The `options_json` string the Wasm entry points take.
 *
 * `today`, the scroll window and the time zone used to be three positional
 * arguments; they are keys of one JSON object now, so a new option adds a key
 * and not a position. Only the keys that have a value are written, and when
 * none has one the result is `undefined`, which the entry points read as the
 * defaults.
 */
export interface ScrollViewportOption {
  scroll_y: number;
  client_height: number;
}

/** Whether the chart draws its own legend. Left out, the legend is drawn. */
export type LegendMode = 'shown' | 'hidden';

/**
 * Whether the chart draws one kind of its own text: `rowLabels` the task titles
 * left of the days, `percentLabels` the `NN%` on each bar. Left out, it is drawn.
 */
export type LabelMode = 'shown' | 'hidden';

/**
 * Colours that replace the chart's own, by colour name. A value is a CSS
 * colour: `#hex`, `rgb()`/`hsl()`/`oklch()`, or `var(--name)` to draw with the
 * caller's design tokens. The crate refuses anything else.
 */
export type PaletteOverrides = Partial<Record<ColorIdName, string>>;

/** Where a bar sits in its row. Left out, it is centred. */
export type BarAlign = 'middle' | 'bottom';

/**
 * What the date header marks: `'week'` a line and `M/D` at each Monday (the
 * default), `'day'` every day's number and weekday under the month, `'month'`
 * a line and `YYYY/M` at the first of each month.
 */
export type Scale = 'week' | 'day' | 'month';

/**
 * The days a chart shows, `from` through `to` (YYYY-MM-DD, both included). A
 * bar running past either end is cut there and marked `← M/D` or `→ M/D`; a
 * task wholly outside is drawn as its mark alone.
 */
/**
 * How the chart draws the progress line: `'shown'` (the default) is the line as
 * before, `'hidden'` draws none, and `'by_row'` bends out from the today line to
 * each row that is late or ahead and back at its bottom edge.
 */
export type ProgressLineMode = 'shown' | 'hidden' | 'by_row';

export interface ViewRange {
  from: string;
  to: string;
}

export interface ChartOptionsInput {
  today?: string | null;
  scrollViewport?: ScrollViewportOption | null;
  tzOffsetMinutes?: number | null;
  legend?: LegendMode | null;
  rowLabels?: LabelMode | null;
  percentLabels?: LabelMode | null;
  rowHeightPx?: number | null;
  headerHeightPx?: number | null;
  barHeightPx?: number | null;
  barAlign?: BarAlign | null;
  palette?: PaletteOverrides | null;
  pxPerDay?: number | null;
  scale?: Scale | null;
  /** Weekday numbers drawn as weekend bands, 0 for Monday to 6 for Sunday. */
  weekendDays?: number[] | null;
  /** Seven labels, Monday first, for the `'day'` scale. */
  weekdayLabels?: string[] | null;
  view?: ViewRange | null;
  progressLine?: ProgressLineMode | null;
}

/**
 * The object `options_json` carries. Every key is optional, and a chart drawn
 * with none of them is the chart drawn before any key existed.
 */
export interface ChartOptions {
  today?: string;
  scroll_viewport?: ScrollViewportOption;
  tz_offset_minutes?: number | string;
  legend?: LegendMode;
  row_labels?: LabelMode;
  percent_labels?: LabelMode;
  row_height_px?: number;
  header_height_px?: number;
  bar_height_px?: number;
  bar_align?: BarAlign;
  palette?: PaletteOverrides;
  px_per_day?: number;
  scale?: Scale;
  weekend_days?: number[];
  weekday_labels?: string[];
  view?: ViewRange;
  progress_line?: ProgressLineMode;
}

export function buildOptionsJson(input: ChartOptionsInput): string | undefined {
  const options: ChartOptions = {};
  if (input.today != null) options.today = input.today;
  if (input.scrollViewport != null) options.scroll_viewport = input.scrollViewport;
  if (input.tzOffsetMinutes != null) {
    // JSON cannot carry NaN or Infinity (`JSON.stringify` would turn them into
    // `null`, which means "no offset"). A string keeps the unusable value
    // visible, so the entry point refuses it as it refuses any bad offset.
    options.tz_offset_minutes = Number.isFinite(input.tzOffsetMinutes)
      ? input.tzOffsetMinutes
      : String(input.tzOffsetMinutes);
  }
  // Only a given value is written. A key that is always present would turn
  // "left out" into a choice, and an old reader of the options would refuse it.
  if (input.legend != null) options.legend = input.legend;
  if (input.rowLabels != null) options.row_labels = input.rowLabels;
  if (input.percentLabels != null) options.percent_labels = input.percentLabels;
  if (input.rowHeightPx != null) options.row_height_px = input.rowHeightPx;
  if (input.headerHeightPx != null) options.header_height_px = input.headerHeightPx;
  if (input.barHeightPx != null) options.bar_height_px = input.barHeightPx;
  if (input.barAlign != null) options.bar_align = input.barAlign;
  if (input.palette != null) options.palette = input.palette;
  if (input.pxPerDay != null) options.px_per_day = input.pxPerDay;
  if (input.scale != null) options.scale = input.scale;
  if (input.weekendDays != null) options.weekend_days = input.weekendDays;
  if (input.weekdayLabels != null) options.weekday_labels = input.weekdayLabels;
  if (input.view != null) options.view = { from: input.view.from, to: input.view.to };
  if (input.progressLine != null) options.progress_line = input.progressLine;
  return Object.keys(options).length === 0 ? undefined : JSON.stringify(options);
}
