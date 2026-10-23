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

export interface ChartOptionsInput {
  today?: string | null;
  scrollViewport?: ScrollViewportOption | null;
  tzOffsetMinutes?: number | null;
  legend?: LegendMode | null;
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
  return Object.keys(options).length === 0 ? undefined : JSON.stringify(options);
}
